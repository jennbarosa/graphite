// The checks on a generated graph that the durable entry points share.
import { describe, expect, test } from 'bun:test';
import type { GraphLike } from '../../src/frontend/typescript/durable/contract';
import {
	declares,
	outcomeConditionsOf,
	taskNames,
	taskOf,
} from '../../src/frontend/typescript/durable/graph-checks';

/** A graph of nodes, each with an optional task, and the named conditions. */
function graphWith(
	nodes: readonly [name: string, task?: string][],
	conditions: readonly string[]
): GraphLike {
	return {
		data: { nodeCount: nodes.length },
		nodeName: (id) => nodes[id]?.[0] ?? '',
		nodeAnnotations: (node) => {
			const task = nodes.find(([name]) => name === node)?.[1];
			return task === undefined ? [] : [{ key: 'task', value: task }];
		},
		nodeFlags: () => [],
		conditionId: (name) => {
			if (!conditions.includes(name)) throw new Error(`no condition ${name}`);
			return conditions.indexOf(name);
		},
	};
}

describe('taskOf', () => {
	test('returns the annotation value on a node, or undefined when it has none', () => {
		const graph = graphWith([['draft', 'write'], ['end']], []);
		expect(taskOf(graph, 'draft', 'task')).toBe('write');
		expect(taskOf(graph, 'end', 'task')).toBeUndefined();
		expect(taskOf(graph, 'draft', 'other')).toBeUndefined();
	});
});

describe('taskNames', () => {
	test('lists each node that has a task, with its label', () => {
		const graph = graphWith([['draft', 'write'], ['end'], ['check', 'lint']], []);
		expect(taskNames(graph, 'task')).toEqual([
			{ name: 'write', label: 'draft (write)' },
			{ name: 'lint', label: 'check (lint)' },
		]);
	});
});

describe('outcomeConditionsOf', () => {
	test('needs both passed and failed', () => {
		expect(outcomeConditionsOf(graphWith([], ['passed']))).toBeUndefined();
		expect(outcomeConditionsOf(graphWith([], ['failed']))).toBeUndefined();
	});

	test('names pass and fail, and adds errored when the graph declares it', () => {
		expect(outcomeConditionsOf(graphWith([], ['passed', 'failed']))).toEqual({
			pass: 'passed',
			fail: 'failed',
		});
		expect(outcomeConditionsOf(graphWith([], ['passed', 'failed', 'errored']))).toEqual({
			pass: 'passed',
			fail: 'failed',
			error: 'errored',
		});
	});
});

describe('declares', () => {
	test('is true only for a condition the graph has', () => {
		const graph = graphWith([], ['passed']);
		expect(declares(graph, 'passed')).toBe(true);
		expect(declares(graph, 'failed')).toBe(false);
	});
});
