//! Reads a DSL file into compiled graphs, and picks graphs, front ends and string IDs out of them.
//! The file is read by `pipeline::build::compile_file`, which resolves the front end and mode.

use std::path::Path;

use graphite::frontend::registry::{self, Target};
use graphite::graph::CompiledGraph;
use graphite::pipeline::build::compile_file;
use graphite::preprocess::Directives;

use crate::allow::Allowlist;
use crate::resolve::{self, Output};

/// A file read for a command: its source, its graphs, and the front end and mode it resolved to.
pub struct Loaded {
    pub source: String,
    pub graphs: Vec<CompiledGraph>,
    pub front_end: &'static Target,
}

/// Reads `path` with no front end named on the command line.
pub fn graphs(path: &Path, allow: Option<&Path>) -> Result<(String, Vec<CompiledGraph>), String> {
    let allowlist = allow.map(Allowlist::load).transpose()?;
    graphs_with(path, allowlist.as_ref())
}

/// [`graphs`], with the allowlist already built, for callers that adjust it first.
pub fn graphs_with(
    path: &Path,
    allowlist: Option<&Allowlist>,
) -> Result<(String, Vec<CompiledGraph>), String> {
    let loaded = read(path, allowlist, None, Output::Unused)?;
    Ok((loaded.source, loaded.graphs))
}

/// Reads `path`, resolving its front end from `front_end` (the `--frontend` flag), its directives
/// and its name. The allowlist's node flags are legal in the graphs.
/// `output` says whether the command writes front-end output. See [`Output`].
pub fn read(
    path: &Path,
    allowlist: Option<&Allowlist>,
    front_end: Option<&str>,
    output: Output,
) -> Result<Loaded, String> {
    let source =
        std::fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let node_flags = allowlist.map_or(Vec::new(), Allowlist::node_flags);
    let select = |name: &str, directives: &Directives<'_>| {
        resolve::resolve(name, front_end, directives, output)
    };
    let (chosen, graphs) = compile_file(&source, path, &node_flags, &select)?;
    if let Some(allowlist) = allowlist {
        allowlist.check(&graphs)?;
    }
    Ok(Loaded {
        source,
        graphs,
        front_end: chosen.target,
    })
}

/// Refuses a pure file for a command that runs machine graphs: `run` and `debug`.
pub fn require_runnable(graphs: &[CompiledGraph], path: &Path) -> Result<(), String> {
    match graphs.first() {
        Some(graph) if !graph.kind.runnable() => Err(format!(
            "{} is a {} file; `graphite run` and `graphite debug` run machine graphs",
            path.display(),
            graph.kind.label()
        )),
        _ => Ok(()),
    }
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
