//! `graphite debug`: runs the TypeScript debugger over a compiled graph.

#[allow(dead_code)]
#[path = "support/bun.rs"]
mod bun;
#[allow(dead_code)]
#[path = "support/cli.rs"]
mod cli;

use cli::{graphite, manifest, scratch, stderr, stdout, walk_spec};

#[test]
fn runs_the_commands_against_the_first_graph() {
    let spec = walk_spec();
    let output = graphite([
        "debug".as_ref(),
        spec.as_os_str(),
        "finish".as_ref(),
        "step".as_ref(),
    ]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(stdout(&output).starts_with("node windup  status running  transitions 0  true: none"));
    assert!(stdout(&output).contains("node going  status running  transitions 1"));
}

#[test]
fn selects_a_graph_by_name_and_names_the_choices_when_it_is_missing() {
    let spec = walk_spec();
    let output = graphite([
        "debug".as_ref(),
        spec.as_os_str(),
        "--graph".as_ref(),
        "walk".as_ref(),
        "choose facing".as_ref(),
    ]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(stdout(&output).contains("facing: else -> down"));

    let missing = graphite([
        "debug".as_ref(),
        spec.as_os_str(),
        "--graph".as_ref(),
        "nosuch".as_ref(),
    ]);
    assert!(!missing.status.success());
    assert!(stderr(&missing).contains("no graph named \"nosuch\"; the file has walk"));
}

#[test]
fn refuses_an_unknown_front_end() {
    let spec = walk_spec();
    let output = graphite([
        "debug".as_ref(),
        spec.as_os_str(),
        "--frontend".as_ref(),
        "cobol".as_ref(),
    ]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("unknown front end \"cobol\""));
}

#[test]
fn with_out_keeps_the_generated_modules() {
    let out = scratch("cli-debug-out");
    let spec = walk_spec();
    let output = graphite([
        "debug".as_ref(),
        spec.as_os_str(),
        "--out".as_ref(),
        out.as_os_str(),
        "quit".as_ref(),
    ]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(out.join("walk.ts").is_file());
}

#[test]
fn the_debugger_drives_the_compiled_walk_cycle() {
    let out = scratch("cli-debug-walk");
    let spec = walk_spec();
    let output = graphite([
        "compile".as_ref(),
        spec.as_os_str(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);
    assert!(output.status.success(), "{}", stderr(&output));
    bun::assert_bun_test_passes(
        manifest(),
        "tests/typescript/debug_cli.test.ts",
        "GRAPHITE_DEBUG_WALK",
        &out.join("walk.ts"),
    );
}

#[test]
fn a_configured_runner_runs_the_debugger_in_its_place() {
    let spec = walk_spec();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_graphite"))
        .arg("debug")
        .arg(&spec)
        .arg("quit")
        .env("GRAPHITE_JS_RUNNER", "echo runner")
        .output()
        .expect("graphite binary must run");
    assert!(output.status.success(), "{}", stderr(&output));
    let text = stdout(&output);
    assert!(text.starts_with("runner "), "{text}");
    assert!(text.trim_end().ends_with(" quit"), "{text}");
}

#[test]
fn a_configured_runner_that_is_missing_is_named_in_the_error() {
    let spec = walk_spec();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_graphite"))
        .arg("debug")
        .arg(&spec)
        .arg("quit")
        .env("GRAPHITE_JS_RUNNER", "graphite-no-such-runner")
        .output()
        .expect("graphite binary must run");
    assert!(!output.status.success());
    assert!(stderr(&output).contains("cannot start GRAPHITE_JS_RUNNER `graphite-no-such-runner`"));
}
