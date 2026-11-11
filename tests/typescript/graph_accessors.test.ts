// Drives the host accessors against the workflow fixture compiled by `tests/runtime_accessors.rs`.
import { describe, expect, test } from 'bun:test';
import { dirname, join } from 'node:path';

const modulePath = process.env.GRAPHITE_WORKFLOW;
if (modulePath === undefined) {
	throw new Error(
		'GRAPHITE_WORKFLOW must name the compiled workflow module'
	);
}
const { graph } = await import(modulePath);
const { start } = await import(join(dirname(modulePath), 'graphite.ts'));

describe('node accessors', () => {
	test('annotations come back in source order', () => {
		expect(graph.nodeAnnotations('check')).toEqual([
			{ key: 'task', value: 'lint' },
		]);
		expect(graph.nodeAnnotations('draft')).toEqual([
			{ key: 'task', value: 'write' },
		]);
	});

	test('domain flags are listed; a node without any gets an empty list', () => {
		expect(graph.nodeFlags('check')).toEqual(['retry']);
		expect(graph.nodeFlags('draft')).toEqual([]);
	});

	test('a node name maps to a stable ID', () => {
		expect(graph.nodeId('draft')).toBe(0);
		expect(graph.nodeId('check')).toBe(1);
	});

	test('an unknown node name throws', () => {
		expect(() => graph.nodeId('nosuch')).toThrow(RangeError);
		expect(() => graph.nodeAnnotations('nosuch')).toThrow(RangeError);
	});
});

describe('edge accessor', () => {
	test('reports the transition a step took, with its annotations', () => {
		const run = start(graph);
		const trace = new Uint32Array(8);
		run.finish();
		const first = run.step(trace);
		expect(first.taken).toBe(1);
		expect(graph.edge(trace[0])).toEqual({
			id: trace[0],
			source: 'draft',
			target: 'check',
			condition: null,
			annotations: [],
		});
	});

	test('a gated edge names its condition and carries its annotations', () => {
		const run = start(graph);
		const trace = new Uint32Array(8);
		run.finish();
		run.step(trace);
		run.set('passed', true);
		const passed = run.step(trace);
		expect(passed.node).toBe('ship');
		expect(graph.edge(trace[0])).toEqual({
			id: trace[0],
			source: 'check',
			target: 'ship',
			condition: 'passed',
			annotations: [{ key: 'report', value: 'ok' }],
		});
	});

	test('an unknown edge ID throws', () => {
		expect(() => graph.edge(9999)).toThrow(RangeError);
	});
});
