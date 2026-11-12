# Front ends

A **front end** turns a checked graph into code for one host language. The parser, the checks,
and the compiled graph know nothing about any language. A language appears only in its front end.

Graphite ships one front end today: **TypeScript**, the default for every command. The front end
interface is general, so supporting another language means writing one front end, not changing
the graph language.

| Key (`--frontend`) | Language | Status |
| --- | --- | --- |
| `typescript` | TypeScript | Shipped. The default. |
| `rust` | Rust | Shipped. Plain `static` data, one module per graph. |

## What a front end produces

For each graph, a front end writes:

- an **index** module: the graph's names, its shared tables, and the list of its chunks;
- **node chunk** modules: nodes with their outgoing edges, flags and annotations;
- **choice chunk** modules, when the graph has choices;
- a **runtime** file, written once per output directory, that every index imports.

The layout (which nodes go in which chunk, and the file names) is shared by all front ends. A
front end only renders text. Graphite checks the output file names for collisions before it writes
anything, for every front end.

## Choosing a front end

`check`, `compile` and `debug` take `--frontend <key>`:

```sh
graphite compile door.graphite --frontend typescript
```

In Rust, `graphite::frontend::registry::lookup` finds a front end by its key.

## The Rust front end

`--frontend rust` writes one module per graph, `<graph>.rs`, and a `graphite.rs` runtime. The
generated module holds the graph as a `static GRAPH: Graph` value, plus constants for every
name:

- `nodes::CLOSED` is a `NodeId`, and `nodes` names each node in source order.
- `conditions::REQUEST` is the condition's index, as a `u32`.
- `choices::X` is the choice's index, as a `u32`.

Names become upper-case identifiers. Other characters become `_`, and two names that land on
the same identifier are an error. The host crate declares the runtime and the modules:

```rust
mod graphite;   // the runtime, written next to the generated modules
mod door;       // door.rs, generated from door.graphite

let (id, node) = door::GRAPH.node("closed").unwrap();
```

The runtime has the data types, `Graph::node` and `Graph::get`, and a `Run` that steps the graph
with the same rules as the TypeScript runtime. `graphite debug` builds a Rust debugger for the
graph with `rustc`, so `debug` works with `--frontend rust`. The durable engine (`run`,
`--simulate`) is still TypeScript-only.

## Adding a front end

There are two routes.

**1. A language with the same shape as TypeScript.** Implement the `Syntax` trait in
`src/frontend/<language>/`. It has six methods: the file extension, the runtime's file name and
text, the index module, a node chunk, and a choice chunk. Each method writes text and nothing else.
`Layered` supplies the module plan, file names and dispatch. The TypeScript front end in
`src/frontend/typescript/` is the model to copy.

**2. Any other output.** Implement the `Frontend` trait directly. It has five methods: `modules`,
`file_name`, `emit_module`, `emit_runtime` and `runtime_file_name`. A complete example that
writes a plain text file is in [`tests/frontend_contract.rs`](../tests/frontend_contract.rs).

Then register the front end:

1. Add a `Target` for it, with its `key`, its `frontend`, and an optional `debugger`. The debugger
   is a script the CLI runs with `graphite debug`; leave it out if the language has none.
2. Add the `Target` to `TARGETS` in [`src/frontend/registry.rs`](../src/frontend/registry.rs).

The new front end then works with `--frontend <key>` and with the Rust API.

## What a front end does not decide

- **The language's runtime behavior.** A front end emits code that drives the graph. The step
  rules come from the core, but each host language's runtime is written by that front end, so keep
  it faithful to the rules in the [DSL reference](dsl.md).
- **Durable runs.** `graphite run` uses the TypeScript durable engine today. A new front end does not get
  durable runs unless someone writes a durable engine for its language.
