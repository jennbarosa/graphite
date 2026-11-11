//! Parses the command line into a [`Command`]. Rejects options a command does not take.

use std::path::PathBuf;

pub const USAGE: &str = "usage:
  graphite compile <file.dsl> --out <dir> [--target typescript] [--allow <list.toml>]
  graphite check <file.dsl> [--target typescript] [--allow <list.toml>]
  graphite inspect <file.dsl>
  graphite debug <file.dsl> [--target typescript] [--graph <name>] [--out <dir>] [--allow <list.toml>] [command...]
  graphite run --durable <file.dsl> [--state <file.json>] [--runner <tasks.ts>] [--allow <list.toml>]";

const DEFAULT_TARGET: &str = "typescript";

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Compile {
        input: PathBuf,
        out: PathBuf,
        target: String,
        allow: Option<PathBuf>,
    },
    Check {
        input: PathBuf,
        target: String,
        allow: Option<PathBuf>,
    },
    Inspect {
        input: PathBuf,
    },
    Debug {
        input: PathBuf,
        target: String,
        graph: Option<String>,
        out: Option<PathBuf>,
        allow: Option<PathBuf>,
        /// Each entry is one debugger command line, run in order.
        commands: Vec<String>,
    },
    /// Runs the first graph under the durable host. `runner` names a module whose default
    /// export is the task runner; without it, every task settles as `none`.
    Run {
        input: PathBuf,
        state: Option<PathBuf>,
        runner: Option<PathBuf>,
        allow: Option<PathBuf>,
    },
}

/// Options as written, before each command decides which ones it accepts.
#[derive(Default)]
struct Options {
    out: Option<PathBuf>,
    target: Option<String>,
    graph: Option<String>,
    allow: Option<PathBuf>,
    durable: bool,
    state: Option<PathBuf>,
    runner: Option<PathBuf>,
}

pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Command, String> {
    let mut args = args.into_iter();
    let verb = args.next().ok_or_else(|| USAGE.to_string())?;
    let mut options = Options::default();
    let mut positionals = Vec::new();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--out" => options.out = Some(PathBuf::from(value(&mut args, "--out")?)),
            "--target" => options.target = Some(value(&mut args, "--target")?),
            "--graph" => options.graph = Some(value(&mut args, "--graph")?),
            "--allow" => options.allow = Some(PathBuf::from(value(&mut args, "--allow")?)),
            "--state" => options.state = Some(PathBuf::from(value(&mut args, "--state")?)),
            "--runner" => options.runner = Some(PathBuf::from(value(&mut args, "--runner")?)),
            "--durable" => options.durable = true,
            flag if flag.starts_with("--") => return Err(format!("unknown option {flag}")),
            _ => positionals.push(arg),
        }
    }
    let mut positionals = positionals.into_iter();
    let input = PathBuf::from(positionals.next().ok_or_else(|| USAGE.to_string())?);
    let rest: Vec<String> = positionals.collect();
    let Options {
        out,
        target,
        graph,
        allow,
        durable,
        state,
        runner,
    } = options;
    let target_given = target.is_some();
    let target = target.unwrap_or_else(|| DEFAULT_TARGET.to_string());
    match verb.as_str() {
        "compile" => {
            only_file(&rest, &verb)?;
            refuse(graph.is_some(), "--graph", &verb)?;
            refuse_run_flags(durable, &state, &runner, &verb)?;
            let out = out.ok_or("compile needs --out <dir>")?;
            Ok(Command::Compile {
                input,
                out,
                target,
                allow,
            })
        }
        "check" => {
            only_file(&rest, &verb)?;
            refuse(out.is_some(), "--out", &verb)?;
            refuse(graph.is_some(), "--graph", &verb)?;
            refuse_run_flags(durable, &state, &runner, &verb)?;
            Ok(Command::Check {
                input,
                target,
                allow,
            })
        }
        "inspect" => {
            only_file(&rest, &verb)?;
            refuse(out.is_some(), "--out", &verb)?;
            refuse(graph.is_some(), "--graph", &verb)?;
            refuse(target_given, "--target", &verb)?;
            refuse(allow.is_some(), "--allow", &verb)?;
            refuse_run_flags(durable, &state, &runner, &verb)?;
            Ok(Command::Inspect { input })
        }
        "run" => {
            only_file(&rest, &verb)?;
            refuse(out.is_some(), "--out", &verb)?;
            refuse(graph.is_some(), "--graph", &verb)?;
            refuse(target_given, "--target", &verb)?;
            if !durable {
                return Err(
                    "run needs --durable; the durable host is the only run mode for now".into(),
                );
            }
            Ok(Command::Run {
                input,
                state,
                runner,
                allow,
            })
        }
        "debug" => {
            refuse_run_flags(durable, &state, &runner, &verb)?;
            Ok(Command::Debug {
                input,
                target,
                graph,
                out,
                allow,
                commands: rest,
            })
        }
        _ => Err(USAGE.to_string()),
    }
}

fn value(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, String> {
    args.next().ok_or(format!("{flag} needs a value"))
}

fn only_file(rest: &[String], verb: &str) -> Result<(), String> {
    if rest.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{verb} takes one file, got extra argument \"{}\"",
            rest[0]
        ))
    }
}

/// `--durable`, `--state` and `--runner` belong to `run` alone.
fn refuse_run_flags(
    durable: bool,
    state: &Option<PathBuf>,
    runner: &Option<PathBuf>,
    verb: &str,
) -> Result<(), String> {
    refuse(durable, "--durable", verb)?;
    refuse(state.is_some(), "--state", verb)?;
    refuse(runner.is_some(), "--runner", verb)
}

fn refuse(given: bool, flag: &str, verb: &str) -> Result<(), String> {
    if given {
        Err(format!("{verb} does not take {flag}"))
    } else {
        Ok(())
    }
}
