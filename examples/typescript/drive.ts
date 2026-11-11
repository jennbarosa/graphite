import { dirname, join, resolve } from 'node:path';

// Host driver for a generated Graphite graph. It uses only the public API:
//   bun run drive.ts <path/to/<graph>.ts> [steps]
// It sets every condition on, then drives the graph: one finish per step, printing each
// transition, and it resolves the first choice it finds.

const modulePath = process.argv[2];
if (!modulePath) {
	console.error('usage: bun run drive.ts <path/to/graph.ts> [steps]');
	process.exit(1);
}
const steps = Number(process.argv[3] ?? 20);

const { graph } = await import(resolve(modulePath));
// The runtime sits next to every generated module, so load it from the graph's folder.
const { start } = await import(
	join(dirname(resolve(modulePath)), 'graphite.ts')
);
const run = start(graph);
const conditions: string[] = graph.data.conditions.map(
	(_: number, id: number) => graph.conditionName(id)
);

console.log(
	`graph ${graph.name}: ${graph.data.nodeCount} nodes, ${conditions.length} conditions`
);
for (const name of conditions) run.set(name, true);

const firstChoice = graph.data.choiceChunks[0]?.choices[0];
if (firstChoice) {
	const choice = graph.text(firstChoice.name);
	console.log(`choose(${choice}) ->`, run.choose(choice));
}

for (let i = 0; i < steps && run.status === 'running'; i++) {
	run.finish();
	const result = run.step();
	console.log(
		`step ${i + 1}: node=${result.node} taken=${result.taken} status=${result.status}`
	);
}
console.log(`final: node=${run.node} status=${run.status} error=${run.error}`);
