//! `graphite run`: compiles the graph and runs it under the durable host. With `--each`, the item
//! graph runs once per item the first graph produces. State is saved after each checkpoint, so
//! running the same command again resumes where it stopped.

use std::ffi::OsString;
use std::path::Path;

use graphite::frontend::Module;
use graphite::frontend::registry;
use graphite::pipeline::write_modules;

use crate::allow::Allowlist;
use crate::{host_assets, js, load, scratch, tasks};

/// What one `graphite run` takes, besides the graph and its state file.
pub struct Options<'a> {
    /// Names a runner module. Its code runs as this user.
    pub runner: Option<&'a Path>,
    /// Names a `tasks.toml`. Its commands run as this user.
    pub tasks: Option<&'a Path>,
    /// A JSON file whose value every task receives as its input.
    pub input_data: Option<&'a Path>,
    pub allow: Option<&'a Path>,
    /// Runs the item graph once per item the graph produces.
    pub fan_out: Option<FanOut<'a>>,
}

/// The item graph and the rules for running it once per item.
pub struct FanOut<'a> {
    pub each: &'a Path,
    /// The field of each item that names it.
    pub key: &'a str,
    /// The seen file. Defaults beside the state file.
    pub seen: Option<&'a Path>,
    /// Stop starting items once this many have ended on a passing stage.
    pub until_passed: Option<u32>,
}

pub fn run(input: &Path, state: Option<&Path>, options: &Options<'_>) -> Result<(), String> {
    if let Some(path) = options.runner {
        warn_as_code(
            &format!("--runner runs {} as code", path.display()),
            "runners",
        );
    }
    if let Some(path) = options.tasks {
        warn_as_code(
            &format!("--tasks runs the commands in {}", path.display()),
            "commands",
        );
    }
    // Read before anything is locked or written, so a bad manifest changes nothing.
    let task_table = options.tasks.map(tasks::table_json).transpose()?;
    // With a manifest, its task names are the only ones a graph may use, so the allowlist need not
    // list tasks itself. Without one, the allowlist's own task list applies.
    let allowlist = match (options.allow, options.tasks) {
        (allow, Some(tasks_path)) => {
            let base = allow.map(Allowlist::load).transpose()?;
            Some(
                base.unwrap_or_else(Allowlist::unrestricted)
                    .restrict("task", tasks::names(tasks_path)?),
            )
        }
        (allow, None) => allow.map(Allowlist::load).transpose()?,
    };
    let (_, graphs) = load::graphs_with(input, allowlist.as_ref())?;
    let entry = load::select(&graphs, None)?;
    let item_graphs = options
        .fan_out
        .as_ref()
        .map(|fan| load::graphs_with(fan.each, allowlist.as_ref()).map(|(_, graphs)| graphs))
        .transpose()?;
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
    let host = bundle.join(host_assets::HOST_ENTRY);

    let mut args: Vec<OsString> = vec![];
    let result = write_run_modules(frontend, &graphs, entry, &directory, &mut args, &state)
        .and_then(|()| {
            if let (Some(fan), Some(item_graphs)) = (&options.fan_out, &item_graphs) {
                let item_dir = directory.join("item");
                write_modules(frontend, item_graphs, &item_dir)
                    .map_err(|error| error.to_string())?;
                let item_entry = load::select(item_graphs, None)?;
                let item_module = item_dir.join(frontend.file_name(item_entry, Module::Index));
                push_flag(&mut args, "--each", Some(item_module.as_os_str()));
                push_flag(&mut args, "--key", Some(fan.key.as_ref()));
                push_flag(&mut args, "--seen", fan.seen.map(Path::as_os_str));
                let until = fan
                    .until_passed
                    .map(|count| OsString::from(count.to_string()));
                push_flag(&mut args, "--until-passed", until.as_deref());
            }
            push_flag(&mut args, "--runner", options.runner.map(Path::as_os_str));
            push_flag(
                &mut args,
                "--input",
                options.input_data.map(Path::as_os_str),
            );
            if let Some(table) = &task_table {
                let file = directory.join("tasks.json");
                std::fs::write(&file, table)
                    .map_err(|error| format!("{}: {error}", file.display()))?;
                push_flag(&mut args, "--tasks", Some(file.as_os_str()));
            }
            let status = js::launch(&host, &args)?;
            if status.success() {
                Ok(())
            } else {
                Err(format!("durable run did not end ({status})"))
            }
        });
    host_assets::discard(&bundle);
    let _ = std::fs::remove_dir_all(&directory);
    result
}

/// Writes the graph's modules, and starts the host's argument list with the module and state path.
fn write_run_modules(
    frontend: &dyn graphite::frontend::Frontend,
    graphs: &[graphite::graph::CompiledGraph],
    entry: &graphite::graph::CompiledGraph,
    directory: &Path,
    args: &mut Vec<OsString>,
    state: &Path,
) -> Result<(), String> {
    write_modules(frontend, graphs, directory).map_err(|error| error.to_string())?;
    let module = directory.join(frontend.file_name(entry, Module::Index));
    args.push(module.into_os_string());
    args.push(state.as_os_str().to_owned());
    Ok(())
}

/// Appends `flag value` to the host's arguments, when there is a value.
fn push_flag(args: &mut Vec<OsString>, flag: &str, value: Option<&std::ffi::OsStr>) {
    if let Some(value) = value {
        args.push(flag.into());
        args.push(value.to_owned());
    }
}

/// The warning for a flag that runs code. The code runs with the user's permissions, so say so.
fn warn_as_code(what: &str, reviewed: &str) {
    eprintln!(
        "warning: {what} with your full user permissions: they can read your files, use the network, and see your secrets. Run only {reviewed} you wrote or have reviewed."
    );
}
