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
same name, it fails before writing anything.

## Advanced: fixed storage

`graphite::parser::parse(source, schema, capacities)` takes one `Capacity` per graph, in source
order, and fails if a capacity is too small rather than growing. Use it when storage must be
allocated up front, for example in a loop that must not allocate.
`graphite::pipeline::block_capacities(source, schema)` computes capacities that always fit.

## Compiled data

Compiled graphs are flat arrays. Nodes, edges, choices, arms, flags and annotations are indexed
by ID, and each node's outgoing edges are a range in one array. Treat the fields as read-only:
changing them directly breaks the guarantees parsing established. The annotations on the `graph`
line are the range `graph_annotations` into `annotations`.

## Documentation

The API is documented in the source. Run `cargo doc --open` in a checkout to read it in your
browser.
