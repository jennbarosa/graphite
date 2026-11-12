# Tutorial: a door that opens and closes

In this tutorial you build a small graph from nothing, check it, compile it, and drive it from
TypeScript. It takes about ten minutes. You need the `graphite` command
([install](../README.md#install)). The TypeScript part needs [Bun](https://bun.sh).

## 1. Describe the states

A graph is a list of **stages** (`node`). Create `door.graphite`:

```text
graph door
  node closed  @anim:idle
  node opening @anim:open
  node open    @anim:idle_open
  node closing @anim:close
```

The text after each name is an annotation (`@anim:idle`). Graphite does not interpret it. Your
game uses it to pick an animation.

The first node is where a run starts, so the door starts `closed`.

## 2. Add the rules

Rules are **edges**. Declare every `node` before any `edge`.

```text
  edge closed  -> opening when request
  edge opening -> open
  edge open    -> closing when timeout
  edge closing -> closed
```

There are two kinds of edge:

- `when request` makes a **gate**. The edge is taken when the host sets the `request` condition
  to true.
- No `when` makes a **sequence edge**. It is taken after the host calls `finish()` for the stage
  it leaves. Here, `opening -> open` happens once the opening animation has played.

## 3. Check it

```sh
graphite check door.graphite
```

```text
ok: 1 graph
```

Try breaking the file to see a located error. Rename `opening` to `openning` in one edge and run
`check` again. Graphite points at the line and column with `unknown node; declare it with `node`
before linking to it`.

Build-time checks catch dead ends, unreachable stages, duplicate names, and unknown flags. Each
one is reported with the place that caused it.

## 4. Step it in the terminal

The debugger runs a graph without any code. Each argument is one command:

```sh
graphite debug door.graphite "set request on" step finish step show
```

- `set request on` sets the condition.
- `step` settles the graph once and reports the edges taken.
- `finish` says the current stage's work is done.
- `show` prints the current stage.

The debugger prints the result of each command. After the first `step` the door is `opening`.
After `finish` and the next `step` it is `open`, and the last `show` confirms that.

## 5. Compile

```sh
graphite compile door.graphite
```

The TypeScript front end is the default, so this writes `graphite-out/door/`:

- `door.ts` describes the graph. Its node and condition names are types.
- `door.nodes.0.ts` holds the node data.
- `graphite.ts` is the runtime. Every compiled graph gets its own copy.

Compiling to the same directory again is refused, so the output is never overwritten by accident.
Remove the directory, or pass `--out`, to compile again.

## 6. Drive it from TypeScript

Create `main.ts` next to `graphite-out/`:

```ts
import { graph } from "./graphite-out/door/door";
import { start } from "./graphite-out/door/graphite";

const run = start(graph);

run.set("request", true);
console.log(run.step().node);   // "opening"

run.finish();                   // the opening animation has played
console.log(run.step().node);   // "open"

run.set("timeout", true);
console.log(run.step().node);   // "closing"
```

Run it with Bun:

```sh
bun run main.ts
```

Notice what the compiler did for you. `run.set("request", true)` is checked. Misspell
`"request"` and `tsc` reports the error before the game runs. See the
[TypeScript guide](typescript.md) for the full API.

## 7. Add a choice

A **choice** picks a value from the state the host reports, such as a facing direction:

```text
  choose facing
    when left  -> side @flip:true
    when right -> side
    when up    -> up
    else       -> down
```

In TypeScript, `run.choose("facing").tag` is the value of the arm that holds: `"side"`, `"up"`, or
`"down"`. Add the `choose` block at the end of the file and compile to a new directory with
`--out`, because the earlier output is still there.

## Where next

- [DSL reference](dsl.md): every line type, and the rules Graphite enforces.
- [Durable workflows](durable-workflows.md): run a graph where each stage is a task with
  checkpoints.
- [Examples](../examples/): a walk cycle, a deploy pipeline, and an order lifecycle.
