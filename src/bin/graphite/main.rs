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

fn main() -> ExitCode {
    match args::parse(std::env::args().skip(1)).and_then(run) {
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
        Command::Init { input } => init::run(&input),
        Command::Doctor => doctor::run(),
        Command::Compile {
            input,
            out,
            target,
            allow,
        } => compile::run(&input, out.as_deref(), &target, allow.as_deref()),
        Command::Check {
            input,
            target,
            allow,
        } => check::run(&input, &target, allow.as_deref()),
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
            target,
            graph,
            out,
            allow,
            commands,
        } => debug::run(
            &input,
            &target,
            graph.as_deref(),
            out.as_deref(),
            allow.as_deref(),
            &commands,
        ),
    }
}
