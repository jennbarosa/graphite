// Runs the runtime against modules exported by `tests/typescript_frontend.rs`:
//   mkdir -p target/ts-fixtures
//   GRAPHITE_CODEGEN_TEST_OUTPUT=$PWD/target/ts-fixtures cargo test --test typescript_frontend export_modules_for_cross_language_checks
//   bun test tests/typescript
//
// Each behavior runs twice: against default chunks (one chunk per small graph) and against
// two-item chunks, where the node and choice chunk boundaries are actually crossed.

import { describe, expect, test } from 'bun:test';

async function load(root: string) {
	const runtime = await import(`${root}/runtime/graphite.ts`);
	const graph = async (folder: string, name: string) =>
		(await import(`${root}/${folder}/${name}.ts`)).graph;
	return {
		start: runtime.start as (graph: unknown) => any,
		door: await graph('door', 'door'),
		ended: await graph('runtime', 'ended_at_start'),
		priority: await graph('runtime', 'priority'),
		sequenceLast: await graph('runtime', 'sequence_last'),
		stall: await graph('runtime', 'stall'),
		spin: await graph('runtime', 'spin'),
		tail: await graph('runtime', 'tail'),
		chooser: await graph('runtime', 'chooser'),
		strict: await graph('runtime', 'strict'),
		wide: await graph('runtime', 'wide'),
	};
}

