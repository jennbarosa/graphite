<p align="left">
  <img src="docs/assets/graphite.png" alt="Graphite" width="480">
</p>

Graphite compiles **state graphs** and runs them one step at a time. You describe the
stages and the rules that move between them in a small indentation-based language. Graphite
checks the graph when you build it, and can generate compiled graphs in any supported language.

It is written for animation states, NPC behavior, and multi-step jobs. The core knows nothing
about games or workflows; your own names and annotations give a graph its meaning.

> **Status:** early development. Use it for experiments, not for critical production work.
> See [ROADMAP.md](ROADMAP.md) for what is missing.

```text
graph door
  node closed  @anim:idle
  node opening @anim:open
  node open    @anim:idle_open
  node closing @anim:close

  edge closed  -> opening when request
  edge opening -> open
  edge open    -> closing when timeout
  edge closing -> closed  @sfx:latch
```

## Install

You need [Rust](https://rustup.rs) 1.88 or newer. Nothing else is required to check and compile graphs:

```sh
git clone https://github.com/jennbarosa/graphite && cd graphite
cargo install --path .
```

`graphite run` and `graphite debug` also need a JavaScript runtime, [Bun](https://bun.sh) or
`npx tsx`. `graphite doctor` says what works on your machine and what is missing.

## Quick start

Create a starter graph, check it, and compile it. This uses the TypeScript front end, the default:

```sh
graphite init hello.graphite
graphite check hello.graphite        # ok: 1 graph
graphite compile hello.graphite      # writes graphite-out/hello/
```

Step through it in the terminal. Each argument is one command:

```sh
graphite debug hello.graphite "set start on" step show finish step show
```

Drive it from a TypeScript host:

```ts
import { graph } from "./graphite-out/hello/hello";
import { start } from "./graphite-out/hello/graphite";

const run = start(graph);
run.set("start", true);      // a condition: the host turns it on
run.step();                  // idle -> work, through the `start` gate
run.finish();                // the work stage is done
console.log(run.step().node); // "done"
```

The [tutorial](docs/tutorial.md) builds a real graph from scratch and explains each step.

## Documentation

| Read this | To learn |
| --- | --- |
| [Tutorial](docs/tutorial.md) | Build a graph, check it, compile it, and drive it. |
| [DSL reference](docs/dsl.md) | Every line type, the rules, and chains. |
| [Front ends](docs/front-ends.md) | How graphs become code in each language, and how to add one. |
| [TypeScript front end](docs/typescript.md) | The generated TypeScript API and writing a host. |
| [PureScript front end](docs/purescript.md) | Pure decision tables, written as PureScript modules. |
| [Durable workflows](docs/durable-workflows.md) | Running tasks with checkpoints and retries. |
| [CLI reference](docs/cli.md) | Every command and option. Also `graphite help <command>`. |
| [Rust API](docs/rust.md) | Compiling graphs from Rust code. |
| [FAQ](docs/faq.md) | When to use Graphite, why TypeScript comes first, and what *gate*, *host* and the other terms mean. |
| [Troubleshooting](docs/troubleshooting.md) | Fixes for the errors you are most likely to see. |

### Annotations

Annotations are `@key:value` pairs that Graphite passes to your host unchanged. They can sit on
the `graph` line, on nodes and edges, and on choice arms. A `default` line sets annotations for
every node at once.

```text
graph walk @choice:facing @idle:stopped
  default @fps:8
  node going    loop @anim:walk
  node winddown end  @anim:winddown @fps:12
```

- **Graph annotations** describe the graph as a whole. Read them with `graph.annotations()` in
  TypeScript or `GRAPH.annotations` in Rust.
- **Defaults** come before the first node. Each node gets them after its own annotations, unless
  it sets the same key. Above, `going` has `@anim:walk @fps:8`, and `winddown` keeps its own
  `@fps:12`. They are copied at compile time, so `nodeAnnotations` returns them like any other.
- **Flags** are not annotations. Read a node's `loop` and `end` with `nodeLooping(name)` and
  `nodeEnd(name)`, so you need no `@loop` annotation.

Working examples are in [`examples/`](examples/): `door.graphite` and `walk.graphite` for animation,
`order.graphite` for an order lifecycle, and `workflow/` for a durable task pipeline.

## Pure decision tables

A graph that only chooses (no states, no runs) is a pure file. It declares its inputs and enums,
and each choice names the arm it picks:

```graphite
@!frontend:purescript
graph combat
  enum Tactic flee hit hold
  input lowHealth : bool
  choose tactic : Tactic
    when lowHealth -> flee
    else           -> hold
```

`graphite compile combat.graphite` writes `Combat.purs`, a PureScript module with a plain function
`tactic :: Input -> Tactic`. The [DSL reference](docs/dsl.md#pure-files) lists the lines it allows.

## What Graphite catches before a run

Graphite checks a graph when you compile it, so a routing mistake fails the build instead of a run:
an unreachable node, a node with no exit, and, in a durable graph, a task with no route for an
outcome or a loop that can never reach an `end`. `graphite debug --simulate` runs a graph with
stubbed task outcomes and names the node a run gets stuck at. See
[durable workflows](docs/durable-workflows.md#checking-routes-without-running-tasks) and the
[DSL reference](docs/dsl.md).

## Development

See [CONTRIBUTING.md](CONTRIBUTING.md) to build, test, and send a change. Security problems go
to [SECURITY.md](SECURITY.md).

## License

Graphite is licensed under the GNU Affero General Public License v3.0. See [LICENSE](LICENSE).
