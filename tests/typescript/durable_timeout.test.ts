// The `@timeout:MS` node annotation in the durable engine: it sets a node's limit per attempt, wins
// over the run-wide `taskTimeoutMs`, and is refused when it is not a whole number of milliseconds.
// Compiled and run by `tests/durable.rs`.
import { describe, expect, test } from 'bun:test';
import { existsSync, mkdtempSync } from 'node:fs';
import { join } from 'node:path';
import {
	type Outcome,
	type Runner,
	type RunEvent,
	runDurable,
} from '../../src/frontend/typescript/durable/engine';

const graphModule = process.env.GRAPHITE_TIMEOUTS;
const invalidModule = process.env.GRAPHITE_TIMEOUTS_INVALID;
const stateDir = process.env.GRAPHITE_STATE_DIR;
if (graphModule === undefined || invalidModule === undefined || stateDir === undefined) {
	throw new Error('GRAPHITE_TIMEOUTS, GRAPHITE_TIMEOUTS_INVALID and GRAPHITE_STATE_DIR must be set');
}

const names = {
	taskAnnotation: 'task',
	retryFlag: 'retry',
	outcomeConditions: { pass: 'passed', fail: 'failed' },
};

/** The node a task key belongs to: keys are `<transitions>:<node>`. */
function nodeOf(key: string): string {
	return key.slice(key.indexOf(':') + 1);
}

/** A fresh state file for one run. */
function stateFile(): string {
	return join(mkdtempSync(join(stateDir!, 'timeout-')), 'state.json');
}

/**
 * A runner for the timeouts graph. Every task finishes after a short wait, except the nodes in
 * `hangs`, which never settle, and `flaky` on attempts listed in `hangOn`. `work` takes about 30ms;
 * every other task passes at once. Records each call.
 */
function runnerFor(hangs: string[], hangOn: number[] = []) {
	const calls: Array<{ node: string; attempt: number }> = [];
	const runner: Runner = async (_task, context) => {
		const node = nodeOf(context.key);
		calls.push({ node, attempt: context.attempt });
		if (hangs.includes(node)) return new Promise<never>(() => {});
		if (node === 'flaky' && hangOn.includes(context.attempt)) {
			return new Promise<never>(() => {});
		}
		if (node === 'work') await Bun.sleep(30);
		return 'pass' as Outcome;
	};
	return { runner, calls };
}

function run(options: {
	runner: Runner;
	taskTimeoutMs?: number;
	onEvent?: (event: RunEvent) => void;
}) {
	return runDurable({
		modulePath: graphModule!,
		...names,
		stateFile: stateFile(),
		...options,
	});
}

describe('the timeout annotation on a node', () => {
	test('ends a task that outlives its own limit, with no run-wide limit set', async () => {
		const { runner } = runnerFor(['hang']);
		const result = await run({ runner });
		expect(result).toMatchObject({
			status: 'error',
			error: 'task write failed: timed out after 20ms',
		});
		expect(nodeOf((result as { key: string }).key)).toBe('hang');
	});

	test('wins over a larger run-wide limit', async () => {
		const { runner } = runnerFor(['hang']);
		const result = await run({ runner, taskTimeoutMs: 60_000 });
		expect(result).toMatchObject({
			status: 'error',
			error: 'task write failed: timed out after 20ms',
		});
	});

	test('does not cut a task whose limit is larger than the run-wide limit', async () => {
		// `work` takes about 30ms under a 5ms run-wide limit, but its own 5000ms limit applies.
		const settled: Record<string, string | undefined> = {};
		const { runner } = runnerFor(['hang']);
		const result = await run({
			runner,
			taskTimeoutMs: 5,
			onEvent: (event) => {
				if (event.type === 'task-settled') settled[nodeOf(event.key)] = event.outcome;
			},
		});
		expect(settled.work).toBe('pass');
		expect(result).toMatchObject({
			status: 'error',
			error: 'task write failed: timed out after 20ms',
		});
	});

	test('a retry node times out each attempt, and a later attempt can succeed', async () => {
		const attempts: string[] = [];
		const { runner, calls } = runnerFor([], [1]);
		const result = await run({
			runner,
			onEvent: (event) => {
				if (event.type === 'attempt-failed') {
					attempts.push(`${nodeOf(event.key)}#${event.attempt}: ${event.error}`);
				}
			},
		});
		expect(attempts).toEqual(['flaky#1: timed out after 20ms']);
		expect(calls.filter((call) => call.node === 'flaky').map((call) => call.attempt)).toEqual([1, 2]);
		expect(result.status).toBe('ended');
	});

	test('a retry node whose every attempt times out ends with that error', async () => {
		const { runner, calls } = runnerFor([], [1, 2, 3]);
		const result = await run({ runner });
		expect(result).toMatchObject({
			status: 'error',
			error: 'task write failed: timed out after 20ms',
		});
		expect(calls.filter((call) => call.node === 'flaky')).toHaveLength(3);
	});
});

describe('the run-wide limit without the annotation', () => {
	test('applies to a node that has no timeout annotation', async () => {
		const { runner } = runnerFor(['plain']);
		const result = await run({ runner, taskTimeoutMs: 20 });
		expect(result).toMatchObject({
			status: 'error',
			error: 'task write failed: timed out after 20ms',
		});
		expect(nodeOf((result as { key: string }).key)).toBe('plain');
	});

	test('a graph whose tasks all finish runs to the end with no limit set', async () => {
		const { runner } = runnerFor([]);
		const result = await run({ runner });
		expect(result.status).toBe('ended');
	});
});

describe('an invalid timeout annotation', () => {
	test('is refused before any task runs', async () => {
		let ran = 0;
		const file = stateFile();
		const promise = runDurable({
			modulePath: invalidModule!,
			...names,
			stateFile: file,
			runner: async () => {
				ran++;
				return 'pass' as Outcome;
			},
		});
		await expect(promise).rejects.toThrow(
			'node "slow" has timeout:soon; a timeout is a whole number of milliseconds'
		);
		expect(ran).toBe(0);
		expect(existsSync(file)).toBe(false);
	});
});
