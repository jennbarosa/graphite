# Graphite

Graphite compiles **state graphs** and runs them one step at a time. You describe the
stages and the rules that move between them in a small indentation-based language. Graphite
checks the graph when you build it, and generates TypeScript whose names are typed, so a
misspelled stage or condition fails at compile time instead of at runtime.

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

`graphite doctor` reports whether a JavaScript runtime is available. The runtime is needed
for `run`, `debug`, and TypeScript hosts. [Bun](https://bun.sh) is the default; `npx tsx` also
works. The generated TypeScript needs nothing else.

## Quick start

Create a starter graph, check it, and compile it to TypeScript:

```sh
graphite init hello.dsl
graphite check hello.dsl        # ok: 1 graph
graphite compile hello.dsl      # writes graphite-out/hello/
```

Step through it in the terminal. Each argument is one command:

```sh
graphite debug hello.dsl "set start on" step show finish step show
```

Drive it from TypeScript:

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
| [TypeScript guide](docs/typescript.md) | The generated API and writing a host. |
| [Durable workflows](docs/durable-workflows.md) | Running tasks with checkpoints and retries. |
| [CLI reference](docs/cli.md) | Every command and option. Also `graphite help <command>`. |
| [Rust API](docs/rust.md) | Compiling graphs from Rust code. |
| [FAQ](docs/faq.md) | When to use Graphite, and why it uses TypeScript. |
| [Troubleshooting](docs/troubleshooting.md) | Fixes for the errors you are most likely to see. |

Working examples are in [`examples/`](examples/): `door.dsl` and `walk.dsl` for animation,
`order.dsl` for an order lifecycle, and `workflow/` for a durable task pipeline.

## Development

See [CONTRIBUTING.md](CONTRIBUTING.md) to build, test, and send a change. Security problems go
to [SECURITY.md](SECURITY.md).

## License

Graphite is licensed under the GNU Affero General Public License v3.0. See [LICENSE](LICENSE).
