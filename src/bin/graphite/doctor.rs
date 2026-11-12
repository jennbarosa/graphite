//! `graphite doctor`: reports this build, what works without outside tools, each registered front
//! end, and whether its `debug` and `run` have the runtime they need on this machine.

use graphite::frontend::registry::{self, Runtime, Target};

use crate::{Failure, js, json};

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// One check on this machine.
enum State {
    /// Works. The text says what was found, and may be empty.
    Ok(String),
    /// Does not work until the text's fix is applied. Counts against the exit status.
    Blocked(String),
    /// Missing, but optional: nothing depends on it.
    Missing(String),
    /// Not provided by this front end. The text says why.
    Unavailable(String),
}

impl State {
    fn status(&self) -> &'static str {
        match self {
            State::Ok(_) => "ok",
            State::Blocked(_) => "blocked",
            State::Missing(_) => "missing",
            State::Unavailable(_) => "unavailable",
        }
    }

    fn text(&self) -> &str {
        match self {
            State::Ok(text)
            | State::Blocked(text)
            | State::Missing(text)
            | State::Unavailable(text) => text,
        }
    }

    /// The text-mode cell: the status word, then its detail.
    fn cell(&self) -> String {
        match self {
            State::Ok(text) if text.is_empty() => "ok".to_string(),
            State::Ok(text) => format!("ok       {text}"),
            State::Blocked(text) => format!("blocked  {text}"),
            State::Missing(text) => format!("missing  {text}"),
            State::Unavailable(text) => format!("not available: {text}"),
        }
    }

    fn json(&self) -> String {
        json::object(&[
            ("status", json::quote(self.status())),
            ("detail", json::quote(self.text())),
        ])
    }
}

struct FrontEnd {
    key: &'static str,
    default: bool,
    compile: State,
    debug: State,
    run: State,
}

struct Report {
    core: State,
    front_ends: Vec<FrontEnd>,
    test_tools: Vec<State>,
}

impl Report {
    /// `front end command` for each command that is blocked, in report order.
    fn blocked(&self) -> Vec<String> {
        let mut blocked = Vec::new();
        for front_end in &self.front_ends {
            for (command, state) in [("debug", &front_end.debug), ("run", &front_end.run)] {
                if matches!(state, State::Blocked(_)) {
                    blocked.push(format!("{} {command}", front_end.key));
                }
            }
        }
        blocked
    }
}

/// The result of checking one runtime, found once per run.
type Probe = Result<String, String>;

fn probe(runtime: Runtime) -> Probe {
    match runtime {
        Runtime::JavaScript => match js::detect() {
            Some((name, version)) => Ok(format!("{name} {version}")),
            None => Err(js::MISSING_HINT.to_string()),
        },
    }
}

/// The state of a command that needs the front end's runtime.
fn runtime_state(target: &Target, probes: &mut Vec<(Runtime, Probe)>) -> State {
    let Some(runtime) = target.runtime else {
        return State::Ok(String::new());
    };
    let result = match probes.iter().find(|(known, _)| *known == runtime) {
        Some((_, result)) => result.clone(),
        None => {
            let result = probe(runtime);
            probes.push((runtime, result.clone()));
            result
        }
    };
    match result {
        Ok(found) => State::Ok(format!("runtime: {found}")),
        Err(hint) => State::Blocked(format!("needs a {runtime:?} runtime: {hint}")),
    }
}

fn tsc_state() -> State {
    let program = std::env::var("GRAPHITE_TSC").unwrap_or_else(|_| "tsc".into());
    match js::version(&program, &["--version"]) {
        Some(found) => State::Ok(format!(
            "tsc for the TypeScript type-check test: {program} {found}"
        )),
        None => State::Missing(
            "tsc for the TypeScript type-check test: install TypeScript 5 or later, or set GRAPHITE_TSC"
                .into(),
        ),
    }
}

fn build() -> Report {
    let mut probes = Vec::new();
    let mut front_ends = Vec::new();
    for target in registry::TARGETS {
        let debug = match target.debugger {
            Some(_) => runtime_state(target, &mut probes),
            None => State::Unavailable("this front end has no debugger".into()),
        };
        let run = if target.key == registry::DURABLE_FRONT_END {
            runtime_state(target, &mut probes)
        } else {
            State::Unavailable(format!(
                "the durable engine is written for {}",
                registry::DURABLE_FRONT_END
            ))
        };
        front_ends.push(FrontEnd {
            key: target.key,
            default: target.key == registry::DEFAULT_FRONT_END,
            compile: State::Ok(String::new()),
            debug,
            run,
        });
    }
    Report {
        core: State::Ok(
            "core: check, inspect, init, compile, and the Rust API need no outside tools".into(),
        ),
        front_ends,
        test_tools: vec![tsc_state()],
    }
}

fn print_text(report: &Report, blocked: &[String]) {
    println!(
        "graphite {VERSION} ({} {})",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    println!();
    println!("{}", report.core.cell());
    println!();
    println!("front ends:");
    for front_end in &report.front_ends {
        let default = if front_end.default { " (default)" } else { "" };
        println!("  {}{default}", front_end.key);
        println!("    {:<8} {}", "compile", front_end.compile.cell());
        println!("    {:<8} {}", "debug", front_end.debug.cell());
        println!("    {:<8} {}", "run", front_end.run.cell());
    }
    println!();
    println!("test tools (only `cargo test` uses these):");
    for tool in &report.test_tools {
        println!("  {}", tool.cell());
    }
    println!();
    if blocked.is_empty() {
        println!("summary: everything graphite offers works on this machine");
    } else {
        println!("summary: blocked: {}", blocked.join(", "));
    }
}

fn print_json(report: &Report, blocked: &[String]) {
    let front_ends: Vec<String> = report
        .front_ends
        .iter()
        .map(|front_end| {
            json::object(&[
                ("key", json::quote(front_end.key)),
                ("default", front_end.default.to_string()),
                ("compile", front_end.compile.json()),
                ("debug", front_end.debug.json()),
                ("run", front_end.run.json()),
            ])
        })
        .collect();
    let test_tools: Vec<String> = report.test_tools.iter().map(State::json).collect();
    let blocked: Vec<String> = blocked.iter().map(|name| json::quote(name)).collect();
    println!(
        "{}",
        json::object(&[
            ("ok", blocked.is_empty().to_string()),
            (
                "graphite",
                json::object(&[
                    ("version", json::quote(VERSION)),
                    ("os", json::quote(std::env::consts::OS)),
                    ("arch", json::quote(std::env::consts::ARCH)),
                ]),
            ),
            ("core", report.core.json()),
            ("front_ends", json::array(&front_ends)),
            ("test_tools", json::array(&test_tools)),
            ("blocked", json::array(&blocked)),
        ])
    );
}

pub fn run(json: bool) -> Result<(), Failure> {
    let report = build();
    let blocked = report.blocked();
    if json {
        print_json(&report, &blocked);
    } else {
        print_text(&report, &blocked);
    }
    match (blocked.is_empty(), json) {
        (true, _) => Ok(()),
        (false, true) => Err(Failure::Reported),
        (false, false) => Err(Failure::Message(
            "some commands cannot run here; fix the lines marked blocked".into(),
        )),
    }
}
