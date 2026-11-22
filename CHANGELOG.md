# Changelog

All notable changes to Graphite are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses
[Semantic Versioning](https://semver.org/). While the version starts with `0.`, any release may
change the interface.

## [Unreleased]

### Added

- A `@timeout:MS` node annotation sets a per-attempt time limit, in milliseconds, for that node's task.
- `Node::timeout` in the Rust front end's runtime reads a node's `@timeout:MS` annotation with the
  TypeScript engine's rule for valid values. The Rust front end has no durable runner, so it does not
  enforce the limit.

- `graphite run --retry <key>` clears a failed task's stored error, so the run runs that task again.

### Fixed

- Fan-out (`run --each`) no longer leaves an item unfinished after a crash. A seen item is resumed from its
  own state, and `--until-passed` counts the items that passed in earlier runs.
- The results file drops the records of finished stages, so it no longer grows with a long run.

### Changed

- The Criterion benchmarks, the allocation-count benches and the experimental parallel lexer are removed.
- The SIMD lexer and its `lexer-simd` feature are removed. The scalar scan is the only path. On the
  medium lexer benchmark the SIMD scan ran in 8.03 ms and the scalar scan in 8.19 ms, about 2% apart.
- **Breaking:** a file is pure only when its front end is pure: `@!frontend:purescript`, a `.purs`
  name, or `--frontend purescript`. The `--pure` flag, the `@!pure` directive, `@!frontend:pure` and
  a `.pure` name no longer make a file pure. Rename `x.pure.graphite` to `x.purs.graphite`.
- **Breaking (library):** the `graphite::arena` module is removed. Nothing in the library used it.
- **Breaking (library):** `graphite::parser::parse` takes `(source, schema)`. The per-graph `Capacity`
  hints, `graphite::pipeline::block_capacities` and `Interner::with_capacity` are removed, and so is
  the `graphite::stage` module. The string table and the graph arrays grow as the source is read, so
  no caller sizes them. A string table that would pass the 4 GiB offset limit reports
  "a graph's strings exceed the 4 GiB offset limit", in place of the old capacity messages.
- The lexer, grammar, structure checks and `@!` directive reader are plain Rust. The `.graphite` rule
  files under `src/graphs`, their generated data under `src/generated`, and `examples/bootstrap_dump.rs`
  are removed, and so is the CI and CONTRIBUTING step that regenerated them. Nothing in the build reads
  them now.
- `pipeline::write_modules` renders every output in memory before it creates the output directory. A
  failed render writes nothing and leaves no empty directory, where it used to leave an empty one.
- **Breaking:** a dotted file name now picks its front end from the infix. `door.rs.graphite`
  builds Rust, where it used to build TypeScript, with no warning. Pass `--frontend typescript` or add
  `@!frontend:typescript` to keep the old output. A name such as `my.walk.graphite` with no known
  infix is now an error for `compile` and `check` unless a flag or directive names the front end.
  Other commands ignore the infix. See the
  [CLI reference](docs/cli.md#choosing-the-front-end-and-mode).
- `check` and `compile` pick the front end from the file's name and directives when `--frontend` is
  not given. The default is still `typescript` for machine files.
- The allowlist's `[annotations]` check now covers enum, member, input and choose lines.
- **Breaking (library):** `Frontend` gains a required `extension` method. `Schema` gains a `pure`
  field, so a struct literal needs `..Default::default()`. `CompiledGraph`, `Choice` and the pure
  data types gain fields. `pipeline::build::Request` takes a `select` callback in place of
  `front_end` and `pure`, and the file is read once.
- `check` now renders every output, so it reports the same name clashes as `compile`. A clash
  found while rendering, such as two machine nodes that become one Rust constant, is now reported
  with the line of its graph, and it is refused before anything is written.
- A pure name that PureScript cannot use, or that clashes with a name the module imports (such as
  `Nothing` or `otherwise`), is refused with the line of the declaration that causes it.
- Fan-out seen keys are resumed, not skipped, on later runs: a finished item returns its stored result
  without running its tasks again.
- The `--durable` flag is removed. It did nothing since `run` became durable, and passing it is now a usage error.
- `graphite inspect` prints the annotations on the `graph` line, in text and in `--json`.
- Durable tasks no longer have a 60-second default limit. Without a `timeout` annotation or a
  `taskTimeoutMs` option, a task runs until it finishes.

- Graph files now end in `.graphite` instead of `.dsl`. The examples, fixtures and bundled graph
  descriptions use the new extension. The language itself is unchanged.

### Added

- Pure files: `enum`, `input`, `choose NAME : ENUM` and `when` tests on declared inputs, with the
  `purescript` front end writing a PureScript module. A pure file is a decision table, so it has
  no states and cannot be run. See [pure files](docs/dsl.md#pure-files) and
  [PureScript](docs/purescript.md).
- Directives at the top of a file: `@!pure` and `@!frontend:KEY`.
- `--pure` for `check`, `compile` and `inspect`. `inspect` reports a pure graph's kind, enums and
  inputs.
- Annotations on the `graph` line (`graph walk @choice:facing`), read with `graph.annotations()` in TypeScript
  and `GRAPH.annotations` in Rust.
- `default @key:value` lines, before the first node, add annotations to every node that does not set the key.
- `nodeLooping(name)` and `nodeEnd(name)` on the TypeScript graph, so a host need not repeat `loop` as an annotation.
- Neovim syntax highlighting and file-type detection for `*.graphite` files, in `editors/nvim`.

- With `--tasks`, the manifest's `[tasks]` keys are the allowed task names, so the allowlist can list only
  node flags. The allowlist's `[annotations]` table is optional. An allowlist with a `[tasks]` section is
  refused: commands belong in the manifest.
- `graphite debug --simulate <task>=<outcomes>` runs the durable engine over a graph with each task's
  outcome stubbed (`pass`, `fail`, `none` or `throw`), so its routes can be checked before any task runs.
  It reports each stage and where a run stalls, and stops a loop after 10,000 transitions.
- Fan-out: `graphite run list.graphite --each item.graphite --key domain` runs the item graph once per item the list graph
  produces, with a seen file to skip items already started and `--until-passed N` to stop after N pass.
- Stage outputs: a task returns `output`, and later tasks read it with `context.outputs.read(node)`. The end node's
  name, its stage's `outcome`, and its output appear in the run result.
- Error routing: a graph that declares `errored` routes a task that throws, instead of stopping the run.
- Command-backed tasks: `graphite run --tasks tasks.toml` runs each task as a command. The command gets
  the task context as JSON on stdin and reports its outcome as JSON on stdout.
- `graphite run --input data.json` gives every task the JSON value as `context.input`. A resumed run
  with a different input is refused.
- Durable runners can report a detail with an outcome (`{ outcome, detail }`). The engine stores it,
  logs it, and passes the most recent failed stage to later tasks as `context.failure`, including
  after a resume.

### Changed

- The state file's snapshot is now `{ run, lastFailure }`. Snapshots written by earlier versions still
  load, but a state file written by this version cannot be resumed by an earlier one.
- `StateStore` records and loads results with their detail, so custom stores must store the result
  object rather than the bare outcome.
- The durable runner moved from `host/` to `src/frontend/typescript/durable/`, and `host/durable.ts` is
  now `engine.ts`. Its `HostEvent` type is `RunEvent`. Code that imported these from the old paths must
  update its imports.

### Fixed

- A seen file whose last line was cut short by a crash could make the next key join that line, so an
  item could be started again. The torn line is now cut before the next key is appended.
- Fan-out with the same `--seen` file and a new `--state` no longer runs the tasks of items an earlier state
  started. Each state records the items it started, and a key without a record there is skipped.
  An item started before this version has no record, so an interrupted one is skipped, not resumed. Finish it by hand.
- `--allow` checks the annotations on the `graph` line, which it skipped before.
- The state lock is written whole, so a lock with no readable pid is refused rather than taken over.
- `--retry` refuses a stage the run has already passed, such as a task whose error an `errored` edge routed,
  instead of reporting a retry that never runs.

## [0.1.0] - 2026-11-12

The first release. Graphite is an early proof of concept, so expect the interface to change.

### Added

- The graph language: `graph`, `node` (with `loop`, `end` and allowed flags), `edge` (gates with
  `when`, sequence edges, and chains with `else` fallbacks), `choose` with `else`, and `@key:value`
  annotations. Checks run at build time, with source-located errors.
- Allowlists (`--allow`) for domain flags and annotation values.
- A Rust API: `pipeline::compile`, `pipeline::write_modules` and `check_outputs`, with the
  `Frontend` trait for new output languages.
- Front ends. The TypeScript front end generates typed modules and a runtime. It is the default and
  the only one shipped.
- A durable host (`graphite run`) that runs tasks through a runner, with checkpoints, retries and
  resumption.
- A debugger (`graphite debug`) that steps a graph from the command line or from standard input.
- The `graphite` command: `init`, `check`, `compile`, `inspect`, `debug`, `run`, `doctor`, `help`,
  `completions` and `man`, with `--version` and `--json` output for `check`, `inspect` and `doctor`.
- Exit status 0 for success, 1 for a failed command, and 2 for a usage error.
- The TypeScript host and debugger are compiled into the binary, so an installed `graphite` does
  not need the source tree.
- `graphite doctor` reports the version, each front end, and whether `debug` and `run` can work on
  this machine.
- Documentation in `docs/`: a tutorial, a glossary, the DSL, the CLI, the front ends, durable
  workflows, the Rust API, the FAQ and troubleshooting.
