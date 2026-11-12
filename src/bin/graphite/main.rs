//! The `graphite` command: `init`, `check`, `compile`, `inspect`, `debug`, `run` and `doctor`.

mod allow;
mod args;
mod check;
mod compile;
mod debug;
mod debugger;
mod doctor;
mod help;
mod host_assets;
mod init;
mod inspect;
mod js;
mod load;
mod run;
mod scratch;

use std::process::ExitCode;

use args::Command;

/// Exit status: 0 on success, 1 when a command fails, 2 for a usage error such as an unknown
/// command or option. Scripts can tell a bad invocation from a failed check.
fn main() -> ExitCode {
    let command = match args::parse(std::env::args().skip(1)) {
        Ok(command) => command,
        Err(message) => {
            eprintln!("graphite: {message}");
            return ExitCode::from(2);
        }
    };
    match run(command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("graphite: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run(command: Command) -> Result<(), String> {
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
        Command::Init { input } => init::run(&input),
        Command::Doctor => doctor::run(),
        Command::Compile {
            input,
            out,
            front_end,
            allow,
        } => compile::run(&input, out.as_deref(), &front_end, allow.as_deref()),
        Command::Check {
            input,
            front_end,
            allow,
        } => check::run(&input, &front_end, allow.as_deref()),
        Command::Inspect { input } => inspect::run(&input),
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
    }
}
