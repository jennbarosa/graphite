// A headless host driving the workflow fixture. Each stage's `@task` annotation names the work
// to do; a scripted runner stands in for the work and reports its outcome, which the host turns
// into conditions. Compiled and run by `tests/runtime_accessors.rs`.
import { describe, expect, test } from 'bun:test';
import { dirname, join } from 'node:path';

const modulePath = process.env.GRAPHITE_WORKFLOW;
if (modulePath === undefined) {
	throw new Error(
		'GRAPHITE_WORKFLOW must name the compiled workflow module'
	);
}
const { graph } = await import(modulePath);
const { start, restore } = await import(
	join(dirname(modulePath), 'graphite.ts')
);

type Outcome = 'pass' | 'fail' | 'ok';

/** Runs the workflow to its end. `runner` performs one task and returns its outcome. */
function runWorkflow(runner: (task: string) => Outcome, limit = 20) {
	const run = start(graph);
	const trace = new Uint32Array(16);
	const visited: string[] = [];
	const tasks: string[] = [];
	for (;;) {
		const node = run.node;
		visited.push(node);
		if (visited.length > limit) {
			throw new Error(`workflow exceeded ${limit} stages`);
		}
		const task = graph
			.nodeAnnotations(node)
			.find((entry: { key: string }) => entry.key === 'task');
		if (task !== undefined) {
			tasks.push(task.value);
			const outcome = runner(task.value);
			run.set('passed', outcome === 'pass');
			run.set('failed', outcome === 'fail');
		}
		if (run.status !== 'running') {
			return { visited, tasks, status: run.status };
		}
		run.finish();
		const result = run.step(trace);
		if (result.status === 'error') {
			throw new Error(`workflow stopped: ${result.error}`);
		}
		if (result.taken === 0) {
			throw new Error(`workflow stalled at ${run.node}`);
		}
	}
}

/** A runner whose lint task returns the given outcomes in turn; every other task passes. */
function lintReturns(...outcomes: Outcome[]) {
	const queue = [...outcomes];
	return (task: string): Outcome => {
		if (task !== 'lint') return 'ok';
		const outcome = queue.shift();
		if (outcome === undefined)
			throw new Error('lint ran more times than scripted');
		return outcome;
	};
}

describe('workflow host', () => {
	test('a failing lint goes through fix and back, then ships', () => {
		const result = runWorkflow(lintReturns('fail', 'pass'));
		expect(result.visited).toEqual([
			'draft',
			'check',
			'fix',
			'check',
			'ship',
		]);
		expect(result.tasks).toEqual([
			'write',
			'lint',
			'fix',
			'lint',
			'deploy',
		]);
		expect(result.status).toBe('ended');
	});

	test('a passing lint ships without visiting fix', () => {
		const result = runWorkflow(lintReturns('pass'));
		expect(result.visited).toEqual(['draft', 'check', 'ship']);
		expect(result.tasks).toEqual(['write', 'lint', 'deploy']);
		expect(result.status).toBe('ended');
	});

	test('a lint that never passes is stopped by the host limit, not left to spin', () => {
		const alwaysFails = (task: string): Outcome =>
			task === 'lint' ? 'fail' : 'ok';
		expect(() => runWorkflow(alwaysFails, 10)).toThrow(
			'exceeded 10 stages'
		);
	});

	test('the runtime limit stops a run that keeps looping, even if the host never stops it', () => {
		const run = start(graph, { maxTransitions: 3 });
		const trace = new Uint32Array(16);
		for (let i = 0; i < 20 && run.status === 'running'; i++) {
			run.set('failed', true);
			run.finish();
			run.step(trace);
		}
		expect(run.status).toBe('error');
		expect(run.error).toBe('limit');
		expect(run.transitions).toBe(3);
	});

	test('a snapshot taken mid-run, stored as JSON, resumes where it left off', () => {
		const run = start(graph);
		run.finish();
		run.step();
		const stored = JSON.stringify(run.snapshot());
		const resumed = restore(graph, JSON.parse(stored));
		expect(resumed.node).toBe('check');
		expect(resumed.transitions).toBe(1);
		resumed.set('passed', true);
		const result = resumed.step();
		expect(result.node).toBe('ship');
		expect(result.status).toBe('ended');
		expect(result.transitions).toBe(2);
	});

	test('a snapshot keeps the finished flag, so the sequence edge still fires after restore', () => {
		const run = start(graph);
		run.finish();
		const resumed = restore(
			graph,
			JSON.parse(JSON.stringify(run.snapshot()))
		);
		expect(resumed.step().node).toBe('check');
	});

	test('a snapshot keeps condition values', () => {
		const run = start(graph);
		run.set('failed', true);
		const resumed = restore(
			graph,
			JSON.parse(JSON.stringify(run.snapshot()))
		);
		expect(resumed.holds('failed')).toBe(true);
		expect(resumed.holds('passed')).toBe(false);
	});

	test('a snapshot from another graph is refused', async () => {
		const { graph: walk } = await import(
			join(dirname(modulePath), 'walk.ts')
		);
		expect(() => restore(walk, start(graph).snapshot())).toThrow(
			'does not belong to graph "walk"'
		);
	});

	test('a snapshot with state the graph cannot be in is refused', () => {
		const good = start(graph).snapshot();
		expect(() => restore(graph, { ...good, bits: [] })).toThrow(
			'cannot be in'
		);
		expect(() => restore(graph, { ...good, node: 99 })).toThrow(
			'cannot be in'
		);
		expect(() =>
			restore(graph, { ...good, phase: 'paused' as 'running' })
		).toThrow('cannot be in');
		expect(() => restore(graph, { ...good, version: 2 as 1 })).toThrow(
			'unsupported snapshot version'
		);
	});
});
