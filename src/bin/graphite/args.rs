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
        /// `None` lets the file's name, directives and `--pure` pick the front end.
        front_end: Option<String>,
        allow: Option<PathBuf>,
        pure: bool,
    },
    Check {
        input: PathBuf,
        front_end: Option<String>,
        allow: Option<PathBuf>,
        pure: bool,
        json: bool,
    },
    Inspect {
        input: PathBuf,
        json: bool,
        pure: bool,
    },
    Debug {
        input: PathBuf,
        front_end: String,
        graph: Option<String>,
        out: Option<PathBuf>,
        allow: Option<PathBuf>,
        /// Each entry is one debugger command line, run in order.
        commands: Vec<String>,
        /// Each entry is `<task>=<outcomes>`: a stub for the durable engine's run of the graph.
        simulate: Vec<String>,
    },
    /// Runs the first graph under the durable engine. `runner` names a module whose default
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
        /// `--each`: the item graph, run once per item the graph produces.
        each: Option<PathBuf>,
        /// `--key`: the item field that names each item. Needed with `--each`.
        key: Option<String>,
        /// `--seen`: the file of item keys already started.
        seen: Option<PathBuf>,
        /// `--until-passed`: stop starting items after this many pass.
        until_passed: Option<u32>,
        /// `--retry`: the key of a stored task error to clear, so the run runs that task again.
        retry: Option<String>,
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
    pure: bool,
    graph: Option<String>,
    allow: Option<PathBuf>,
    state: Option<PathBuf>,
    runner: Option<PathBuf>,
    json: bool,
    input_data: Option<PathBuf>,
    tasks: Option<PathBuf>,
    each: Option<PathBuf>,
    key: Option<String>,
    seen: Option<PathBuf>,
    until_passed: Option<u32>,
    retry: Option<String>,
    simulate: Vec<String>,
}

pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Command, String> {
    let args: Vec<String> = args.into_iter().collect();
    let Some(verb) = args.first().cloned() else {
        return Err(help::OVERVIEW.to_string());
    };
    if let Some(command) = parse_standalone(&verb, &args) {
        return command;
    }
    if args[1..].iter().any(|arg| arg == "--help" || arg == "-h") {
        return Ok(Command::Help { topic: Some(verb) });
    }
    if help::for_command(&verb).is_none() {
        return Err(format!("unknown command \"{verb}\"\n\n{}", help::OVERVIEW));
    }
    let (options, positionals) = parse_options(&args[1..])?;
    let mut positionals = positionals.into_iter();
    let input = PathBuf::from(
        positionals
            .next()
            .ok_or_else(|| format!("{verb} needs a file\n\n{}", help::OVERVIEW))?,
    );
    let rest: Vec<String> = positionals.collect();
    command(&verb, input, &rest, options)
}

/// The commands that take no file, each with its own argument rules. `None` for every other verb.
fn parse_standalone(verb: &str, args: &[String]) -> Option<Result<Command, String>> {
    let command = match verb {
        "--version" | "-V" if args.len() > 1 => {
            Err(format!("{verb} takes no arguments, got \"{}\"", args[1]))
        }
        "--version" | "-V" => Ok(Command::Version),
        "help" | "--help" | "-h" => help_command(args),
        "completions" => completions_command(args),
        "man" => man_command(args),
        "doctor" => doctor_command(args),
        _ => return None,
    };
    Some(command)
}

fn help_command(args: &[String]) -> Result<Command, String> {
    if args.len() > 2 {
        return Err("help takes at most one command name".into());
    }
    let topic = args.get(1).cloned();
    if let Some(name) = &topic
        && help::for_command(name).is_none()
    {
        return Err(format!("no help for \"{name}\"\n\n{}", help::OVERVIEW));
    }
    Ok(Command::Help { topic })
}

fn completions_command(args: &[String]) -> Result<Command, String> {
    match args.get(1).map(String::as_str) {
        Some("--help" | "-h") => Ok(Command::Help {
            topic: Some("completions".into()),
        }),
        Some(shell @ ("bash" | "zsh" | "fish")) if args.len() == 2 => Ok(Command::Completions {
            shell: shell.to_string(),
        }),
        Some(other) if args.len() == 2 => Err(format!(
            "unknown shell \"{other}\"; expected bash, zsh or fish"
        )),
        _ => Err("completions needs a shell: bash, zsh or fish".into()),
    }
}

fn man_command(args: &[String]) -> Result<Command, String> {
    match args.get(1).map(String::as_str) {
        None => Ok(Command::Man),
        Some("--help" | "-h") => Ok(Command::Help {
            topic: Some("man".into()),
        }),
        Some(_) => Err(format!("man takes no arguments, got \"{}\"", args[1])),
    }
}

fn doctor_command(args: &[String]) -> Result<Command, String> {
    match args.get(1).map(String::as_str) {
        None => Ok(Command::Doctor { json: false }),
        Some("--json") if args.len() == 2 => Ok(Command::Doctor { json: true }),
        Some(_) => Err(format!(
            "doctor takes no arguments except --json, got \"{}\"",
            args[1..].join(" ")
        )),
    }
}

