//! `graphite doctor`: reports this build's version, what works without outside tools, each
//! registered front end, and the runtime that each front end's `debug` and `run` need here.

use graphite::frontend::registry::{self, Runtime, Target};

use crate::js;

/// The result of checking one runtime: its name and version, or the fix when it is missing.
type Probe = Result<String, String>;

fn probe(runtime: Runtime) -> Probe {
    match runtime {
        Runtime::JavaScript => match js::detect() {
            Some((name, version)) => Ok(format!("{name} {version}")),
            None => Err(js::MISSING_HINT.to_string()),
        },
    }
}

/// Checks one runtime, once per run, and reports the result for `command` of `target`.
fn report(
    command: &str,
    target: &Target,
    cache: &mut Vec<(Runtime, Probe)>,
    blocked: &mut Vec<String>,
) {
    let Some(runtime) = target.runtime else {
        println!("    {command:<8} ok");
        return;
    };
    let result = match cache.iter().find(|(known, _)| *known == runtime) {
        Some((_, result)) => result.clone(),
        None => {
            let result = probe(runtime);
            cache.push((runtime, result.clone()));
            result
        }
    };
    match result {
        Ok(found) => println!("    {command:<8} ok       runtime: {found}"),
        Err(hint) => {
            println!("    {command:<8} blocked  needs a {runtime:?} runtime: {hint}");
            blocked.push(format!("{} {command}", target.key));
        }
    }
}

fn test_tools() {
    println!("test tools (only `cargo test` uses these):");
    let program = std::env::var("GRAPHITE_TSC").unwrap_or_else(|_| "tsc".into());
    match js::version(&program, &["--version"]) {
        Some(found) => {
            println!("  ok       tsc for the TypeScript type-check test: {program} {found}")
        }
        None => println!(
            "  missing  tsc for the TypeScript type-check test: install TypeScript 5 or later, or set GRAPHITE_TSC"
        ),
    }
}

pub fn run() -> Result<(), String> {
    println!(
        "graphite {} ({} {})",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    println!();
    println!(
        "ok       core: check, inspect, init, compile, and the Rust API need no outside tools"
    );
    println!();

    println!("front ends:");
    let mut cache: Vec<(Runtime, Probe)> = Vec::new();
    let mut blocked: Vec<String> = Vec::new();
    for target in registry::TARGETS {
        let default = if target.key == registry::DEFAULT_FRONT_END {
            " (default)"
        } else {
            ""
        };
        println!("  {}{default}", target.key);
        println!("    compile  ok");
        if target.debugger.is_some() {
            report("debug", target, &mut cache, &mut blocked);
        } else {
            println!("    debug    not available: this front end has no debugger");
        }
        if target.key == registry::DURABLE_FRONT_END {
            report("run", target, &mut cache, &mut blocked);
        } else {
            println!(
                "    run      not available: the durable host is written for {}",
                registry::DURABLE_FRONT_END
            );
        }
    }
    println!();

    test_tools();
    println!();

    if blocked.is_empty() {
        println!("summary: everything graphite offers works on this machine");
        Ok(())
    } else {
        println!("summary: blocked: {}", blocked.join(", "));
        Err("some commands cannot run here; fix the lines marked blocked".into())
    }
}
