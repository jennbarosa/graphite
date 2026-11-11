// Stage order on a graph whose `when passed` gate sits on a middle node. A stage's outcome must
// never reach the next node's gate, so every stage's task runs. Compiled and run by
// `tests/durable.rs`.
import { describe, expect, test } from 'bun:test';
import { mkdtempSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { runDurable } from '../../host/durable';

const graphModule = process.env.GRAPHITE_STAGE_ORDER;
const stateDir = process.env.GRAPHITE_STATE_DIR;
if (graphModule === undefined || stateDir === undefined) {
	throw new Error(
		'GRAPHITE_STAGE_ORDER and GRAPHITE_STATE_DIR must be set'
	);
}

const names = {
	taskAnnotation: 'task',
	retryFlag: 'retry',
	outcomeConditions: { pass: 'passed', fail: 'failed' },
};

async function load() {
	const { graph } = await import(graphModule as string);
	const runtime = await import(
		join(dirname(graphModule as string), 'graphite.ts')
	);
	return { graph, start: runtime.start as (graph: unknown) => any };
}

describe('advance takes one transition per call', () => {
	test('a stale passed condition does not carry through the next node', async () => {
		const { graph, start } = await load();
		const run = start(graph);
		run.set('passed', true);
		run.finish();
		expect(run.advance()).toBeGreaterThanOrEqual(0);
		expect(run.node).toBe('b');
		expect(run.transitions).toBe(1);
		// The host clears b's outcome before b's own move; a stale `passed` alone must not move it.
		run.set('passed', false);
		expect(run.advance()).toBe(-1);
		expect(run.node).toBe('b');
		expect(run.transitions).toBe(1);
	});

	test('a finished stage moves once and then waits for the next stage', async () => {
		const { graph, start } = await load();
		const run = start(graph);
		run.finish();
		expect(run.advance()).toBeGreaterThanOrEqual(0);
		expect(run.node).toBe('b');
		expect(run.advance()).toBe(-1);
		expect(run.node).toBe('b');
	});
});

describe('the durable host runs every stage it reaches', () => {
	test('a, b and c each run their task once, in order', async () => {
		const seen: string[] = [];
		const dir = mkdtempSync(join(stateDir, 'pass-'));
		const result = await runDurable({
			...names,
			modulePath: graphModule as string,
			stateFile: join(dir, 'state.json'),
			runner: async (_task: string, context) => {
				seen.push(context.key);
				return 'pass';
			},
		});
		expect(result.status).toBe('ended');
		expect(seen).toEqual(['0:a', '1:b', '2:c']);
	});

	test('a failing b stops the run without running c', async () => {
		const seen: string[] = [];
		const dir = mkdtempSync(join(stateDir, 'fail-'));
		const result = await runDurable({
			...names,
			modulePath: graphModule as string,
			stateFile: join(dir, 'state.json'),
			runner: async (_task: string, context) => {
				seen.push(context.key);
				return context.key.endsWith(':b') ? 'fail' : 'pass';
			},
		});
		expect(result.status).toBe('ended');
		expect(seen).toEqual(['0:a', '1:b']);
	});
});
