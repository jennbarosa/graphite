# Graphite
!! WARNING: Graphite as it is today an early-development **proof of concept** for educational and experimental purposes only, not something you should be using for critical production-level tasks. It is **NOT** production ready. Use at your own risk.

Graphite is a graph compiler and step executor for **state graphs**: stages that
advance when a host sets conditions or reports that a stage has finished. You write
the graph in a small indentation-based DSL. Graphite checks it at build time, emits
TypeScript modules whose names are typed, and runs it one step at a time under a host
that owns time.

The core knows nothing about games, animation, or workflows. A domain gives the
graph its meaning through annotations (`@anim:walk`, `@task:deploy`) and its own
front end or host.

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

## Why

Animation states, NPC behavior, and multi-step jobs are often written as nested
`if`/`switch` code that drifts out of sync with the designers' intent. Typos in
state or condition names only show up at runtime. Graphite moves those failures to
build time:

- **Build-time checks.** Dead ends, unreachable edge sources, duplicate names, and
  unknown flags are compile errors with a source location.
- **Typed hosts.** Generated TypeScript types make `run.set("keyp", true)` and
  `choose("facng")` type errors, so a misspelled name fails `tsc`, not the game.
- **Flat, allocation-free execution.** A graph is compiled into flat arrays. A step
  does not allocate, and the core has no timers, threads, or callbacks, so the same
  graph runs in a game loop, a test, or a server.
- **Resumable tasks.** The durable host runs each stage's task, saves a checkpoint
  after every result, and resumes after a crash without re-running settled tasks.