/// Reads the options and the positional arguments. An option takes the next word as its value.
fn parse_options(args: &[String]) -> Result<(Options, Vec<String>), String> {
    let mut options = Options::default();
    let mut positionals = Vec::new();
    let mut words = args.iter().cloned();
    while let Some(arg) = words.next() {
        match arg.as_str() {
            "--out" => options.out = Some(PathBuf::from(value(&mut words, "--out")?)),
            "--frontend" => options.front_end = Some(value(&mut words, "--frontend")?),
            "--pure" => options.pure = true,
            "--graph" => options.graph = Some(value(&mut words, "--graph")?),
            "--allow" => options.allow = Some(PathBuf::from(value(&mut words, "--allow")?)),
            "--state" => options.state = Some(PathBuf::from(value(&mut words, "--state")?)),
            "--runner" => options.runner = Some(PathBuf::from(value(&mut words, "--runner")?)),
            "--json" => options.json = true,
            "--input" => options.input_data = Some(PathBuf::from(value(&mut words, "--input")?)),
            "--tasks" => options.tasks = Some(PathBuf::from(value(&mut words, "--tasks")?)),
            "--each" => options.each = Some(PathBuf::from(value(&mut words, "--each")?)),
            "--key" => options.key = Some(value(&mut words, "--key")?),
            "--seen" => options.seen = Some(PathBuf::from(value(&mut words, "--seen")?)),
            "--until-passed" => {
                let text = value(&mut words, "--until-passed")?;
                let count = text
                    .parse::<u32>()
                    .map_err(|_| format!("--until-passed needs a whole number, got {text}"))?;
                options.until_passed = Some(count);
            }
            "--retry" => options.retry = Some(value(&mut words, "--retry")?),
            "--simulate" => options.simulate.push(value(&mut words, "--simulate")?),
            flag if flag.starts_with("--") => return Err(format!("unknown option {flag}")),
            _ => positionals.push(arg),
        }
    }
    Ok((options, positionals))
}

/// The options each command accepts. Any other option given to that command is refused.
fn accepted_flags(verb: &str) -> &'static [&'static str] {
    match verb {
        "init" => &[],
        "compile" => &["--out", "--frontend", "--allow", "--pure"],
        "check" => &["--frontend", "--allow", "--json", "--pure"],
        "inspect" => &["--json", "--pure"],
        "run" => &[
            "--allow",
            "--state",
            "--runner",
            "--input",
            "--tasks",
            "--each",
            "--key",
            "--seen",
            "--until-passed",
            "--retry",
        ],
        "debug" => &["--frontend", "--graph", "--out", "--allow", "--simulate"],
        _ => &[],
    }
}

/// The option flags that were given, in a fixed order.
fn given_flags(options: &Options) -> Vec<&'static str> {
    [
        ("--out", options.out.is_some()),
        ("--frontend", options.front_end.is_some()),
        ("--pure", options.pure),
        ("--graph", options.graph.is_some()),
        ("--allow", options.allow.is_some()),
        ("--state", options.state.is_some()),
        ("--runner", options.runner.is_some()),
        ("--json", options.json),
        ("--input", options.input_data.is_some()),
        ("--tasks", options.tasks.is_some()),
        ("--each", options.each.is_some()),
        ("--key", options.key.is_some()),
        ("--seen", options.seen.is_some()),
        ("--until-passed", options.until_passed.is_some()),
        ("--retry", options.retry.is_some()),
        ("--simulate", !options.simulate.is_empty()),
    ]
    .into_iter()
    .filter_map(|(flag, given)| given.then_some(flag))
    .collect()
}

/// Builds the command for `verb` from its file and options, refusing any option it does not accept.
fn command(
    verb: &str,
    input: PathBuf,
    rest: &[String],
    options: Options,
) -> Result<Command, String> {
    let accepted = accepted_flags(verb);
    if let Some(flag) = given_flags(&options)
        .into_iter()
        .find(|flag| !accepted.contains(flag))
    {
        return Err(format!("{verb} does not take {flag}"));
    }
    match verb {
        "init" => {
            only_file(rest, verb)?;
            Ok(Command::Init { input })
        }
        "compile" => {
            only_file(rest, verb)?;
            Ok(Command::Compile {
                input,
                out: options.out,
                front_end: options.front_end,
                allow: options.allow,
                pure: options.pure,
            })
        }
        "check" => {
            only_file(rest, verb)?;
            Ok(Command::Check {
                input,
                front_end: options.front_end,
                allow: options.allow,
                pure: options.pure,
                json: options.json,
            })
        }
        "inspect" => {
            only_file(rest, verb)?;
            Ok(Command::Inspect {
                input,
                json: options.json,
                pure: options.pure,
            })
        }
        "run" => {
            only_file(rest, verb)?;
            check_run_options(&options)?;
            Ok(Command::Run {
                input,
                state: options.state,
                runner: options.runner,
                allow: options.allow,
                input_data: options.input_data,
                tasks: options.tasks,
                each: options.each,
                key: options.key,
                seen: options.seen,
                until_passed: options.until_passed,
                retry: options.retry,
            })
        }
        "debug" => {
            if !options.simulate.is_empty() && !rest.is_empty() {
                return Err(
                    "debug --simulate takes no commands: each stub decides its task's outcome"
                        .into(),
                );
            }
            Ok(Command::Debug {
                input,
                front_end: options
                    .front_end
                    .unwrap_or_else(|| registry::DEFAULT_FRONT_END.to_string()),
                graph: options.graph,
                out: options.out,
                allow: options.allow,
                commands: rest.to_vec(),
                simulate: options.simulate,
            })
        }
        _ => Err(help::OVERVIEW.to_string()),
    }
}

/// The rules among `run`'s own options: one source of task code, and the fan-out flags together.
fn check_run_options(options: &Options) -> Result<(), String> {
    if options.runner.is_some() && options.tasks.is_some() {
        return Err("--runner and --tasks both name the task code: pass one".into());
    }
    if options.each.is_some() != options.key.is_some() {
        return Err(
            "--each and --key go together: each item is run by --each, named by --key".into(),
        );
    }
    if options.each.is_none() && (options.seen.is_some() || options.until_passed.is_some()) {
        return Err("--seen and --until-passed apply to --each only".into());
    }
    if options.each.is_some() && options.retry.is_some() {
        return Err("--retry clears a task in one run, so it does not apply to --each".into());
    }
    Ok(())
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
