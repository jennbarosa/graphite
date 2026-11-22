# Rust API

The Rust crate parses and checks graphs, and generates code through any registered front end. Use it
when your build or tooling is written in Rust. [`examples/rust_api.rs`](../examples/rust_api.rs)
is a complete program, and `cargo run --example rust_api` runs it.

## Compile a graph

```rust
use graphite::parser::{ParseError, Schema};
use graphite::pipeline::compile;

fn main() -> Result<(), ParseError> {
    let graphs = compile(
        "graph walk\n  node stopped end @anim:idle\n",
        &Schema::default(),
    )?;
    let graph = &graphs[0];
    println!("{}", graph.text(graph.name)); // walk
    Ok(())
}
```

`compile(source, schema)` parses and checks every graph in the source, in order. It sizes its
storage from the source, so you do not need to count nodes or strings yourself. This is the function
to use.

## Allow domain flags

`Schema` lists the flags beyond `loop` and `end` that a graph may use. The full program is in
[`examples/rust_api.rs`](../examples/rust_api.rs):

```rust
let flags = ["retry"];
let schema = Schema { node_flags: &flags };
compile("graph job\n  node run retry end\n", &schema)?;
```

## Generate code

`graphite::pipeline::write_modules(frontend, &graphs, directory)` writes the runtime and every
module. `graphite::pipeline::check_outputs(frontend, &graphs)` checks the output names without
writing anything. Front ends are listed in `graphite::frontend::registry::TARGETS`, and
`graphite::frontend::registry::lookup(key)` finds one by its key. The TypeScript front end is
`graphite::frontend::typescript::TYPESCRIPT`.

`write_modules` never overwrites a file. If a file already exists, or two graphs would write the
same name, it fails before writing anything. It renders every file in memory first, so a render
error leaves nothing on disk. `check_outputs` checks the names and each front end's cheaper name check,
without rendering the files.
`compile` reads `@!frontend:KEY` only to see whether the key names a pure front end. It does not
refuse an unregistered key; the caller does, as the CLI does.

## Compiled data

Compiled graphs are flat arrays. Nodes, edges, choices, arms, flags and annotations are indexed
by ID, and each node's outgoing edges are a range in one array. Treat the fields as read-only:
changing them directly breaks the guarantees parsing established. The annotations on the `graph`
line are the range `graph_annotations` into `annotations`.

## Documentation

The API is documented in the source. Run `cargo doc --open` in a checkout to read it in your
browser.
