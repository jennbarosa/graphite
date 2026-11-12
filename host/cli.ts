// Command-line entry for `graphite run`. It runs one generated graph under the durable host and
// prints the result as JSON. Usage:
//   bun run cli.ts <graph.ts> <state.json> [--runner <tasks.ts>] [--input <input.json>]
//                  [--tasks <tasks.json>]
// `--runner` names a module whose default export is the task runner, with the `Runner` signature.
// `--tasks` names a JSON file of task commands, `{ "cwd", "commands": { task: [argv...] } }`. Pass
// one of them, not both. `--input` names a JSON file whose value every task receives as
// `context.input`. The run log goes to stderr.
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { type CommandTable, commandRunner } from './commands';
import { type Runner, runDurable } from './durable';
import { runLog } from './run-log';

const USAGE =
	'usage: cli.ts <graph.ts> <state.json> [--runner <tasks.ts> | --tasks <tasks.json>] [--input <input.json>]';
const NEEDS_TASKS = 3;

const args = process.argv.slice(2);
const positional = args.filter((arg, index) => !isFlag(arg, index, args));
const [modulePath, statePath] = positional;
const runnerPath = flag(args, '--runner');
const tasksPath = flag(args, '--tasks');
const inputPath = flag(args, '--input');
if (!modulePath || !statePath) {
	console.error(USAGE);
	process.exit(2);
}
if (runnerPath !== undefined && tasksPath !== undefined) {
	console.error('--runner and --tasks both name the task code: pass one');
	process.exit(2);
}

const { graph } = await import(resolve(modulePath));
// The outcome conditions are passed only when the graph declares both, so a graph without tasks
// runs as it is. A graph with tasks that lacks them fails in runDurable with a clear message.
const outcomeConditions =
	declares(graph, 'passed') && declares(graph, 'failed')
		? { pass: 'passed', fail: 'failed' }
		: undefined;

const tasks = taskNames(graph, 'task');
const table = tasksPath === undefined ? undefined : await loadTable(tasksPath);
// A graph with tasks needs a runner or a command for each task. Checked before the run starts,
// so nothing is locked or recorded.
if (tasks.length > 0 && runnerPath === undefined && table === undefined) {
	console.error(
		`the graph has tasks (${tasks.map((t) => t.label).join(', ')}) but no runner or commands: pass --runner <tasks.ts> or --tasks <tasks.json>`
	);
	process.exit(NEEDS_TASKS);
}
if (table !== undefined) {
	const missing = tasks.filter(
		(task) => !Object.hasOwn(table.commands, task.name)
	);
	if (missing.length > 0) {
		console.error(
			`no command in ${table.source ?? tasksPath} for: ${missing.map((t) => t.name).join(', ')}`
		);
		process.exit(NEEDS_TASKS);
	}
}

const runner: Runner =
	table !== undefined
		? commandRunner(table)
		: runnerPath !== undefined
			? await loadRunner(runnerPath)
			: async () => {
					throw new Error('no runner, and the graph has no tasks');
				};

const result = await runDurable({
	modulePath: resolve(modulePath),
	stateFile: resolve(statePath),
	runner,
	input: inputPath === undefined ? undefined : loadInput(inputPath),
	taskAnnotation: 'task',
	retryFlag: 'retry',
	...(outcomeConditions === undefined ? {} : { outcomeConditions }),
	onEvent: runLog(console.error),
});
console.log(JSON.stringify(result));
process.exit(result.status === 'ended' ? 0 : 1);

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

/** Each task's node and name, for the nodes that have one. */
function taskNames(
	graph: {
		data: { nodeCount: number };
		nodeName(id: number): string;
		nodeAnnotations(
			node: string
		): readonly { key: string; value?: string }[];
	},
	annotation: string
): { node: string; name: string; label: string }[] {
	const found: { node: string; name: string; label: string }[] = [];
	for (let id = 0; id < graph.data.nodeCount; id++) {
		const node = graph.nodeName(id);
		const name = graph
			.nodeAnnotations(node)
			.find((entry) => entry.key === annotation)?.value;
		if (name !== undefined) found.push({ node, name, label: `${node} (${name})` });
	}
	return found;
}

function declares(
	graph: { conditionId(name: string): number },
	name: string
): boolean {
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
	const value: unknown = JSON.parse(readFileSync(path, 'utf8'));
	if (
		typeof value !== 'object' ||
		value === null ||
		typeof (value as { cwd?: unknown }).cwd !== 'string' ||
		typeof (value as { commands?: unknown }).commands !== 'object' ||
		(value as { commands?: unknown }).commands === null
	) {
		throw new Error(`${path} is not a task table`);
	}
	return value as CommandTable;
}

/** Reads the run's input. Any JSON value is allowed, including null. */
function loadInput(path: string): unknown {
	return JSON.parse(readFileSync(path, 'utf8'));
}
