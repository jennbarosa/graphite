// Command-line entry for `graphite run`. It runs one generated graph under the durable engine and
// prints the result as JSON. Usage:
//   bun run cli.ts <graph.ts> <state.json> [--runner <tasks.ts> | --tasks <tasks.json>]
//                  [--input <input.json>]
//                  [--each <item.ts> --key <field> [--seen <file.json>] [--until-passed <N>]]
// `--runner` names a module whose default export is the task runner, with the `Runner` signature.
// `--tasks` names a JSON file of task commands, `{ "cwd", "commands": { task: [argv...] } }`. Pass
// one of them, not both. `--input` names a JSON file whose value every task receives as
// `context.input`. `--each` runs the item graph once per item of the first graph's output, with the
// fan-out rules in fanout.ts. The run log goes to stderr.
import { mkdirSync, readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { type CommandTable, commandRunner } from './commands';
import { type DurableOptions, type Runner, runDurable } from './engine';
import { fanOut } from './fanout';
import { runLog } from './run-log';

const USAGE =
	'usage: cli.ts <graph.ts> <state.json> [--runner <tasks.ts> | --tasks <tasks.json>] [--input <input.json>] [--each <item.ts> --key <field>]';
const NEEDS_TASKS = 3;

const args = process.argv.slice(2);
const positional = args.filter((arg, index) => !isFlag(arg, index, args));
const [modulePath, statePath] = positional;
const runnerPath = flag(args, '--runner');
const tasksPath = flag(args, '--tasks');
const inputPath = flag(args, '--input');
const eachPath = flag(args, '--each');
const keyField = flag(args, '--key');
const seenPath = flag(args, '--seen');
const untilText = flag(args, '--until-passed');
if (!modulePath || !statePath) {
	console.error(USAGE);
	process.exit(2);
}
if (runnerPath !== undefined && tasksPath !== undefined) {
	console.error('--runner and --tasks both name the task code: pass one');
	process.exit(2);
}
if ((eachPath === undefined) !== (keyField === undefined)) {
	console.error('--each and --key go together: each item is run by --each, named by --key');
	process.exit(2);
}
if (eachPath === undefined && (seenPath !== undefined || untilText !== undefined)) {
	console.error('--seen and --until-passed apply to --each only');
	process.exit(2);
}
const untilPassed = untilText === undefined ? undefined : parseCount(untilText);

const { graph } = await import(resolve(modulePath));
const itemGraph = eachPath === undefined ? undefined : (await import(resolve(eachPath))).graph;
const table = tasksPath === undefined ? undefined : await loadTable(tasksPath);

// A graph with tasks needs a runner or a command for each task. Checked before the run starts,
// so nothing is locked or recorded.
for (const g of itemGraph === undefined ? [graph] : [graph, itemGraph]) {
	const tasks = taskNames(g, 'task');
	if (tasks.length > 0 && runnerPath === undefined && table === undefined) {
		console.error(
			`the graph has tasks (${tasks.map((t) => t.label).join(', ')}) but no runner or commands: pass --runner <tasks.ts> or --tasks <tasks.json>`
		);
		process.exit(NEEDS_TASKS);
	}
	if (table !== undefined) {
		const missing = tasks.filter((task) => !Object.hasOwn(table.commands, task.name));
		if (missing.length > 0) {
			console.error(
				`no command in ${table.source ?? tasksPath} for: ${missing.map((t) => t.name).join(', ')}`
			);
			process.exit(NEEDS_TASKS);
		}
	}
}

const runner = await pickRunner(table, runnerPath);
// One log sink for every run: it clears its own counts at each run's start.
const log = runLog(console.error);
const input = inputPath === undefined ? undefined : loadJson(inputPath);
const modulePathResolved = resolve(modulePath);
const statePathResolved = resolve(statePath);

/** The options for one run of `g` from `module`, with the state file and input given. */
const optionsFor = (
	g: unknown,
	module: string,
	stateFile: string,
	runInput: unknown
): DurableOptions => ({
	modulePath: module,
	stateFile,
	runner,
	input: runInput,
	taskAnnotation: 'task',
	retryFlag: 'retry',
	outcomeConditions: outcomeConditionsOf(g),
	onEvent: log,
});

if (itemGraph === undefined || eachPath === undefined || keyField === undefined) {
	const result = await runDurable(
		optionsFor(graph, modulePathResolved, statePathResolved, input)
	);
	console.log(JSON.stringify(result));
	process.exit(result.status === 'ended' ? 0 : 1);
}

// Fan-out: the state path is a directory for the item states, and the seen file defaults beside it.
const itemDir = `${statePathResolved}.items`;
mkdirSync(itemDir, { recursive: true });
const eachModule = resolve(eachPath);
const summary = await fanOut({
	list: optionsFor(graph, modulePathResolved, statePathResolved, input),
	item: (item, stateFile) => optionsFor(itemGraph, eachModule, stateFile, item),
	itemStateFile: (key) => `${itemDir}/${key}.state.json`,
	seenFile: seenPath === undefined ? `${statePathResolved}.seen.json` : resolve(seenPath),
	key: keyField,
	...(untilPassed === undefined ? {} : { untilPassed }),
	onItem: (key, result) => console.error(`[item] ${key} ${JSON.stringify(result)}`),
});
console.log(JSON.stringify(summary));
process.exit(summary.status === 'ended' ? 0 : 1);

/** The task runner: the task commands when given, else the runner module, else none. */
async function pickRunner(
	table: CommandTable | undefined,
	runnerFile: string | undefined
): Promise<Runner> {
	if (table !== undefined) return commandRunner(table);
	if (runnerFile !== undefined) return loadRunner(runnerFile);
	return async () => {
		throw new Error('no runner, and the graph has no tasks');
	};
}

/** The outcome conditions a graph needs: pass and fail, and errored when declared. */
function outcomeConditionsOf(
	g: unknown
): { pass: string; fail: string; error?: string } | undefined {
	const graph = g as { conditionId(name: string): number };
	if (!declares(graph, 'passed') || !declares(graph, 'failed')) return undefined;
	return {
		pass: 'passed',
		fail: 'failed',
		...(declares(graph, 'errored') ? { error: 'errored' } : {}),
	};
}

/** The value after `name`, or undefined when the flag is absent. */
function flag(list: string[], name: string): string | undefined {
	const index = list.indexOf(name);
	if (index === -1) return undefined;
	const value = list[index + 1];
	if (value === undefined) {
		console.error(`${name} needs a value`);
		process.exit(2);
	}
	return value;
}

/** Whether `arg` is a flag or the value of one, so it is not a positional argument. */
function isFlag(arg: string, index: number, list: string[]): boolean {
	if (arg.startsWith('--')) return true;
	const before = list[index - 1];
	return before !== undefined && before.startsWith('--');
}

function parseCount(text: string): number {
	const value = Number(text);
	if (!Number.isInteger(value) || value < 0) {
		console.error(`--until-passed needs a whole number, got ${text}`);
		process.exit(2);
	}
	return value;
}

/** Each task's node and name, for the nodes that have one. */
function taskNames(
	graph: {
		data: { nodeCount: number };
		nodeName(id: number): string;
		nodeAnnotations(node: string): readonly { key: string; value?: string }[];
	},
	annotation: string
): { name: string; label: string }[] {
	const found: { name: string; label: string }[] = [];
	for (let id = 0; id < graph.data.nodeCount; id++) {
		const node = graph.nodeName(id);
		const name = graph
			.nodeAnnotations(node)
			.find((entry) => entry.key === annotation)?.value;
		if (name !== undefined) found.push({ name, label: `${node} (${name})` });
	}
	return found;
}

function declares(graph: { conditionId(name: string): number }, name: string): boolean {
	try {
		graph.conditionId(name);
		return true;
	} catch {
		return false;
	}
}

async function loadRunner(path: string): Promise<Runner> {
	const module = await import(resolve(path));
	if (typeof module.default !== 'function') {
		throw new Error(`${path} must default-export the task runner`);
	}
	return module.default as Runner;
}

/** Reads the task table `graphite run --tasks` wrote. */
async function loadTable(path: string): Promise<CommandTable> {
	const table = loadJson(path) as Partial<CommandTable>;
	if (typeof table.cwd !== 'string' || typeof table.commands !== 'object' || table.commands === null) {
		throw new Error(`${path} is not a task table`);
	}
	return table as CommandTable;
}

/** Reads a JSON file. Any JSON value is allowed, including null. */
function loadJson(path: string): unknown {
	return JSON.parse(readFileSync(path, 'utf8'));
}
