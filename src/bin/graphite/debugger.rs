//! Runs a front end's debugger script with a JavaScript runtime, on one generated module.

use std::ffi::OsString;
use std::path::Path;

use crate::js;

pub fn launch(script: &Path, module: &Path, commands: &[String]) -> Result<(), String> {
    let mut args = vec![module.as_os_str().to_owned()];
    args.extend(commands.iter().map(OsString::from));
    let status = js::launch(script, &args)?;
    exit_status(status)
}

/// Runs a compiled debugger program with `commands`, its output going straight to the terminal.
pub fn run_program(program: &Path, commands: &[String]) -> Result<(), String> {
    let status = std::process::Command::new(program)
        .args(commands)
        .status()
        .map_err(|error| format!("cannot start the debugger: {error}"))?;
    exit_status(status)
}

fn exit_status(status: std::process::ExitStatus) -> Result<(), String> {
    if status.success() {
        Ok(())
    } else {
        Err(format!("debugger exited with {status}"))
    }
}
