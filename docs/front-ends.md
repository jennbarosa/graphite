# Front ends

A **front end** turns a checked graph into code for one host language. The parser, the checks,
and the compiled graph know nothing about any language. A language appears only in its front end.

Graphite ships one front end today: **TypeScript**, the default for every command. The front end
interface is general, so supporting another language means writing one front end, not changing
the graph language.

| Key (`--frontend`) | Language | Status |
| --- | --- | --- |
| `typescript` | TypeScript | Shipped. The default. |

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
graphite compile door.dsl --frontend typescript
```

In Rust, `graphite::frontend::registry::lookup` finds a front end by its key.

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
- **Durable runs.** `graphite run` uses the TypeScript host today. A new front end does not get
  durable runs unless someone writes a host for its language.
