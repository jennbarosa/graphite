# Front ends

A **front end** turns a checked graph into code for one host language. The parser, the checks,
and the compiled graph know nothing about any language. A language appears only in its front end.

Graphite ships two front ends: **TypeScript**, the default for every command, and **Rust**. The
front end interface is general, so supporting another language means writing one front end, not
changing the graph language.

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

`check`, `compile` and `debug` take `--frontend <key>`. Without it, `check` and `compile` pick the
front end from the file's name and directives, as the [CLI reference](cli.md#choosing-the-front-end-and-mode)
describes:

```sh
graphite compile door.graphite --frontend typescript
graphite compile combat.pure.graphite          # PureScript, from the name
```

In Rust, `graphite::frontend::registry::lookup` finds a front end by its key.

A front end writes machine graphs (`Kind::Machine`, the default) or pure graphs (`Kind::Pure`). The
pipeline refuses a graph whose kind differs from its front end before it writes anything.

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
`--simulate`) is still TypeScript-only. `Node::timeout` reads a node's `@timeout:MS` annotation
with the same rule as the TypeScript engine, and returns `Err` for a value it refuses. It does not
enforce the limit; a Rust host applies it to its own tasks:

```rust
let limit = door::GRAPH.get(door::nodes::CLOSED).timeout()?; // Option<u32>, in milliseconds
```

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

### Adding a pure language

A pure language is one `Language`, one `PureSyntax` and one `TARGETS` entry. The shared pure front
end, `Pure<S>` in `src/frontend/pure/`, does the rest: it lowers the graph, checks the names, and
names the file.

1. **`src/frontend/lang/<language>.rs`**: a `Language` impl with the language's facts only: its
   extension, its string escape rule, its `none` and `some` literals, its keywords, `spell` (one
   of the case converters `pascal`, `camel` or `snake`), `escape_keyword`, and `namespace`. The
   string, identifier and annotation literals are provided methods, so no language copies them.
   Two optional methods: `is_valid` refuses a spelling the language cannot use (the default
   accepts any plain identifier), and `imported` on `PureSyntax` lists the names the module
   imports, so a source name cannot clash with them.
2. **`src/frontend/pure/<language>.rs`**: a `PureSyntax` impl with the layout (`module`) and the
   names the layout generates (`generated`), so the collision check sees them. Its `Target` goes
   next to it.
3. **One entry** in `TARGETS`. Inference finds the target by its extension and kind, so nothing
   else changes.

[`src/frontend/pure/tests.rs`](../src/frontend/pure/tests.rs) holds a test-only language that does
exactly these three steps. [PureScript](purescript.md) is the worked example. The `namespace` method says which names a language keeps apart, and the
[PureScript](purescript.md) entry shows the answer for one language.

## What a front end does not decide

- **The language's runtime behavior.** A front end emits code that drives the graph. The step
  rules come from the core, but each host language's runtime is written by that front end, so keep
  it faithful to the rules in the [DSL reference](dsl.md).
- **Durable runs.** `graphite run` uses the TypeScript durable engine today. A new front end does not get
  durable runs unless someone writes a durable engine for its language.
