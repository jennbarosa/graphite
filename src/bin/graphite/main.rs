//! The `graphite` command: `init`, `check`, `compile`, `inspect`, `debug`, `run` and `doctor`.

mod allow;
mod args;
mod check;
mod compile;
mod completions;
mod debug;
mod debugger;
mod doctor;
mod help;
mod host_assets;
mod init;
mod inspect;
mod js;
mod json;
mod load;
mod man;
mod run;
mod scratch;

use std::process::ExitCode;

use args::Command;

/// Why a command stopped. `Reported` means the command already printed its own error, so `main`
/// adds nothing.
pub enum Failure {
    Message(String),
    Reported,
}

impl From<String> for Failure {
    fn from(message: String) -> Self {
        Failure::Message(message)
    }
}

/// Exit status: 0 on success, 1 when a command fails, 2 for a usage error such as an unknown
/// command or option. Scripts can tell a bad invocation from a failed check.
///
/// With `--json`, a failure is also a JSON object on stdout, `{"ok":false,"error":...}`, so a
/// script reads one stream whether the command worked or not.
fn main() -> ExitCode {
    let command = match args::parse(std::env::args().skip(1)) {
        Ok(command) => command,
        Err(message) if message == help::OVERVIEW => {
            eprintln!("{message}");
            return ExitCode::from(2);
        }
        Err(message) => {
            eprintln!("graphite: {message}");
            return ExitCode::from(2);
        }
    };
    let json = matches!(
        command,
        Command::Check { json: true, .. }
            | Command::Inspect { json: true, .. }
            | Command::Doctor { json: true }
    );
    match run(command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(Failure::Reported) => ExitCode::FAILURE,
        Err(Failure::Message(message)) if json => {
            let error = json::object(&[
                ("ok", "false".to_string()),
                ("error", json::quote(&message)),
            ]);
            println!("{error}");
            ExitCode::FAILURE
        }
        Err(Failure::Message(message)) => {
            eprintln!("graphite: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run(command: Command) -> Result<(), Failure> {
    match command {
        Command::Doctor { json } => doctor::run(json),
        Command::Check {
            input,
            front_end,
            allow,
            json,
        } => check::run(&input, &front_end, allow.as_deref(), json),
        Command::Inspect { input, json } => inspect::run(&input, json),
        other => dispatch(other).map_err(Failure::Message),
    }
}

fn dispatch(command: Command) -> Result<(), String> {
    match command {
        Command::Help { topic } => {
            match topic.as_deref() {
                None => println!("{}", help::OVERVIEW),
                Some(name) => println!("{}", help::for_command(name).unwrap_or(help::OVERVIEW)),
            }
            Ok(())
        }
        Command::Version => {
            println!("graphite {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Command::Completions { shell } => {
            let script =
                completions::script(&shell).ok_or_else(|| format!("unknown shell \"{shell}\""))?;
            print!("{script}");
            Ok(())
        }
        Command::Man => {
            print!("{}", man::page());
            Ok(())
        }
        Command::Init { input } => init::run(&input),
        Command::Compile {
            input,
            out,
            front_end,
            allow,
        } => compile::run(&input, out.as_deref(), &front_end, allow.as_deref()),
        Command::Run {
            input,
            state,
            runner,
            allow,
        } => run::run(
            &input,
            state.as_deref(),
            runner.as_deref(),
            allow.as_deref(),
        ),
        Command::Debug {
            input,
            front_end,
            graph,
            out,
            allow,
            commands,
        } => debug::run(
            &input,
            &front_end,
            graph.as_deref(),
            out.as_deref(),
            allow.as_deref(),
            &commands,
        ),
        Command::Doctor { .. } | Command::Check { .. } | Command::Inspect { .. } => {
            unreachable!("handled in run")
        }
    }
}
