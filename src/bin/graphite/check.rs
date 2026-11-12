//! `graphite check`: parses and validates a DSL file, and checks the front end's output names,
//! without writing anything.

use std::path::Path;

use graphite::pipeline::check_outputs;

use crate::load;

pub fn run(input: &Path, front_end: &str, allow: Option<&Path>) -> Result<(), String> {
    let front_end = load::front_end(front_end)?;
    let (source, graphs) = load::graphs(input, allow)?;
    check_outputs(front_end.frontend, &graphs)
        .map_err(|collision| collision.diagnostic(&source).to_string())?;
    let plural = if graphs.len() == 1 { "" } else { "s" };
    println!("ok: {} graph{plural}", graphs.len());
    Ok(())
}
