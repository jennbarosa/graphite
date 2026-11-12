// Simulates one graph under the durable engine, with each task's outcome stubbed, so no task code
// runs. Usage:
//   bun durable/simulate.ts <graph.ts> [<task>=<outcomes>...]
// The stubs are parsed and answered by stubs.ts. A task with no stub settles as `pass`. The run
// prints one line per event and a last line for how it stopped. It exits 0 when the run ends,
// otherwise 1. A stub that is not valid exits 2.
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import type { GraphLike } from './contract';
import { runDurable } from './engine';
import { outcomeConditionsOf, taskNames } from './graph-checks';
import {
	type StubOutcome,
	eventLine,
	lineLimit,
	parseStubs,
	resultLine,
	stubRunner,
} from './stubs';

/** A simulated run stops after this many transitions, so a loop that never ends is reported. */
const SIMULATED_TRANSITIONS = 10_000;
/** Event lines printed before the rest are left out. */
const PRINTED_EVENT_LIMIT = 400;

function messageOf(error: unknown): string {
	return error instanceof Error ? error.message : String(error);
}

/** Runs the graph with the stubs, in a scratch state directory that is removed afterwards. */
async function simulate(
	modulePath: string,
	graph: GraphLike,
	stubs: ReadonlyMap<string, readonly StubOutcome[]>
): Promise<number> {
	const emit = lineLimit(PRINTED_EVENT_LIMIT, (line) => console.log(line));
	const dir = mkdtempSync(join(tmpdir(), 'graphite-simulate-'));
	try {
		const result = await runDurable({
			modulePath,
			stateFile: join(dir, 'state.json'),
			runner: stubRunner(stubs),
			taskAnnotation: 'task',
			retryFlag: 'retry',
			outcomeConditions: outcomeConditionsOf(graph),
			maxTransitions: SIMULATED_TRANSITIONS,
			onEvent: (event) => {
				const line = eventLine(event);
				if (line !== undefined) emit(line);
			},
		});
		const { line, ended } = resultLine(result);
		console.log(line);
		return ended ? 0 : 1;
	} catch (error) {
		console.error(`simulate: ${messageOf(error)}`);
		return 1;
	} finally {
		rmSync(dir, { recursive: true, force: true });
	}
}

async function main(argv: string[]): Promise<number> {
	const [modulePath, ...specs] = argv;
	if (modulePath === undefined) {
		console.error('usage: bun durable/simulate.ts <graph.ts> [<task>=<outcomes>...]');
		return 2;
	}
	const { graph } = await import(resolve(modulePath));
	const tasks = taskNames(graph, 'task').map((task) => task.name);
	let stubs: Map<string, StubOutcome[]>;
	try {
		stubs = parseStubs(specs, tasks);
	} catch (error) {
		console.error(`simulate: ${messageOf(error)}`);
		return 2;
	}
	const unstubbed = tasks.filter((task) => !stubs.has(task));
	if (unstubbed.length > 0) console.log(`no stub, so pass: ${unstubbed.join(', ')}`);
	return simulate(resolve(modulePath), graph, stubs);
}

// Nothing imports this file, so it runs `main` directly. It avoids top-level `await`, as cli.ts does.
main(process.argv.slice(2)).then((code) => {
	process.exitCode = code;
});
