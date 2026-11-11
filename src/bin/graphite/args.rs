//! Parses the command line into a [`Command`]. Rejects options a command does not take.

use std::path::PathBuf;

use crate::help;

const DEFAULT_TARGET: &str = "typescript";

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Init {
        input: PathBuf,
    },
    Compile {
        input: PathBuf,
        /// `None` means `graphite-out/<file stem>`, resolved by the command.
        out: Option<PathBuf>,
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
    Doctor,
    /// `graphite help [command]`, or `--help` after any command.
    Help {
        topic: Option<String>,
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
    let args: Vec<String> = args.into_iter().collect();
    let Some(verb) = args.first().cloned() else {
        return Err(help::OVERVIEW.to_string());
    };
    match verb.as_str() {
        "help" | "--help" | "-h" => {
            if args.len() > 2 {
                return Err("help takes at most one command name".into());
            }
            let topic = args.get(1).cloned();
            if let Some(name) = &topic
                && help::for_command(name).is_none()
            {
                return Err(format!("no help for \"{name}\"\n\n{}", help::OVERVIEW));
            }
            return Ok(Command::Help { topic });
        }
        "doctor" => {
            if args.len() > 1 {
                return Err(format!("doctor takes no arguments, got \"{}\"", args[1]));
            }
            return Ok(Command::Doctor);
        }
        _ => {}
    }
    if args[1..].iter().any(|arg| arg == "--help" || arg == "-h") {
        return Ok(Command::Help { topic: Some(verb) });
    }
    if help::for_command(&verb).is_none() {
        return Err(format!("unknown command \"{verb}\"\n\n{}", help::OVERVIEW));
    }
    let mut options = Options::default();
    let mut positionals = Vec::new();
    let mut rest_args = args[1..].iter().cloned();
    while let Some(arg) = rest_args.next() {
        match arg.as_str() {
            "--out" => options.out = Some(PathBuf::from(value(&mut rest_args, "--out")?)),
            "--target" => options.target = Some(value(&mut rest_args, "--target")?),
            "--graph" => options.graph = Some(value(&mut rest_args, "--graph")?),
            "--allow" => options.allow = Some(PathBuf::from(value(&mut rest_args, "--allow")?)),
            "--state" => options.state = Some(PathBuf::from(value(&mut rest_args, "--state")?)),
            "--runner" => options.runner = Some(PathBuf::from(value(&mut rest_args, "--runner")?)),
            "--durable" => options.durable = true,
            flag if flag.starts_with("--") => return Err(format!("unknown option {flag}")),
            _ => positionals.push(arg),
        }
    }
    let mut positionals = positionals.into_iter();
    let input = PathBuf::from(
        positionals
            .next()
            .ok_or_else(|| format!("{verb} needs a file\n\n{}", help::OVERVIEW))?,
    );
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
        "init" => {
            only_file(&rest, &verb)?;
            refuse(out.is_some(), "--out", &verb)?;
            refuse(graph.is_some(), "--graph", &verb)?;
            refuse(target_given, "--target", &verb)?;
            refuse(allow.is_some(), "--allow", &verb)?;
            refuse_run_flags(durable, &state, &runner, &verb)?;
            Ok(Command::Init { input })
        }
        "compile" => {
            only_file(&rest, &verb)?;
            refuse(graph.is_some(), "--graph", &verb)?;
            refuse_run_flags(durable, &state, &runner, &verb)?;
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
        _ => Err(help::OVERVIEW.to_string()),
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

/// `--durable`, `--state` and `--runner` belong to `run` alone (`--durable` is accepted there).
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

#[cfg(test)]
mod tests {
    use super::{Command, parse};

    fn args(list: &[&str]) -> Result<Command, String> {
        parse(list.iter().map(|arg| arg.to_string()))
    }

    #[test]
    fn help_is_available_as_a_command_a_flag_and_per_command() {
        assert_eq!(args(&["--help"]), Ok(Command::Help { topic: None }));
        assert_eq!(
            args(&["help", "run"]),
            Ok(Command::Help {
                topic: Some("run".into())
            })
        );
        assert_eq!(
            args(&["compile", "x.dsl", "--help"]),
            Ok(Command::Help {
                topic: Some("compile".into())
            })
        );
        assert!(args(&["help", "nope"]).is_err());
    }

    #[test]
    fn run_is_durable_without_the_flag_and_the_flag_is_accepted() {
        let plain = args(&["run", "x.dsl"]).unwrap();
        let flagged = args(&["run", "--durable", "x.dsl"]).unwrap();
        assert_eq!(plain, flagged);
    }

    #[test]
    fn compile_out_is_optional() {
        assert!(matches!(
            args(&["compile", "x.dsl"]),
            Ok(Command::Compile { out: None, .. })
        ));
    }
}
