// Checks on a generated graph that the durable entry points share: which node has a task, which
// nodes have tasks, and the outcome conditions those tasks need.
import type { GraphLike } from './contract';

/** The value of `annotation` on `node`, or undefined when the node has none. */
export function taskOf(
	graph: GraphLike,
	node: string,
	annotation: string
): string | undefined {
	return graph
		.nodeAnnotations(node)
		.find((entry) => entry.key === annotation)?.value;
}

/** Each task's node and name, for the nodes that have one. */
export function taskNames(
	graph: GraphLike,
	annotation: string
): { name: string; label: string }[] {
	const found: { name: string; label: string }[] = [];
	for (let id = 0; id < graph.data.nodeCount; id++) {
		const node = graph.nodeName(id);
		const name = taskOf(graph, node, annotation);
		if (name !== undefined) found.push({ name, label: `${node} (${name})` });
	}
	return found;
}

/** The outcome conditions a graph needs: pass and fail, and errored when declared. */
export function outcomeConditionsOf(
	graph: GraphLike
): { pass: string; fail: string; error?: string } | undefined {
	if (!declares(graph, 'passed') || !declares(graph, 'failed')) return undefined;
	return {
		pass: 'passed',
		fail: 'failed',
		...(declares(graph, 'errored') ? { error: 'errored' } : {}),
	};
}

/** Whether the graph has a condition named `name`. */
export function declares(graph: GraphLike, name: string): boolean {
	try {
		graph.conditionId(name);
		return true;
	} catch {
		return false;
	}
}
