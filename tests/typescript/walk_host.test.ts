// A host for the spec's walk cycle, compiled from `fixtures/runtime/walk_spec.graphite` by
// `cargo test --test pipeline writes_the_spec_walk_cycle_for_the_host`.
import { describe, expect, test } from 'bun:test';

const root = process.env.GRAPHITE_WALK_HOST ?? '../../target/tmp/walk-host';
const { start } = await import(`${root}/graphite.ts`);
const { graph } = await import(`${root}/walk.ts`);

describe('walk cycle host', () => {
	test('the graph line annotations are readable', () => {
		expect(graph.annotations()).toEqual([
			{ key: 'choice', value: 'facing' },
			{ key: 'idle', value: 'stopped' },
		]);
	});

	test('loop and end declarations are readable without annotations', () => {
		expect(graph.nodeLooping('going')).toBe(true);
		expect(graph.nodeLooping('windup')).toBe(false);
		expect(graph.nodeEnd('stopped')).toBe(true);
		expect(graph.nodeEnd('going')).toBe(false);
		expect(() => graph.nodeLooping('nosuch')).toThrow(RangeError);
	});

	test('entry is windup; a finish moves into the looping walk node', () => {
		const run = start(graph);
		expect(run.node).toBe('windup');
		run.finish();
		const result = run.step();
		expect(result.node).toBe('going');
		expect(result.status).toBe('running');
	});

	test('keyup while walking winds down and ends', () => {
		const run = start(graph);
		run.finish();
		run.step();
		run.set('keyup', true);
		const result = run.step();
		expect(result.node).toBe('winddown');
		expect(result.status).toBe('ended');
	});

	test('keyup during windup stops immediately, beating the sequence edge', () => {
		const run = start(graph);
		run.set('keyup', true);
		const result = run.step();
		expect(result.node).toBe('stopped');
		expect(result.status).toBe('ended');
	});

	test('the walk node is marked looping in the compiled data', () => {
		expect(graph.nodeRecord(1).looping).toBe(true);
		expect(graph.nodeRecord(0).looping).toBe(false);
	});

	test('choose resolves facing, with the else arm last', () => {
		const run = start(graph);
		run.set('left', true);
		expect(run.choose('facing')).toEqual({
			condition: 'left',
			tag: 'side',
			annotations: [{ key: 'flip', value: 'true' }],
		});
		run.set('left', false);
		run.set('up', true);
		expect(run.choose('facing')?.tag).toBe('up');
		run.set('up', false);
		expect(run.choose('facing')?.tag).toBe('down');
	});
});
