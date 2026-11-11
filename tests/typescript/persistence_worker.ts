// One process in a persistence test. Each invocation is a fresh process, so a snapshot written
// by one is read by another, as a real restart would.
//
//   start   <module> <state>        run to the first check, save the snapshot, exit
//   resume  <module> <state> <lints> restore the snapshot and finish the workflow
//   full    <module> ''      <lints> run the whole workflow in one process
//   refuse  <module> <state>        restore the snapshot into this module; must fail
//
// `<lints>` is a comma-separated list of lint outcomes, each `pass` or `fail`.
import { dirname, join, resolve } from 'node:path';

const [mode = '', modulePath = '', statePath = '', lints = ''] =
	process.argv.slice(2);
const file = resolve(modulePath);
const { graph } = await import(file);
const { restore, start } = await import(join(dirname(file), 'graphite.ts'));

const outcomes = lints === '' ? [] : lints.split(',');

function drive(run: any) {
	const trace = new Uint32Array(16);
	const visited: string[] = [];
	for (;;) {
		const node: string = run.node;
		visited.push(node);
		if (visited.length > 100)
			throw new Error(
				`workflow did not finish; visited ${visited.join(' ')}`
			);
		const task = graph
			.nodeAnnotations(node)
			.find((entry: { key: string }) => entry.key === 'task');
		// Conditions persist until set: clear the lint outcome at every other node, or a stale
		// `failed` sends the run back to `fix` forever without reaching a lint.
		const outcome =
			task?.value === 'lint' ? outcomes.shift() : undefined;
		if (task?.value === 'lint' && outcome === undefined) {
			throw new Error('lint ran more times than scripted');
		}
		run.set('passed', outcome === 'pass');
		run.set('failed', outcome === 'fail');
		if (run.status !== 'running') {
			return {
				visited,
				status: run.status,
				transitions: run.transitions,
			};
		}
		run.finish();
		const result = run.step(trace);
		if (result.status === 'error')
			throw new Error(`workflow stopped: ${result.error}`);
		if (result.taken === 0)
			throw new Error(`workflow stalled at ${run.node}`);
	}
}

try {
	switch (mode) {
		case 'start': {
			const run = start(graph);
			run.finish();
			run.step();
			await Bun.write(statePath, JSON.stringify(run.snapshot()));
			console.log(JSON.stringify({ node: run.node }));
			break;
		}
		case 'resume': {
			const snapshot = JSON.parse(await Bun.file(statePath).text());
			const run = restore(graph, snapshot);
			console.log(JSON.stringify(drive(run)));
			break;
		}
		case 'full': {
			console.log(JSON.stringify(drive(start(graph))));
			break;
		}
		case 'refuse': {
			const snapshot = JSON.parse(await Bun.file(statePath).text());
			restore(graph, snapshot);
			console.log('restored: should not happen');
			process.exitCode = 1;
			break;
		}
		default:
			throw new Error(`unknown mode "${mode}"`);
	}
} catch (error) {
	console.error(error instanceof Error ? error.message : String(error));
	process.exitCode = 3;
}
