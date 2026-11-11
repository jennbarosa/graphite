//! A chained `edge a -> b -> c when COND else T` must run exactly like the expanded edges it
//! stands for. Both forms of the gads graph run through the debugger under the same commands.

#[allow(dead_code)]
#[path = "support/cli.rs"]
mod cli;

use cli::{graphite, manifest, stderr, stdout};

fn fixture(name: &str) -> std::path::PathBuf {
    manifest().join("tests/fixtures/runtime").join(name)
}

/// Debugger output with edge ids replaced by their count: the chained form pushes its edges in
/// a different order, so ids differ while the transitions taken do not.
fn normalize(output: &str) -> String {
    output
        .lines()
        .map(|line| {
            let (prefix, ids) = match line.strip_prefix("edges ") {
                Some(ids) => ("edges ", ids),
                None => ("", line),
            };
            let numbers: Vec<&str> = ids.split_whitespace().collect();
            if !numbers.is_empty() && numbers.iter().all(|n| n.parse::<u32>().is_ok()) {
                format!("{prefix}{} ids", numbers.len())
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn run(dsl: &str, commands: &[&str]) -> String {
    let mut args = vec![
        "debug".to_string(),
        fixture(dsl).display().to_string(),
        "--allow".to_string(),
        fixture("gads_allow.toml").display().to_string(),
    ];
    args.extend(commands.iter().map(|command| command.to_string()));
    let output = graphite(&args);
    assert!(output.status.success(), "{dsl}: {}", stderr(&output));
    stdout(&output)
}

fn assert_same_behavior(commands: &[&str]) -> String {
    let expanded = normalize(&run("gads_expanded.dsl", commands));
    let chained = normalize(&run("gads_chain.dsl", commands));
    assert_eq!(chained, expanded, "commands: {commands:?}");
    expanded
}

#[test]
fn the_happy_path_reaches_report_through_every_stage() {
    let mut commands = vec!["set pass on"];
    commands.extend(std::iter::repeat_n(["finish", "step"], 12).flatten());
    commands.extend(["show", "history"]);
    let output = assert_same_behavior(&commands);
    assert!(output.contains("node report"), "{output}");
}

#[test]
fn every_failure_falls_back_to_report_from_the_first_stage() {
    let output = assert_same_behavior(&["finish", "step", "show", "history"]);
    assert!(output.contains("node report"), "{output}");
}

#[test]
fn a_gate_moves_forward_without_finish_the_same_way() {
    assert_same_behavior(&["set pass on", "step", "show"]);
}

#[test]
fn failing_midway_takes_the_fallback_from_the_current_stage() {
    let mut commands = vec![
        "set pass on",
        "finish",
        "step",
        "finish",
        "step",
        "finish",
        "step",
    ];
    commands.extend(["set pass off", "finish", "step", "show", "history"]);
    let output = assert_same_behavior(&commands);
    assert!(output.contains("node report"), "{output}");
}

#[test]
fn toggling_the_condition_between_stages_matches() {
    assert_same_behavior(&[
        "set pass on",
        "finish",
        "step",
        "set pass off",
        "finish",
        "step",
        "show",
        "history",
        "set pass on",
        "finish",
        "step",
        "finish",
        "step",
        "show",
        "history",
    ]);
}
