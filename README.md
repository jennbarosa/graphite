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

You need [Rust](https://rustup.rs) 1.88 or newer. From a checkout of this repository:

```sh
cargo install --path .
graphite doctor
```

Shell completions and a manual page come from the binary too: `graphite completions bash`
and `graphite man`. See the [CLI reference](docs/cli.md#shell-completions-and-the-manual-page).

`graphite doctor` reports this build's version, each front end, and whether `debug` and `run` can
work here. The TypeScript front end needs a JavaScript runtime for `run` and `debug`. [Bun](https://bun.sh) is the default; `npx tsx` also
works. Compiling needs nothing else.

## Quick start

Create a starter graph, check it, and compile it. The TypeScript front end is the one Graphite
ships today:

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
| [Glossary](docs/glossary.md) | What *gate*, *stage*, *host*, *finish* and the other terms mean. |
| [DSL reference](docs/dsl.md) | Every line type, the rules, and chains. |
| [Front ends](docs/front-ends.md) | How graphs become code in each language, and how to add one. |
| [TypeScript front end](docs/typescript.md) | The generated TypeScript API and writing a host. |
| [Durable workflows](docs/durable-workflows.md) | Running tasks with checkpoints and retries. |
| [CLI reference](docs/cli.md) | Every command and option. Also `graphite help <command>`. |
| [Rust API](docs/rust.md) | Compiling graphs from Rust code. |
| [FAQ](docs/faq.md) | When to use Graphite, and why TypeScript comes first. |
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

## What Graphite catches before a run

Graphite checks a graph when you compile it, so routing mistakes fail the build instead of
a run. These are the checks you get without writing any test:

- **Stalls and endless loops at build time, in durable graphs.** A durable graph is one whose
  tasks route only on `passed` and `failed`, and whose other nodes are all `end` nodes. In it,
  every node reachable from the entry must be able to reach an `end`. A loop with no way out is
  rejected with the node that can never finish:

  ```text
  error: node can never reach an end, so a run that gets here never finishes
    --> line 3, column 8
    |
  3 |   node b @task:b
  ```

- **Missing outcomes at build time.** In a durable graph, each task needs an edge for every
  outcome the graph uses (`passed`, `failed`, `errored`), so no task can finish without a route.
- **Unreachable nodes at build time.** A node that no edge leads to is an error at its declaration.
- **Stalls in simulation.** `graphite debug --simulate` runs a graph with stubbed task outcomes,
  and names the node a run gets stuck at. A loop that never ends stops at 10,000 transitions:

  ```text
  stalled at stuck: no edge from it is taken
  ```

See [Durable workflows](docs/durable-workflows.md#checking-routes-without-running-tasks) for
`--simulate`, and the [DSL reference](docs/dsl.md) for the rules.

## Development

See [CONTRIBUTING.md](CONTRIBUTING.md) to build, test, and send a change. Security problems go
to [SECURITY.md](SECURITY.md).

## License

Graphite is licensed under the GNU Affero General Public License v3.0. See [LICENSE](LICENSE).
