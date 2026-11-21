//! `graphite check`: parses and validates a DSL file, and checks the front end's output names,
//! without writing anything.

use std::path::Path;

use graphite::pipeline::check_outputs;

use crate::allow::Allowlist;
use crate::{Failure, json, load};

pub fn run(
    input: &Path,
    front_end: Option<&str>,
    allow: Option<&Path>,
    pure: bool,
    json: bool,
) -> Result<(), Failure> {
    let graphs = count(input, front_end, allow, pure)?;
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
fn count(
    input: &Path,
    front_end: Option<&str>,
    allow: Option<&Path>,
    pure: bool,
) -> Result<usize, String> {
    let allowlist = allow.map(Allowlist::load).transpose()?;
    let loaded = load::read(input, allowlist.as_ref(), front_end, pure)?;
    check_outputs(loaded.front_end.frontend, &loaded.graphs)
        .map_err(|collision| collision.diagnostic(&loaded.source).to_string())?;
    Ok(loaded.graphs.len())
}
