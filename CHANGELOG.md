# Changelog

All notable changes to Graphite are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses
[Semantic Versioning](https://semver.org/). While the version starts with `0.`, any release may
change the interface.

## [Unreleased]

### Added

- Durable runners can report a detail with an outcome (`{ outcome, detail }`). The host stores it,
  logs it, and passes the most recent failed stage to later tasks as `context.failure`, including
  after a resume.

### Changed

- The state file's snapshot is now `{ run, lastFailure }`. Snapshots written by earlier versions still
  load, but a state file written by this version cannot be resumed by an earlier one.
- `StateStore` records and loads results with their detail, so custom stores must store the result
  object rather than the bare outcome.

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
