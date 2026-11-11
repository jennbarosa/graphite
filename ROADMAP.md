# Roadmap

What Graphite does not do yet, and known limits. Items are roughly in priority order.

## Missing

- **A Rust executor.** The step executor exists only in the TypeScript runtime. Rust code can
  parse, check and generate code through a front end, but cannot step a graph itself.
- **Static checks for condition stalls.** Nothing yet warns when a node's conditions can never
  become true.
- **A condition handler for gate cases source order cannot express.** Not designed.
- **Very large graphs in TypeScript.** Generated modules over about 536 million characters are too
  large for `tsc` and V8. The million-stage benchmark output is one such case. See the benchmarks
  section of [CONTRIBUTING.md](CONTRIBUTING.md).

## Known limits

- **Durable takeover is not atomic.** Two operators recovering the same run at the same moment can
  race. See the note in [`host/durable.ts`](host/durable.ts).
- **The index holds every node name in one union.** TypeScript gives up on very large unions, so a
  huge graph may need its vocabulary split per chunk. See
  [`src/frontend/typescript/index.rs`](src/frontend/typescript/index.rs).
- **One top-level block runs serially in the parallel benchmark.** Parallelizing it needs
  indentation checkpoints. See [`benches/support/parallel.rs`](benches/support/parallel.rs).
- **Only the TypeScript front end exists.** The front-end interface is ready for others. See
  [front ends](docs/front-ends.md) for how to add one.

## Done recently

- The durable host and debugger are compiled into the binary, so `graphite` works after install.
- `graphite init`, `graphite doctor`, and per-command help.
- `compile` defaults its output directory, and reports collisions with a location.