The performance goals are compiling a million-stage graph in well under a second and
keeping the step path allocation-free. These are targets, not guarantees; see
[Benchmarks](#benchmarks).

## Status

Implemented:

- Parser and structural checks, with source-located errors.
- Rust API: `parse` and `compile`, returning owned, immutable compiled graphs.
- CLI: `check`, `inspect`, `compile`, `debug`, and `run --durable`.
- TypeScript front end and runtime (`src/frontend/typescript/runtime.ts`).
- Durable host (`host/durable.ts`) with a file-backed state store.

Not yet implemented:

- A Rust executor. The step executor exists only in the TypeScript runtime.
- Static checks for condition stalls (a node whose conditions never become true).
- A condition handler for gate cases that source order cannot express. Not designed.
- Generated modules over about 536 million characters. The million-stage benchmark
  output is too large for `tsc` and V8; see [Benchmarks](#benchmarks).

Limitation: the CLI loads its TypeScript host (`host/cli.ts`, `cli/debug.ts`) from
the source tree, through a path fixed at build time. Run the binary from a checkout
of this repository. Installing it elsewhere will not find those files.

## Requirements

- Rust 1.88 or newer (edition 2024).
- [Bun](https://bun.sh) 1.4.4 to run the generated TypeScript (`run --durable`, the
  debugger, the tests). `npx tsx` also works; see `src/bin/graphite/js.rs`.
- TypeScript 5 or later for the type-check test. Set `GRAPHITE_TSC` to the compiler's
  path, or put `tsc` on your `PATH`.

## Quick start

```sh
cargo build --release
./target/release/graphite check examples/door.dsl
./target/release/graphite inspect examples/door.dsl
./target/release/graphite compile examples/door.dsl --out target/door
```

`check` prints `ok: 1 graph` when the file is valid. `compile` writes one index module
per graph (`door.ts`), its chunks (`door.nodes.0.ts`), and the runtime (`graphite.ts`).

## The DSL

A file holds one or more `graph` blocks. Indentation sets the structure; there are
no parentheses.

| Line | Meaning |
| --- | --- |
| `graph NAME` | Starts a graph. |
| `node NAME [flags] [@key:value ...]` | A stage. Flags are `loop`, `end`, and any the schema allows. |
| `edge FROM -> TO when COND` | A **gate**: taken when the host sets `COND` true. |
| `edge FROM -> TO` | A **sequence edge**: taken after the host calls `finish()` for the stage. |
| `choose NAME` | A set of `when COND -> TAG` arms, optionally ending in `else -> TAG`. |

Rules to know when writing graphs:

1. Declare all `node` lines before any `edge` or `choose`.
2. The first node is the entry. A run starts there.
3. A node with no outgoing edge is an error unless it is marked `end`. An `end` node
   may not have outgoing edges.
4. A node has at most one sequence edge. When a gate and the sequence edge are both
   ready, the gate wins, whatever the source order.
5. Gates are checked in source order; the first that holds is taken.
6. Cycles are allowed.
7. Tags, annotation keys, and annotation values are opaque strings. The core never
   branches on them.

A walk cycle with a choice:

```text
graph walk
  node windup   @anim:windup
  node going    loop @anim:walk @group:locomotion
  node winddown end @anim:winddown
  node stopped  end @anim:idle

  edge windup    -> stopped   when keyup
  edge windup    -> going
  edge going     -> winddown  when keyup

  choose facing
    when left  -> side @flip:true
    when right -> side
    when up    -> up
    else       -> down
```

See [`examples/walk.dsl`](examples/walk.dsl).

## Running a graph

Conditions are set by the host and keep their values until the host changes them.
Each step takes every edge it can. A sequence edge needs `finish()` first, which
means "this stage's playback is done".

The debugger runs a graph from the command line. Each argument is one command:

```sh
./target/release/graphite debug examples/door.dsl \
  "set request on" step finish step "set request off" "set timeout on" step show
```

| Command | Effect |
| --- | --- |
| `show` | Current node, status, transition count, and true conditions. |
| `set <cond> on\|off` | Set a condition. |
| `finish` | Mark the current stage finished. |
| `step` | Settle the graph once and print the edges taken. |
| `choose <choice>` | Show the arm the choice selects now. |
| `history` | List every edge taken since the start or the last reset. |
| `reset` | Start a new run; conditions go back to off. |

Without arguments the debugger reads commands from standard input.

## Using the generated TypeScript

Compile a graph, then drive it from TypeScript. The generated `start` and `Run` types
come from the compiled vocabulary, so the node, condition, and choice names are
checked.

```sh
./target/release/graphite compile examples/door.dsl --out target/door
bun run examples/typescript/drive.ts target/door/door.ts 10
```

Each compiled file needs its own `--out` directory. Every output directory gets its
own `graphite.ts` runtime, and the compiler refuses to overwrite one that exists.

A host, with its imports relative to the output directory it was compiled into:

```ts
import { graph } from "./door";
import { start } from "./graphite";

const run = start(graph);
run.set("request", true);

let result = run.step();     // closed -> opening, through the `request` gate
console.log(result.node);    // "opening"

run.finish();                // the opening animation has played
result = run.step();         // opening -> open, through the sequence edge
console.log(result.node);    // "open"

run.set("timeout", true);
run.step();                  // open -> closing through the `timeout` gate
```

Choices return the arm the host should display:

```ts
import { graph } from "./walk";
import { start } from "./graphite";

const run = start(graph);
const arm = run.choose("facing");   // the first arm whose condition holds, or `else`
```

`step()` returns the same result object on every call, updated in place, so steps
do not allocate. Copy the fields if they must outlive the next step.

`step()` also takes an optional trace buffer for recording the edges it takes. See
`src/frontend/typescript/runtime.ts` for the full `Run` interface.

## Durable workflows

`run --durable` executes the first graph in a file as a resumable workflow. A node
with a `@task:NAME` annotation runs that task through a runner you supply. The
runner settles each task with an outcome, and the graph routes on it.

[`examples/workflow/workflow.dsl`](examples/workflow/workflow.dsl):

```text
graph review
  node draft   @task:write
  node check   retry @task:lint
  node fix     @task:fix
  node ship    end @task:deploy

  edge draft -> check
  edge check -> ship when passed @report:ok
  edge check -> fix  when failed
  edge fix   -> check
```

The task names must be allowed, and `retry` is a domain flag, so both go in an
allowlist ([`workflow.toml`](examples/workflow/workflow.toml)):

```toml
node_flags = ["retry"]

[annotations]
task = ["write", "lint", "fix", "deploy"]
```

Run it with a runner ([`runner.ts`](examples/workflow/runner.ts)). The runner's
default export is called with each task name and a context, and returns `"pass"`,
`"fail"`, or `"none"`:

```ts
export default async function (task: string, context: { key: string; attempt: number }) {
  console.error(`running ${task} (${context.key}, attempt ${context.attempt})`);
  return "pass" as const;
}
```

```sh
./target/release/graphite run --durable examples/workflow/workflow.dsl \
  --allow examples/workflow/workflow.toml \
  --state target/review.state.json \
  --runner examples/workflow/runner.ts
```

The run log goes to stderr. The final line is JSON, for example
`{"status":"ended","transitions":2}`. The exit status is 0 when the run ends.

What the host guarantees, and what it does not:

- **Checkpoints.** The state is saved after every result, in `<state>` and the results
  in `<state>.results`. Running the same command again resumes where it stopped.
- **Outcomes.** `pass` and `fail` set the `passed` and `failed` conditions when the
  graph declares both. `none` sets neither. A node without a task is settled as `none`.
- **Retries.** Nodes flagged `retry` get three attempts when a task throws. A task that
  still fails stops the run with an error naming its key.
- **At least once.** A task that was running when the process died runs again on
  resume, with the same `key`. Tasks with external effects must dedupe on `key`.
- **Timeouts.** A task that exceeds `taskTimeoutMs` (default 60 s) counts as a failed
  attempt. The host cannot stop a runner that ignores its `AbortSignal`.

> **Security.** `--runner` runs the file as code with your user's permissions. Use
> only runners you wrote or have reviewed.

The TypeScript host API (`runDurable`, `fileStore`, `runLog`) is documented in
`host/durable.ts` and `host/run-log.ts`.

## Rust API

```rust
use graphite::parser::{parse, Capacity, Schema};

let capacity = Capacity {
    nodes: 1,
    annotations: 1,
    strings: 4,
    string_bytes: "walkstoppedanimidle".len(),
    ..Capacity::default()
};
let graphs = parse(
    "graph walk\n  node stopped end @anim:idle",
    &Schema::default(),
    &[capacity],
)?;
let graph = &graphs[0];
assert_eq!(graph.strings.resolve(graph.nodes[0].name), Some("stopped"));
# Ok::<(), graphite::parser::ParseError>(())
```

`parse(source, schema, capacities)` takes one `Capacity` hint per graph, in source
order. The hints size the arrays and string storage up front. If a hint is too small,
parsing fails with an error and does not grow the buffers. `Schema::node_flags` adds
domain flags; `loop` and `end` are always allowed.

Compiled data is flat: nodes, edges, choices, arms, flags, and annotations are arrays
indexed by ID, and each node's outgoing edges are a range in one array. Treat the
fields as read-only. Changing them directly breaks the guarantees parsing established.

To emit TypeScript from Rust, use `pipeline::compile` and `pipeline::write_modules`
(see `src/pipeline.rs`), or the `compile` CLI command.

## Testing

Run from this directory:

```sh
cargo test                                 # unit, integration, and doc tests
cargo test --no-default-features         # without the SIMD lexer
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

Several tests run TypeScript with Bun, and the type-check test runs `tsc`. They fail
with a clear message when those tools are missing.

The parser fixtures in `tests/fixtures/parser/` are hand-written `.expected` decodings
of each `.dsl` file. They are not generated by the parser, so they check the output
independently. To inspect one case:

```sh
cargo test --test parser_fixtures walk -- --nocapture
```

## Benchmarks

```sh
cargo bench --bench compiler_storage     # arena and interner
cargo bench --bench parser               # full DSL compilation
cargo bench --bench lexer                # tokenizer
cargo bench --bench frontend             # TypeScript emission
cargo test --test allocations            # allocation-count assertions
```

Set `GRAPHITE_BENCH_SIZE=small|medium|huge` (default `small`) and
`GRAPHITE_BENCH_SEED` (default 42). Inputs are generated from the seed, so a run can
be reproduced. Do not run `huge` on a busy machine.

This repository does not include measured results. Run the benchmarks on the
machine you care about. The Huge TypeScript module is too large for `tsc`, as noted above.

## Layout

```text
src/
  lexer.rs, parser.rs      one-pass tokenizer and parser into compiled arrays
  graph.rs, arena.rs,      compiled graph data, arena, and string interner
  interner.rs
  frontend/                output front ends: typescript/ (index, chunks, runtime.ts)
  pipeline.rs              compile and write_modules
  bin/graphite/            the CLI
host/                      durable host, run log, and the run CLI (TypeScript)
cli/                       debugger session (TypeScript)
examples/                  DSL files, a TypeScript driver, and a durable workflow
tests/                     Rust integration tests, fixtures, and Bun tests
benches/                   Criterion benchmarks
```

## License

Graphite is licensed under the GNU Affero General Public License v3.0 (`LICENSE`).
