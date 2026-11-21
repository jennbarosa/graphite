//! Reads a DSL file into compiled graphs, and picks graphs, front ends and string IDs out of them.
//! This is the one place a file is read: it resolves the front end and the mode, then compiles.

use std::borrow::Cow;
use std::path::Path;

use graphite::frontend::registry::{self, Target};
use graphite::graph::CompiledGraph;
use graphite::lexer::Diagnostic;
use graphite::parser::Schema;
use graphite::pipeline::compile;
use graphite::preprocess::preprocess;

use crate::allow::Allowlist;
use crate::resolve;

/// A file read for a command: its source, its graphs, and the front end and mode it resolved to.
pub struct Loaded {
    pub source: String,
    pub graphs: Vec<CompiledGraph>,
    pub front_end: &'static Target,
    pub pure: bool,
}

/// Reads `path` with no front end or mode named on the command line.
pub fn graphs(path: &Path, allow: Option<&Path>) -> Result<(String, Vec<CompiledGraph>), String> {
    let allowlist = allow.map(Allowlist::load).transpose()?;
    graphs_with(path, allowlist.as_ref())
}

/// [`graphs`], with the allowlist already built, for callers that adjust it first.
pub fn graphs_with(
    path: &Path,
    allowlist: Option<&Allowlist>,
) -> Result<(String, Vec<CompiledGraph>), String> {
    let loaded = read(path, allowlist, None, false)?;
    Ok((loaded.source, loaded.graphs))
}

/// Reads `path`, resolving its front end from `front_end` (the `--frontend` flag), `pure` (the
/// `--pure` flag), its name and its directives. The allowlist's node flags are legal in the graphs.
pub fn read(
    path: &Path,
    allowlist: Option<&Allowlist>,
    front_end: Option<&str>,
    pure: bool,
) -> Result<Loaded, String> {
    let source =
        std::fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let directives = preprocess(&source).map_err(|error| error.diagnostic(&source).to_string())?;
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let resolved = resolve::resolve(&name, front_end, pure, &directives)?;
    let node_flags = allowlist.map_or(Vec::new(), Allowlist::node_flags);
    let schema = Schema {
        node_flags: &node_flags,
        pure: resolved.pure,
    };
    let graphs = compile(&source, &schema)
        .map_err(|error| explain_pure(error, &source, resolved.because.as_deref()))?;
    if let Some(allowlist) = allowlist {
        allowlist.check(&graphs)?;
    }
    Ok(Loaded {
        source,
        graphs,
        front_end: resolved.target,
        pure: resolved.pure,
    })
}

/// The diagnostic for a compile error. A machine line in a pure file gets the reason the file is
/// pure, so the fix is clear.
fn explain_pure(
    error: graphite::parser::ParseError,
    source: &str,
    because: Option<&str>,
) -> String {
    let machine_line = error.message.ends_with("is not allowed in a pure file");
    match (because, machine_line) {
        (Some(because), true) => {
            let message = format!("{} (pure {because})", error.message);
            Diagnostic::new(Cow::Owned(message), error.offset, source).to_string()
        }
        _ => error.diagnostic(source).to_string(),
    }
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
