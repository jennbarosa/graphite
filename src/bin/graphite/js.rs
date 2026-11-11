//! Runs a bundled TypeScript script on a JavaScript runtime.
//!
//! The runtime is `GRAPHITE_JS_RUNNER` when it is set, split on whitespace (for example
//! `npx tsx`). Otherwise it is the first of `bun` and `npx tsx` found on the path.

use std::ffi::OsString;
use std::io;
use std::process::{Command, ExitStatus};

/// Runs `script` with `args` and returns its exit status.
pub fn launch(script: &str, args: &[OsString]) -> Result<ExitStatus, String> {
    match std::env::var("GRAPHITE_JS_RUNNER") {
        Ok(runner) => run_configured(&runner, script, args),
        Err(_) => run_default(script, args),
    }
}

fn run_configured(runner: &str, script: &str, args: &[OsString]) -> Result<ExitStatus, String> {
    let mut words = runner.split_whitespace();
    let program = words
        .next()
        .ok_or("GRAPHITE_JS_RUNNER is empty; name a command, such as `bun` or `npx tsx`")?;
    let extra: Vec<&str> = words.collect();
    spawn(program, &extra, script, args)
        .map_err(|error| format!("cannot start GRAPHITE_JS_RUNNER `{runner}`: {error}"))
}

fn run_default(script: &str, args: &[OsString]) -> Result<ExitStatus, String> {
    let candidates: [(&str, &[&str]); 2] = [("bun", &[]), ("npx", &["tsx"])];
    for (program, extra) in candidates {
        match spawn(program, extra, script, args) {
            Ok(status) => return Ok(status),
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(format!("cannot start `{program}`: {error}"));
            }
        }
    }
    Err(
        "no JavaScript runtime found: install bun, or npm and tsx, or set GRAPHITE_JS_RUNNER"
            .into(),
    )
}

fn spawn(program: &str, extra: &[&str], script: &str, args: &[OsString]) -> io::Result<ExitStatus> {
    Command::new(program)
        .args(extra)
        .arg(script)
        .args(args)
        .status()
}
