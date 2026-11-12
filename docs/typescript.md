# TypeScript front end

The TypeScript front end is the one Graphite ships. `graphite compile` uses it by default, writing
TypeScript modules for each graph. This page describes what they contain and how a host drives
them. For the general idea of a front end, see [front ends](front-ends.md).

## What gets generated

For `door.graphite`, `graphite compile door.graphite` writes into `graphite-out/door/`:

| File | Contents |
| --- | --- |
| `door.ts` | The graph's index. Import `graph` from here. |
| `door.nodes.N.ts` | Node data, in chunks of 4096 nodes. |
| `door.choices.N.ts` | Choice data, when the graph has choices. |
| `graphite.ts` | The runtime. Each output directory gets its own copy. |

The generated `start` function and `Run` type use the graph's own names. Node names, condition
names and choice names are checked by the compiler, so a misspelled name is a type error.

## Driving a graph

```ts
import { graph } from "./graphite-out/door/door";
import { start } from "./graphite-out/door/graphite";

const run = start(graph);
run.set("request", true);     // turn a condition on
const result = run.step();    // settle the graph once
console.log(result.node);     // "opening"

run.finish();                 // the current stage's work is done
run.step();                   // a sequence edge can now be taken
```

| Method | Use it to |
| --- | --- |
| `set(name, value)` | Set a condition. It keeps its value until you change it. |
| `step()` | Settle the graph once. Every edge that can be taken is taken, in order. |
| `finish()` | Report that the current stage's work is done. Sequence edges need it. |
| `choose(name)` | Get the arm that holds for a choice, or the `else` arm. |

`step()` returns the same result object on every call, updated in place, so steps do not
allocate. Copy the fields you need before the next step if you must keep them.

`choose()` returns an object with `tag` (the value to show, such as `"side"`), `condition` (the
condition that held, or `null` for `else`) and `annotations` (the arm's annotations). It returns
`undefined` when no arm holds and the choice has no `else`.

```ts
const arm = run.choose("facing");
console.log(arm.tag);         // "side", "up", or "down"
```

## Passing a trace

`step()` takes an optional trace buffer that records the edges it takes. Pass one when you want to
log transitions without reading the node after every step. See
[`src/frontend/typescript/runtime.ts`](../src/frontend/typescript/runtime.ts) for the full `Run`
interface.

## A host that runs to the end

[`examples/typescript/drive.ts`](../examples/typescript/drive.ts) sets every condition, then steps the
graph and prints each transition. It uses only the public API. Run it on a compiled module:

```sh
bun run examples/typescript/drive.ts graphite-out/door/door.ts 10
```

## Importing the runtime

Generated modules import `./graphite`, relative to their own directory. Keep the output directory
together. Moving `door.ts` without `graphite.ts` breaks the import.

For a host that loads a module by a path computed at runtime, load `graphite.ts` from the same
directory as the graph, as `drive.ts` does.
