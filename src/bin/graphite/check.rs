//! `graphite check`: parses and validates a DSL file, and checks the front end's output names,
//! without writing anything.

use std::path::Path;

use graphite::pipeline::check_outputs;

use crate::{Failure, json, load};

pub fn run(input: &Path, front_end: &str, allow: Option<&Path>, json: bool) -> Result<(), Failure> {
    let graphs = count(input, front_end, allow)?;
    if json {
        println!(
            "{}",
            json::object(&[("ok", "true".to_string()), ("graphs", graphs.to_string())])
        );
    } else {
        let plural = if graphs == 1 { "" } else { "s" };
        println!("ok: {graphs} graph{plural}");
    }
    Ok(())
}

/// Checks the file and returns how many graphs it holds.
fn count(input: &Path, front_end: &str, allow: Option<&Path>) -> Result<usize, String> {
    let front_end = load::front_end(front_end)?;
    let (source, graphs) = load::graphs(input, allow)?;
    check_outputs(front_end.frontend, &graphs)
        .map_err(|collision| collision.diagnostic(&source).to_string())?;
    Ok(graphs.len())
}
