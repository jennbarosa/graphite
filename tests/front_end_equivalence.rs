//! Every front end with a debugger must behave like the TypeScript runtime, which is the spec.
//! Each bundled graph runs the same commands under every front end, and the debugger output must
//! match the TypeScript output line for line. The command sequences are fixed for the examples
//! and seeded at random for everything else, so a failure is reproducible.

#[allow(dead_code)]
#[path = "support/cli.rs"]
mod cli;

#[path = "support/rng.rs"]
mod rng;

use rng::Lcg;
use std::path::PathBuf;

use cli::{debugging_front_ends, graphite, manifest, stderr, stdout};
use graphite::frontend::registry::DEFAULT_FRONT_END;

/// One graph under test, with the vocabulary the random sequences draw from.
struct Case {
    dsl: PathBuf,
    allow: Option<PathBuf>,
    conditions: &'static [&'static str],
    choices: &'static [&'static str],
}

fn cases() -> Vec<Case> {
    let root = manifest();
    vec![
        Case {
            dsl: root.join("examples/door.graphite"),
            allow: None,
            conditions: &["request", "timeout"],
            choices: &[],
        },
        Case {
            dsl: root.join("examples/walk.graphite"),
            allow: None,
            conditions: &["keyup"],
            choices: &["facing"],
        },
        Case {
            dsl: root.join("examples/order.graphite"),
            allow: None,
            conditions: &["payment", "stock", "carrier", "returned", "cancelled"],
            choices: &[],
        },
        Case {
            dsl: root.join("tests/fixtures/runtime/cycle.graphite"),
            allow: None,
            conditions: &["x", "out"],
            choices: &[],
        },
        Case {
            dsl: root.join("tests/fixtures/runtime/stage_order.graphite"),
            allow: Some(root.join("tests/fixtures/runtime/stage_order.toml")),
            conditions: &["passed", "failed"],
            choices: &[],
        },
    ]
}

/// Debugger output for `commands` on `case` under `front_end`.
fn debug(front_end: &str, case: &Case, commands: &[String]) -> String {
    let mut args: Vec<std::ffi::OsString> = vec![
        "debug".into(),
        case.dsl.clone().into_os_string(),
        "--frontend".into(),
        front_end.into(),
    ];
    if let Some(allow) = &case.allow {
        args.extend(["--allow".into(), allow.clone().into_os_string()]);
    }
    args.extend(commands.iter().map(Into::into));
    let output = graphite(args);
    assert!(
        output.status.success(),
        "{front_end} on {}: {}",
        case.dsl.display(),
        stderr(&output)
    );
    stdout(&output)
}

/// Runs `commands` under every debugging front end and asserts each one matches the TypeScript
/// output. Returns that output.
fn assert_every_front_end_matches_the_spec(case: &Case, commands: &[String]) -> String {
    let spec = debug(DEFAULT_FRONT_END, case, commands);
    assert!(
        !spec.is_empty(),
        "the reference printed nothing for {}",
        case.dsl.display()
    );
    for front_end in debugging_front_ends() {
        let output = debug(front_end, case, commands);
        assert_eq!(
            output,
            spec,
            "{front_end} differs from {DEFAULT_FRONT_END} on {}\ncommands: {commands:?}",
            case.dsl.display()
        );
    }
    spec
}

fn words(commands: &[&str]) -> Vec<String> {
    commands
        .iter()
        .map(|command| (*command).to_string())
        .collect()
}

#[test]
fn the_fixed_scenarios_match_on_every_front_end() {
    let door = &cases()[0];
    assert_every_front_end_matches_the_spec(
        door,
        &words(&[
            "show",
            "set request on",
            "step",
            "finish",
            "step",
            "set timeout on",
            "step",
            "history",
            "show",
        ]),
    );
    let walk = &cases()[1];
    assert_every_front_end_matches_the_spec(
        walk,
        &words(&[
            "finish",
            "step",
            "set keyup on",
            "step",
            "choose facing",
            "show",
            "history",
        ]),
    );
    let order = &cases()[2];
    assert_every_front_end_matches_the_spec(
        order,
        &words(&[
            "set payment on",
            "set stock on",
            "set carrier on",
            "set returned on",
            "step",
            "step",
            "show",
            "history",
        ]),
    );
    let cycle = &cases()[3];
    let output = assert_every_front_end_matches_the_spec(
        cycle,
        &words(&["set x on", "step", "show", "history"]),
    );
    assert!(output.contains("(cycle)"), "{output}");
}

/// A tiny deterministic generator, so the random sequences are the same on every run.
/// A random command sequence over `case`'s vocabulary. It includes resets and a few mistakes, so
/// the error text is compared too.
fn random_commands(case: &Case, seed: u64, length: usize) -> Vec<String> {
    let mut random = Lcg::new(seed);
    let mut commands = Vec::with_capacity(length);
    for _ in 0..length {
        let command = match random.below(12) {
            0 | 1 => format!(
                "set {} {}",
                pick(&mut random, case.conditions),
                on_or_off(&mut random)
            ),
            2 => "set missing on".to_string(),
            3 | 4 => "finish".to_string(),
            5..=7 => "step".to_string(),
            8 if !case.choices.is_empty() => format!("choose {}", pick(&mut random, case.choices)),
            8 => "show".to_string(),
            9 => "history".to_string(),
            10 => "show".to_string(),
            _ => "reset".to_string(),
        };
        commands.push(command);
    }
    commands
}

fn pick<'a>(random: &mut Lcg, names: &[&'a str]) -> &'a str {
    names[random.below(names.len())]
}

fn on_or_off(random: &mut Lcg) -> &'static str {
    if random.below(2) == 0 { "on" } else { "off" }
}

#[test]
fn random_command_sequences_match_on_every_front_end() {
    for (index, case) in cases().iter().enumerate() {
        for seed in 0..3 {
            let commands = random_commands(case, (index as u64) * 1000 + seed, 120);
            assert_every_front_end_matches_the_spec(case, &commands);
        }
    }
}

#[test]
fn every_debugging_front_end_is_checked_here() {
    assert_eq!(debugging_front_ends()[0], DEFAULT_FRONT_END);
    assert!(
        debugging_front_ends().len() >= 2,
        "a second front end must have a debugger"
    );
}
