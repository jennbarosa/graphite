//! `graphite debug`: compiles a DSL file, then runs the front end's debugger on one of its graphs.
//! Output goes to a scratch directory unless `--out` names one, and the scratch copy is removed.

use std::path::Path;

use graphite::frontend::{Frontend, Module};
use graphite::graph::CompiledGraph;
use graphite::pipeline::write_modules;

use crate::{debugger, load, runtime_assets, scratch};

pub fn run(
    input: &Path,
    front_end: &str,
    graph: Option<&str>,
    out: Option<&Path>,
    allow: Option<&Path>,
    commands: &[String],
) -> Result<(), String> {
    let front_end = load::front_end(front_end)?;
    let script = front_end
        .debugger
        .ok_or_else(|| format!("no debugger for front end \"{}\"", front_end.key))?;
    let (_, graphs) = load::graphs(input, allow)?;
    let entry = load::select(&graphs, graph)?;
    let scratch = out.is_none();
    let directory = match out {
        Some(out) => out.to_path_buf(),
        None => scratch::create("graphite-debug")?,
    };
    let bundle = runtime_assets::materialize()?;
    let result = debug_in(
        front_end.frontend,
        &graphs,
        entry,
        &directory,
        &bundle.join(script),
        commands,
    );
    runtime_assets::discard(&bundle);
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
    script: &Path,
    commands: &[String],
) -> Result<(), String> {
    write_modules(frontend, graphs, directory).map_err(|error| error.to_string())?;
    let module = directory.join(frontend.file_name(entry, Module::Index));
    debugger::launch(script, &module, commands)
}
