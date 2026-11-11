// Command-line entry for `graphite run --durable`. It runs one generated graph under the durable
// host and prints the result as JSON. Usage:
//   bun run cli.ts <path/to/graph.ts> <state.json> [path/to/runner.ts]
// The runner module's default export is the task runner, with the `Runner` signature. Without one,
// every task settles as `none`. The run log goes to stderr.
import { resolve } from 'node:path';
import { type Runner, runDurable } from './durable';
import { runLog } from './run-log';

const [modulePath, statePath, runnerPath] = process.argv.slice(2);
if (!modulePath || !statePath) {
	console.error('usage: cli.ts <graph.ts> <state.json> [runner.ts]');
	process.exit(2);
}
const NEEDS_RUNNER = 3;

const { graph } = await import(resolve(modulePath));
// The outcome conditions are passed only when the graph declares both, so a graph without tasks
// runs as it is. A graph with tasks that lacks them fails in runDurable with a clear message.
const outcomeConditions =
	declares(graph, 'passed') && declares(graph, 'failed')
		? { pass: 'passed', fail: 'failed' }
		: undefined;

// A graph with tasks needs a runner. Checked before the run starts, so nothing is locked or recorded.
const tasks = taskNames(graph, 'task');
if (runnerPath === undefined && tasks.length > 0) {
	console.error(
		`the graph has tasks (${tasks.join(', ')}) but no runner: pass --runner <tasks.ts>, whose default export settles each task`
	);
	process.exit(NEEDS_RUNNER);
}
const runner: Runner =
	runnerPath === undefined
		? async () => {
				throw new Error(
					'no runner, and the graph has no tasks'
				);
			}
		: await loadRunner(runnerPath);

const result = await runDurable({
	modulePath: resolve(modulePath),
	stateFile: resolve(statePath),
	runner,
	taskAnnotation: 'task',
	retryFlag: 'retry',
	outcomeConditions,
	onEvent: runLog(console.error),
});
console.log(JSON.stringify(result));
process.exit(result.status === 'ended' ? 0 : 1);

/** Each node's task, as `node (task)`, for the nodes that have one. */
function taskNames(
	graph: {
		data: { nodeCount: number };
		nodeName(id: number): string;
		nodeAnnotations(
			node: string
		): readonly { key: string; value?: string }[];
	},
	annotation: string
): string[] {
	const names: string[] = [];
	for (let id = 0; id < graph.data.nodeCount; id++) {
		const node = graph.nodeName(id);
		const task = graph
			.nodeAnnotations(node)
			.find((entry) => entry.key === annotation)?.value;
		if (task !== undefined) names.push(`${node} (${task})`);
	}
	return names;
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
