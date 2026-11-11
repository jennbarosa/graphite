//! Compiles graphs from Rust. Run with `cargo run --example rust_api`.

use graphite::parser::{ParseError, Schema};
use graphite::pipeline::compile;

fn main() -> Result<(), ParseError> {
    let graphs = compile(
        "graph walk\n  node stopped end @anim:idle\n",
        &Schema::default(),
    )?;
    let graph = &graphs[0];
    println!("{}", graph.text(graph.name)); // walk

    // A domain flag is allowed only when the schema names it.
    let flags = ["retry"];
    let schema = Schema { node_flags: &flags };
    compile("graph job\n  node run retry end\n", &schema)?;
    Ok(())
}
