//! `graphite run`: compiles the graph and runs it under the durable host. With `--each`, the item
//! graph runs once per item the first graph produces. State is saved after each checkpoint, so
//! running the same command again resumes where it stopped.

use std::ffi::OsString;
use std::path::Path;

use graphite::frontend::Module;
use graphite::frontend::registry;
use graphite::pipeline::write_modules;

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
        eprintln!(
            "warning: --runner runs {} as code with your full user permissions: it can read your files, use the network, and see your secrets. Run only runners you wrote or have reviewed.",
            path.display()
        );
    }
    if let Some(path) = options.tasks {
        eprintln!(
            "warning: --tasks runs the commands in {} with your full user permissions: they can read your files, use the network, and see your secrets. Run only commands you wrote or have reviewed.",
            path.display()
        );
    }
    // Read before anything is locked or written, so a bad manifest changes nothing.
    let task_table = options.tasks.map(tasks::table_json).transpose()?;
    let (_, graphs) = load::graphs(input, options.allow)?;
    let entry = load::select(&graphs, None)?;
    let item_graphs = match &options.fan_out {
        Some(fan) => Some(load::graphs(fan.each, options.allow)?.1),
        None => None,
    };
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
    let result: Result<(), String> = (|| {
        write_modules(frontend, &graphs, &directory).map_err(|error| error.to_string())?;
        let module = directory.join(frontend.file_name(entry, Module::Index));
        let mut args: Vec<OsString> = vec![module.into_os_string(), state.as_os_str().to_owned()];
        if let (Some(fan), Some(item_graphs)) = (&options.fan_out, &item_graphs) {
            let item_dir = directory.join("item");
            write_modules(frontend, item_graphs, &item_dir).map_err(|error| error.to_string())?;
            let item_entry = load::select(item_graphs, None)?;
            let item_module = item_dir.join(frontend.file_name(item_entry, Module::Index));
            args.extend(["--each".into(), item_module.into_os_string()]);
            args.extend(["--key".into(), fan.key.into()]);
            if let Some(seen) = fan.seen {
                args.extend(["--seen".into(), seen.as_os_str().to_owned()]);
            }
            if let Some(until) = fan.until_passed {
                args.extend(["--until-passed".into(), until.to_string().into()]);
            }
        }
        if let Some(path) = options.runner {
            args.extend(["--runner".into(), path.as_os_str().to_owned()]);
        }
        if let Some(path) = options.input_data {
            args.extend(["--input".into(), path.as_os_str().to_owned()]);
        }
        if let Some(path) = &task_table {
            let file = directory.join("tasks.json");
            std::fs::write(&file, path).map_err(|error| format!("{}: {error}", file.display()))?;
            args.extend(["--tasks".into(), file.into_os_string()]);
        }
        let status = js::launch(&host, &args)?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("durable run did not end ({status})"))
        }
    })();
    host_assets::discard(&bundle);
    let _ = std::fs::remove_dir_all(&directory);
    result
}