function suite(label: string, g: Awaited<ReturnType<typeof load>>) {
	const {
		start,
		door,
		ended,
		priority,
		sequenceLast,
		stall,
		spin,
		tail,
		chooser,
		strict,
		wide,
	} = g;

	describe(label, () => {
		describe('start', () => {
			test('enters the entry node, the first node declared', () => {
				const run = start(priority);
				expect(run.node).toBe('a');
				expect(run.status).toBe('running');
			});

			test('an entry node marked end ends the run immediately', () => {
				const run = start(ended);
				expect(run.node).toBe('only');
				expect(run.status).toBe('ended');
			});

			test('the graph exposes its name', () => {
				expect(priority.name).toBe('priority');
			});
		});

		describe('gates', () => {
			test('an idle node with no true gate and no finish stays put', () => {
				const run = start(priority);
				const result = run.step();
				expect(result.node).toBe('a');
				expect(result.status).toBe('running');
				expect(result.transitions).toBe(0);
			});

			test('the first declared true gate wins', () => {
				const run = start(priority);
				run.set('go', true);
				const result = run.step();
				expect(result.node).toBe('b');
				expect(result.status).toBe('ended');
				expect(result.transitions).toBe(1);
			});

			test('a true gate beats a sequence edge, even when the sequence is declared first', () => {
				const run = start(sequenceLast);
				run.finish();
				run.set('go', true);
				expect(run.step().node).toBe('b');
			});

			test('conditions are not consumed by transitions', () => {
				// closed -> opening when request; opening -> open; open -> closing when timeout;
				// closing -> closed; closed -> opening when request. The request bit stays set.
				const run = start(door);
				run.set('request', true);
				expect(run.step().node).toBe('opening');
				run.finish();
				expect(run.step().node).toBe('open');
				run.set('timeout', true);
				expect(run.step().node).toBe('closing');
				run.finish();
				const result = run.step();
				expect(result.node).toBe('opening');
				expect(result.transitions).toBe(5);
			});
		});

		describe('finish', () => {
			test('a sequence self-transition runs once per finish and then stalls', () => {
				const run = start(stall);
				run.finish();
				const first = run.step();
				expect(first.node).toBe('loop');
				expect(first.transitions).toBe(1);
				expect(first.status).toBe('running');
				const second = run.step();
				expect(second.transitions).toBe(1);
				expect(second.status).toBe('running');
			});

			test('finish on an ended run changes nothing', () => {
				const run = start(ended);
				run.finish();
				expect(run.status).toBe('ended');
				expect(run.step().transitions).toBe(0);
			});
		});

		describe('cycles', () => {
			test('a true self gate reports a cycle and keeps the taken prefix', () => {
				const run = start(spin);
				run.set('go', true);
				const result = run.step();
				expect(result.status).toBe('error');
				expect(result.error).toBe('cycle');
				expect(result.transitions).toBe(2);
			});

			test('a tail into a cycle is detected after the cycle repeats', () => {
				// a -> b (sequence), then b <-> c while go is true.
				const run = start(tail);
				run.finish();
				run.set('go', true);
				const result = run.step();
				expect(result.status).toBe('error');
				expect(result.error).toBe('cycle');
				expect(result.transitions).toBe(4);
			});
		});

		describe('chunks', () => {
			test('a chain walks across node chunk boundaries one finish at a time', () => {
				const run = start(wide);
				for (let i = 1; i <= 9; i++) {
					run.finish();
					expect(run.step().node).toBe(`n${i}`);
				}
				expect(run.status).toBe('ended');
			});

			test('choices in later choice chunks resolve with their own arms', () => {
				const run = start(wide);
				run.set('on', true);
				expect(run.choose('c5')?.tag).toBe('t5');
				expect(run.choose('c0')?.tag).toBe('t0');
				run.set('on', false);
				expect(run.choose('c5')?.tag).toBe('f5');
				expect(run.choose('c5')?.condition).toBeNull();
			});
		});

		describe('choose', () => {
			test('the first true arm wins in source order', () => {
				const run = start(chooser);
				run.set('left', true);
				run.set('right', true);
				const arm = run.choose('facing');
				expect(arm?.tag).toBe('side');
				expect(arm?.annotations).toEqual([
					{ key: 'flip', value: 'true' },
				]);
			});

			test('arms with the same tag but different annotations stay distinct', () => {
				const run = start(chooser);
				run.set('right', true);
				expect(run.choose('facing')?.annotations).toEqual([
					{ key: 'flip', value: 'false' },
				]);
			});

			test('the else arm is taken when no condition matches', () => {
				const arm = start(chooser).choose('facing');
				expect(arm?.tag).toBe('down');
				expect(arm?.condition).toBeNull();
			});

			test('without an else arm, no match returns undefined', () => {
				const run = start(strict);
				expect(run.choose('pick')).toBeUndefined();
				run.set('left', true);
				expect(run.choose('pick')?.tag).toBe('side');
			});

			test('an unknown choice name throws', () => {
				expect(() => start(chooser).choose('nope')).toThrow();
			});
		});

		describe('trace', () => {
			test('records taken edge IDs in caller-owned storage', () => {
				const run = start(priority);
				run.set('go', true);
				const trace = new Uint32Array(4);
				const result = run.step(trace);
				expect(result.taken).toBe(1);
				expect(trace[0]).toBe(0); // a -> b is the first edge declared
			});

			test('zero capacity refuses the first transition without applying it', () => {
				const run = start(priority);
				run.set('go', true);
				const result = run.step(new Uint32Array(0));
				expect(result.error).toBe('output_full');
				expect(result.transitions).toBe(0);
				expect(result.node).toBe('a');
				expect(run.status).toBe('error');
			});

			test('a full trace keeps the transitions already taken', () => {
				const run = start(tail);
				run.finish();
				run.set('go', true);
				const result = run.step(new Uint32Array(2));
				expect(result.error).toBe('output_full');
				expect(result.taken).toBe(2);
				expect(result.transitions).toBe(2);
				expect(result.node).toBe('c');
			});
		});

		describe('names', () => {
			test('an unknown condition name throws', () => {
				expect(() =>
					start(priority).set('nope', true)
				).toThrow();
			});
		});
	});
}

suite('default chunks', await load('../../target/ts-fixtures'));
suite('two-item chunks', await load('../../target/ts-fixtures/chunked'));
