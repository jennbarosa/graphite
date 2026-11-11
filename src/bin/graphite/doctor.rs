//! `graphite doctor`: reports the JavaScript runtime and TypeScript compiler that commands will use.

use std::process::Command;

/// Runs `program args` and returns the first line it prints, or `None` when it cannot run.
fn version(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    text.lines().next().map(|line| line.trim().to_string())
}

/// The JavaScript runtime commands will use, in the order `js.rs` tries them.
fn runtime() -> Option<(String, String)> {
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

/// The TypeScript compiler the tests use: `GRAPHITE_TSC`, or `tsc` on the path.
fn typescript() -> Option<(String, String)> {
    let program = std::env::var("GRAPHITE_TSC").unwrap_or_else(|_| "tsc".into());
    version(&program, &["--version"]).map(|found| (program, found))
}

pub fn run() -> Result<(), String> {
    let mut missing_runtime = false;
    match runtime() {
        Some((name, found)) => println!("ok       JavaScript runtime: {name} {found}"),
        None => {
            missing_runtime = true;
            println!(
                "missing  JavaScript runtime: install bun (https://bun.sh), or npm and tsx, or set GRAPHITE_JS_RUNNER"
            );
        }
    }
    match typescript() {
        Some((program, found)) => println!("ok       TypeScript: {program} {found}"),
        None => println!(
            "missing  TypeScript: optional; only the test suite needs tsc 5 or later. Set GRAPHITE_TSC or put tsc on PATH"
        ),
    }
    if missing_runtime {
        Err("no JavaScript runtime found; `run`, `debug` and the tests need one".into())
    } else {
        Ok(())
    }
}
