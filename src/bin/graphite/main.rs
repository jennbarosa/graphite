//! The `graphite` command: `compile`, `check`, `inspect` and `debug` a DSL file.

mod allow;
mod args;
mod check;
mod compile;
mod debug;
mod debugger;
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
        Command::Compile {
            input,
            out,
            target,
            allow,
        } => compile::run(&input, &out, &target, allow.as_deref()),
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
