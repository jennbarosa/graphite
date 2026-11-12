//! `graphite run --durable`: compiles the first graph and runs it under the durable host. State
//! is saved after each checkpoint, so running the same command again resumes where it stopped.

use std::path::Path;

use graphite::frontend::{Frontend, Module};
use graphite::graph::CompiledGraph;
use graphite::pipeline::write_modules;

use crate::{host_assets, js, load, scratch};

/// Only `--runner` names the runner: a file in the working directory never chooses code to run.
pub fn run(
    input: &Path,
    state: Option<&Path>,
    runner: Option<&Path>,
    allow: Option<&Path>,
) -> Result<(), String> {
    if let Some(path) = runner {
        eprintln!(
            "warning: --runner runs {} as code with your full user permissions: it can read your files, use the network, and see your secrets. Run only runners you wrote or have reviewed.",
            path.display()
        );
    }
    let (_, graphs) = load::graphs(input, allow)?;
    let entry = load::select(&graphs, None)?;
    let frontend = load::front_end("typescript")?.frontend;
    let state = state.map_or_else(|| input.with_extension("state.json"), Path::to_path_buf);
    // The host locks the state file beside itself, so its directory must exist.
    if let Some(parent) = state
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("{}: {error}", parent.display()))?;
    }
    // The generated modules are only needed while the run is active. The state is kept apart.
    let directory = scratch::create("graphite-run")?;
    let bundle = host_assets::materialize()?;
    let result = run_in(
        frontend,
        &graphs,
        entry,
        &directory,
        &state,
        runner,
        &bundle.join(host_assets::HOST_ENTRY),
    );
    host_assets::discard(&bundle);
    let _ = std::fs::remove_dir_all(&directory);
    result
}

fn run_in(
    frontend: &dyn Frontend,
    graphs: &[CompiledGraph],
    entry: &CompiledGraph,
    directory: &Path,
    state: &Path,
    runner: Option<&Path>,
    host: &Path,
) -> Result<(), String> {
    write_modules(frontend, graphs, directory).map_err(|error| error.to_string())?;
    let module = directory.join(frontend.file_name(entry, Module::Index));
    let mut args = vec![module.into_os_string(), state.as_os_str().to_owned()];
    if let Some(runner) = runner {
        args.push(runner.as_os_str().to_owned());
    }
    let status = js::launch(host, &args)?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("durable run did not end ({status})"))
    }
}
