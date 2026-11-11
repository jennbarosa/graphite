// Persistence across real process boundaries. Each step runs in its own Bun process through
// `persistence_worker.ts`. Compiled and run by `tests/persistence.rs`.
import { describe, expect, test } from 'bun:test';
import { join, resolve } from 'node:path';

const reviewModule = process.env.GRAPHITE_WORKFLOW;
const variantModule = process.env.GRAPHITE_VARIANT;
const stateDir = process.env.GRAPHITE_STATE_DIR;
if (
	reviewModule === undefined ||
	variantModule === undefined ||
	stateDir === undefined
) {
	throw new Error(
		'GRAPHITE_WORKFLOW, GRAPHITE_VARIANT and GRAPHITE_STATE_DIR must be set'
	);
}
const worker = resolve(import.meta.dir, 'persistence_worker.ts');
const state = join(stateDir, 'state.json');

/** Runs the worker as its own process and returns its exit code and output. */
function worker_(...args: string[]) {
	const result = Bun.spawnSync([process.execPath, worker, ...args]);
	return {
		code: result.exitCode,
		stdout: result.stdout.toString(),
		stderr: result.stderr.toString(),
	};
}

describe('persistence across processes', () => {
	test('a run saved by one process finishes in another, exactly as an uninterrupted run does', () => {
		const started = worker_('start', reviewModule, state);
		expect(started.code).toBe(0);
		expect(JSON.parse(started.stdout).node).toBe('check');

		const resumed = worker_('resume', reviewModule, state, 'fail,pass');
		expect(resumed.code).toBe(0);
		const full = worker_('full', reviewModule, '', 'fail,pass');
		expect(full.code).toBe(0);

		const tail = JSON.parse(resumed.stdout);
		const whole = JSON.parse(full.stdout);
		expect(tail.visited).toEqual(['check', 'fix', 'check', 'ship']);
		expect(tail.status).toBe('ended');
		expect(whole.visited).toEqual([
			'draft',
			'check',
			'fix',
			'check',
			'ship',
		]);
		expect(whole.visited.slice(1)).toEqual(tail.visited);
		expect(tail.transitions).toBe(whole.transitions);
	});

	test('a snapshot is refused by a workflow whose edges changed', () => {
		const started = worker_('start', reviewModule, state);
		expect(started.code).toBe(0);
		const refused = worker_('refuse', variantModule, state);
		expect(refused.code).not.toBe(0);
		expect(refused.stderr).toContain(
			'does not belong to graph "review"'
		);
	});
});
