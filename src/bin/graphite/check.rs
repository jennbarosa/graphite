//! `graphite check`: parses and validates a DSL file, and checks the target's output names,
//! without writing anything.

use std::path::Path;

use graphite::pipeline::check_outputs;

use crate::load;

pub fn run(input: &Path, target: &str, allow: Option<&Path>) -> Result<(), String> {
    let target = load::target(target)?;
    let (source, graphs) = load::graphs(input, allow)?;
    check_outputs(target.frontend, &graphs)
        .map_err(|collision| collision.diagnostic(&source).to_string())?;
    let plural = if graphs.len() == 1 { "" } else { "s" };
    println!("ok: {} graph{plural}", graphs.len());
    Ok(())
}
