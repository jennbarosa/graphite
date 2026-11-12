//! Parses the command line into a [`Command`]. Rejects options a command does not take.

use std::path::PathBuf;

use graphite::frontend::registry;

use crate::help;

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    /// `--version` or `-V`.
    Version,
    Init {
        input: PathBuf,
    },
    Compile {
        input: PathBuf,
        /// `None` means `graphite-out/<file stem>`, resolved by the command.
        out: Option<PathBuf>,
        front_end: String,
        allow: Option<PathBuf>,
    },
    Check {
        input: PathBuf,
        front_end: String,
        allow: Option<PathBuf>,
        json: bool,
    },
    Inspect {
        input: PathBuf,
        json: bool,
    },
    Debug {
        input: PathBuf,
        front_end: String,
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
        /// `--input`: a JSON file whose value every task receives as `context.input`.
        input_data: Option<PathBuf>,
        /// `--tasks`: a `tasks.toml` mapping each task to the command that runs it.
        tasks: Option<PathBuf>,
    },
    Doctor {
        json: bool,
    },
    /// `graphite completions <shell>`: the shell is `bash`, `zsh` or `fish`.
    Completions {
        shell: String,
    },
    /// `graphite man`: the manual page, in roff.
    Man,
    /// `graphite help [command]`, or `--help` after any command.
    Help {
        topic: Option<String>,
    },
}

/// Options as written, before each command decides which ones it accepts.
#[derive(Default)]
struct Options {
    out: Option<PathBuf>,
    front_end: Option<String>,
    graph: Option<String>,
    allow: Option<PathBuf>,
    durable: bool,
    state: Option<PathBuf>,
    runner: Option<PathBuf>,
    json: bool,
    input_data: Option<PathBuf>,
    tasks: Option<PathBuf>,
}

pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Command, String> {
    let args: Vec<String> = args.into_iter().collect();
    let Some(verb) = args.first().cloned() else {
        return Err(help::OVERVIEW.to_string());
    };
    match verb.as_str() {
        "--version" | "-V" => {
            if args.len() > 1 {
                return Err(format!("{verb} takes no arguments, got \"{}\"", args[1]));
            }
            return Ok(Command::Version);
        }
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
        "completions" => {
            return match args.get(1).map(String::as_str) {
                Some("--help" | "-h") => Ok(Command::Help { topic: Some(verb) }),
                Some(shell @ ("bash" | "zsh" | "fish")) if args.len() == 2 => {
                    Ok(Command::Completions {
                        shell: shell.to_string(),
                    })
                }
                Some(other) if args.len() == 2 => Err(format!(
                    "unknown shell \"{other}\"; expected bash, zsh or fish"
                )),
                _ => Err("completions needs a shell: bash, zsh or fish".into()),
            };
        }
        "man" => {
            return match args.get(1).map(String::as_str) {
                None => Ok(Command::Man),
                Some("--help" | "-h") => Ok(Command::Help { topic: Some(verb) }),
                Some(_) => Err(format!("man takes no arguments, got \"{}\"", args[1])),
            };
        }
        "doctor" => {
            return match args.get(1).map(String::as_str) {
                None => Ok(Command::Doctor { json: false }),
                Some("--json") if args.len() == 2 => Ok(Command::Doctor { json: true }),
                Some(_) => Err(format!(
                    "doctor takes no arguments except --json, got \"{}\"",
                    args[1..].join(" ")
                )),
            };
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
            "--frontend" => options.front_end = Some(value(&mut rest_args, "--frontend")?),
            "--graph" => options.graph = Some(value(&mut rest_args, "--graph")?),
            "--allow" => options.allow = Some(PathBuf::from(value(&mut rest_args, "--allow")?)),
            "--state" => options.state = Some(PathBuf::from(value(&mut rest_args, "--state")?)),
            "--runner" => options.runner = Some(PathBuf::from(value(&mut rest_args, "--runner")?)),
            "--durable" => options.durable = true,
            "--json" => options.json = true,
            "--input" => {
                options.input_data = Some(PathBuf::from(value(&mut rest_args, "--input")?))
            }
            "--tasks" => options.tasks = Some(PathBuf::from(value(&mut rest_args, "--tasks")?)),
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
        front_end,
        graph,
        allow,
        durable,
        state,
        runner,
        json,
        input_data,
        tasks,
    } = options;
    let front_end_given = front_end.is_some();
    if verb != "run" {
        refuse(input_data.is_some(), "--input", &verb)?;
        refuse(tasks.is_some(), "--tasks", &verb)?;
    }
    if json && !matches!(verb.as_str(), "check" | "inspect") {
        return Err(format!("{verb} does not take --json"));
    }
    let front_end = front_end.unwrap_or_else(|| registry::DEFAULT_FRONT_END.to_string());
    match verb.as_str() {
        "init" => {
            only_file(&rest, &verb)?;
            refuse(out.is_some(), "--out", &verb)?;
            refuse(graph.is_some(), "--graph", &verb)?;
            refuse(front_end_given, "--frontend", &verb)?;
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
                front_end,
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
                front_end,
                allow,
                json,
            })
        }
        "inspect" => {
            only_file(&rest, &verb)?;
            refuse(out.is_some(), "--out", &verb)?;
            refuse(graph.is_some(), "--graph", &verb)?;
            refuse(front_end_given, "--frontend", &verb)?;
            refuse(allow.is_some(), "--allow", &verb)?;
            refuse_run_flags(durable, &state, &runner, &verb)?;
            Ok(Command::Inspect { input, json })
        }
        "run" => {
            only_file(&rest, &verb)?;
            refuse(out.is_some(), "--out", &verb)?;
            refuse(graph.is_some(), "--graph", &verb)?;
            refuse(front_end_given, "--frontend", &verb)?;
            if runner.is_some() && tasks.is_some() {
                return Err("--runner and --tasks both name the task code: pass one".into());
            }
            Ok(Command::Run {
                input,
                state,
                runner,
                allow,
                input_data,
                tasks,
            })
        }
        "debug" => {
            refuse_run_flags(durable, &state, &runner, &verb)?;
            Ok(Command::Debug {
                input,
                front_end,
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
