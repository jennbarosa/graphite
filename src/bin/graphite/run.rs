//! `graphite run --durable`: compiles the first graph and runs it under the durable host. State
//! is saved after each checkpoint, so running the same command again resumes where it stopped.

use std::path::Path;

use graphite::frontend::registry;
use graphite::frontend::{Frontend, Module};
use graphite::graph::CompiledGraph;
use graphite::pipeline::write_modules;

use crate::{host_assets, js, load, scratch, tasks};

/// Only `--runner` and `--tasks` name task code: a file in the working directory never chooses it.
pub fn run(
    input: &Path,
    state: Option<&Path>,
    runner: Option<&Path>,
    allow: Option<&Path>,
    input_data: Option<&Path>,
    task_manifest: Option<&Path>,
) -> Result<(), String> {
    if let Some(path) = runner {
        eprintln!(
            "warning: --runner runs {} as code with your full user permissions: it can read your files, use the network, and see your secrets. Run only runners you wrote or have reviewed.",
            path.display()
        );
    }
    if let Some(path) = task_manifest {
        eprintln!(
            "warning: --tasks runs the commands in {} with your full user permissions: they can read your files, use the network, and see your secrets. Run only commands you wrote or have reviewed.",
            path.display()
        );
    }
    // Read before anything is locked or written, so a bad manifest changes nothing.
    let task_table = task_manifest.map(tasks::table_json).transpose()?;
    let (_, graphs) = load::graphs(input, allow)?;
    let entry = load::select(&graphs, None)?;
    let frontend = load::front_end(registry::DURABLE_FRONT_END)?.frontend;
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
    let table_file = match &task_table {
        Some(table) => {
            let file = directory.join("tasks.json");
            std::fs::write(&file, table).map_err(|error| format!("{}: {error}", file.display()))?;
            Some(file)
        }
        None => None,
    };
    let result = run_in(
        frontend,
        &graphs,
        entry,
        &directory,
        &state,
        &Launch {
            runner,
            input_data,
            tasks: table_file.as_deref(),
        },
        &bundle.join(host_assets::HOST_ENTRY),
    );
    host_assets::discard(&bundle);
    let _ = std::fs::remove_dir_all(&directory);
    result
}

/// The options the host takes, as flags.
struct Launch<'a> {
    runner: Option<&'a Path>,
    input_data: Option<&'a Path>,
    tasks: Option<&'a Path>,
}

fn run_in(
    frontend: &dyn Frontend,
    graphs: &[CompiledGraph],
    entry: &CompiledGraph,
    directory: &Path,
    state: &Path,
    launch: &Launch<'_>,
    host: &Path,
) -> Result<(), String> {
    write_modules(frontend, graphs, directory).map_err(|error| error.to_string())?;
    let module = directory.join(frontend.file_name(entry, Module::Index));
    let mut args = vec![module.into_os_string(), state.as_os_str().to_owned()];
    for (flag, value) in [
        ("--runner", launch.runner),
        ("--input", launch.input_data),
        ("--tasks", launch.tasks),
    ] {
        if let Some(value) = value {
            args.push(flag.into());
            args.push(value.as_os_str().to_owned());
        }
    }
    let status = js::launch(host, &args)?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("durable run did not end ({status})"))
    }
}
