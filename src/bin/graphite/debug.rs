//! `graphite debug`: compiles a DSL file, then runs the target's debugger on one of its graphs.
//! Output goes to a scratch directory unless `--out` names one, and the scratch copy is removed.

use std::path::Path;

use graphite::frontend::{Frontend, Module};
use graphite::graph::CompiledGraph;
use graphite::pipeline::write_modules;

use crate::{debugger, load, scratch};

pub fn run(
    input: &Path,
    target: &str,
    graph: Option<&str>,
    out: Option<&Path>,
    allow: Option<&Path>,
    commands: &[String],
) -> Result<(), String> {
    let target = load::target(target)?;
    let script = target
        .debugger
        .ok_or_else(|| format!("no debugger for target \"{}\"", target.key))?;
    let (_, graphs) = load::graphs(input, allow)?;
    let entry = load::select(&graphs, graph)?;
    let scratch = out.is_none();
    let directory = match out {
        Some(out) => out.to_path_buf(),
        None => scratch::create("graphite-debug")?,
    };
    let result = debug_in(
        target.frontend,
        &graphs,
        entry,
        &directory,
        script,
        commands,
    );
    if scratch {
        let _ = std::fs::remove_dir_all(&directory);
    }
    result
}

fn debug_in(
    frontend: &dyn Frontend,
    graphs: &[CompiledGraph],
    entry: &CompiledGraph,
    directory: &Path,
    script: &str,
    commands: &[String],
) -> Result<(), String> {
    write_modules(frontend, graphs, directory).map_err(|error| error.to_string())?;
    let module = directory.join(frontend.file_name(entry, Module::Index));
    debugger::launch(script, &module, commands)
}
