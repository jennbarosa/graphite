//! Runs a bundled TypeScript script on a JavaScript runtime.
//!
//! The runtime is `GRAPHITE_JS_RUNNER` when it is set, split on whitespace (for example
//! `npx tsx`). Otherwise it is the first of `bun` and `npx tsx` found on the path.

use std::ffi::OsString;
use std::io;
use std::path::Path;
use std::process::{Command, ExitStatus};

/// What to do when no JavaScript runtime is found.
pub const MISSING_HINT: &str =
    "install bun (https://bun.sh), or npm and tsx, or set GRAPHITE_JS_RUNNER";

/// The JavaScript runtime `launch` would use: its command and its first version line. `None`
/// when none can start. Mirrors the order `launch` tries.
pub fn detect() -> Option<(String, String)> {
    if let Ok(configured) = std::env::var("GRAPHITE_JS_RUNNER") {
        let mut words = configured.split_whitespace();
        let program = words.next()?;
        let mut args: Vec<&str> = words.collect();
        args.push("--version");
        return version(program, &args)
            .map(|found| (format!("GRAPHITE_JS_RUNNER ({configured})"), found));
    }
    if let Some(found) = version("bun", &["--version"]) {
        return Some(("bun".into(), found));
    }
    // `--no-install` keeps `npx` from downloading tsx during a check.
    version("npx", &["--no-install", "tsx", "--version"]).map(|found| ("npx tsx".into(), found))
}

/// Runs `program args` and returns the first line it prints, or `None` when it cannot run.
pub fn version(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    text.lines().next().map(|line| line.trim().to_string())
}

/// Runs `script` with `args` and returns its exit status.
pub fn launch(script: &Path, args: &[OsString]) -> Result<ExitStatus, String> {
    match std::env::var("GRAPHITE_JS_RUNNER") {
        Ok(runner) => run_configured(&runner, script, args),
        Err(_) => run_default(script, args),
    }
}

fn run_configured(runner: &str, script: &Path, args: &[OsString]) -> Result<ExitStatus, String> {
    let mut words = runner.split_whitespace();
    let program = words
        .next()
        .ok_or("GRAPHITE_JS_RUNNER is empty; name a command, such as `bun` or `npx tsx`")?;
    let extra: Vec<&str> = words.collect();
    spawn(program, &extra, script, args)
        .map_err(|error| format!("cannot start GRAPHITE_JS_RUNNER `{runner}`: {error}"))
}

fn run_default(script: &Path, args: &[OsString]) -> Result<ExitStatus, String> {
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
    Err(format!("no JavaScript runtime found: {MISSING_HINT}"))
}

fn spawn(
    program: &str,
    extra: &[&str],
    script: &Path,
    args: &[OsString],
) -> io::Result<ExitStatus> {
    Command::new(program)
        .args(extra)
        .arg(script)
        .args(args)
        .status()
}
