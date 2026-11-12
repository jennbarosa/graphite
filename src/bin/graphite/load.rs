//! Reads a DSL file into compiled graphs, and picks graphs, front ends and string IDs out of them.

use std::path::Path;

use graphite::frontend::registry::{self, Target};
use graphite::graph::CompiledGraph;
use graphite::parser::Schema;
use graphite::pipeline::compile;

use crate::allow::Allowlist;

/// The graphs in `path`, checked against `allow` when one is given, plus the source text their
/// diagnostics point into. The allowlist's node flags are legal in the graphs.
pub fn graphs(path: &Path, allow: Option<&Path>) -> Result<(String, Vec<CompiledGraph>), String> {
    let allowlist = allow.map(Allowlist::load).transpose()?;
    graphs_with(path, allowlist.as_ref())
}

/// [`graphs`], with the allowlist already built, for callers that adjust it first.
pub fn graphs_with(
    path: &Path,
    allowlist: Option<&Allowlist>,
) -> Result<(String, Vec<CompiledGraph>), String> {
    let source =
        std::fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let node_flags = allowlist.map_or(Vec::new(), Allowlist::node_flags);
    let schema = Schema {
        node_flags: &node_flags,
    };
    let graphs =
        compile(&source, &schema).map_err(|error| error.diagnostic(&source).to_string())?;
    if let Some(allowlist) = allowlist {
        allowlist.check(&graphs)?;
    }
    Ok((source, graphs))
}

pub fn front_end(key: &str) -> Result<&'static Target, String> {
    registry::lookup(key).ok_or(format!("unknown front end \"{key}\""))
}

/// The named graph, or the file's first graph when no name is given.
pub fn select<'a>(
    graphs: &'a [CompiledGraph],
    name: Option<&str>,
) -> Result<&'a CompiledGraph, String> {
    match name {
        None => graphs
            .first()
            .ok_or_else(|| "the file has no graphs".to_string()),
        Some(name) => graphs
            .iter()
            .find(|graph| graph.text(graph.name) == name)
            .ok_or_else(|| {
                let names: Vec<&str> = graphs.iter().map(|graph| graph.text(graph.name)).collect();
                format!(
                    "no graph named \"{name}\"; the file has {}",
                    names.join(", ")
                )
            }),
    }
}
