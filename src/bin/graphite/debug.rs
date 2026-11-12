//! `graphite debug`: compiles a DSL file, then runs the front end's debugger on one of its graphs.
//! With `--simulate`, it runs the durable engine on the graph instead, with each task's outcome
//! stubbed, so no task code runs. Output goes to a scratch directory unless `--out` names one, and
//! the scratch copy is removed.

use std::ffi::OsString;
use std::path::Path;

use graphite::frontend::Module;
use graphite::frontend::registry::{self, Debugger};
use graphite::pipeline::write_modules;

use crate::{debugger, js, load, runtime_assets, scratch};

/// What `debug` runs: the stepping debugger with `commands`, or with `simulate` the durable engine
/// over the graph, one `<task>=<outcomes>` stub per entry.
pub fn run(
    input: &Path,
    front_end: &str,
    graph: Option<&str>,
    out: Option<&Path>,
    allow: Option<&Path>,
    commands: &[String],
    simulate: &[String],
) -> Result<(), String> {
    let front_end = load::front_end(front_end)?;
    let debugger = front_end.debugger;
    if simulate.is_empty() && debugger.is_none() {
        return Err(format!("no debugger for front end \"{}\"", front_end.key));
    }
    if !simulate.is_empty() {
        require_durable_front_end(front_end.key)?;
    }
    let (_, graphs) = load::graphs(input, allow)?;
    let entry = load::select(&graphs, graph)?;
    let stem = entry.text(entry.name).to_owned();
    in_directory(out, |directory| {
        write_modules(front_end.frontend, &graphs, directory).map_err(|error| error.to_string())?;
        let module = directory.join(front_end.frontend.file_name(entry, Module::Index));
        match (simulate.is_empty(), debugger) {
            (true, Some(Debugger::Program(build))) => {
                let program = build(directory, &stem)?;
                debugger::run_program(&program, commands)
            }
            (true, Some(Debugger::Script(script))) => {
                with_bundle(|bundle| debugger::launch(&bundle.join(script), &module, commands))
            }
            _ => with_bundle(|bundle| simulate_run(bundle, &module, simulate)),
        }
    })
}

/// Runs `body` with the bundled scripts written to a scratch directory, removed afterwards.
fn with_bundle(body: impl FnOnce(&Path) -> Result<(), String>) -> Result<(), String> {
    let bundle = runtime_assets::materialize()?;
    let result = body(&bundle);
    runtime_assets::discard(&bundle);
    result
}

/// Runs `body` in `out`, or in a scratch directory that is removed afterwards when `out` is not given.
fn in_directory(
    out: Option<&Path>,
    body: impl FnOnce(&Path) -> Result<(), String>,
) -> Result<(), String> {
    match out {
        Some(out) => body(out),
        None => {
            let directory = scratch::create("graphite-debug")?;
            let result = body(&directory);
            let _ = std::fs::remove_dir_all(&directory);
            result
        }
    }
}

/// Simulation runs the durable engine, which only the durable front end's runtime has.
fn require_durable_front_end(key: &str) -> Result<(), String> {
    if key == registry::DURABLE_FRONT_END {
        Ok(())
    } else {
        Err(format!(
            "--simulate runs the durable engine, which only the \"{}\" front end has",
            registry::DURABLE_FRONT_END
        ))
    }
}

/// Runs the simulation script on `module` with the stubs, and fails when the run does not end.
fn simulate_run(bundle: &Path, module: &Path, stubs: &[String]) -> Result<(), String> {
    let mut args: Vec<OsString> = vec![module.as_os_str().to_owned()];
    args.extend(stubs.iter().map(OsString::from));
    let status = js::launch(&bundle.join(runtime_assets::SIMULATE_ENTRY), &args)?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("the simulated run did not end ({status})"))
    }
}

#[cfg(test)]
mod tests {
    use super::require_durable_front_end;
    use graphite::frontend::registry;

    #[test]
    fn simulation_needs_the_durable_front_end() {
        assert_eq!(
            require_durable_front_end(registry::DURABLE_FRONT_END),
            Ok(())
        );
        assert_eq!(
            require_durable_front_end("other"),
            Err(
                "--simulate runs the durable engine, which only the \"typescript\" front end has"
                    .to_string()
            )
        );
    }
}
