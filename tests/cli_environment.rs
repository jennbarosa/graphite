//! What `doctor` and `run` report when a tool is missing or a file is wrong. Each case sets the
//! environment the way a user would, so the message is the one they see.

#[allow(dead_code)]
#[path = "support/cli.rs"]
mod cli;

use cli::{dsl, graphite, scratch, stderr, stdout, walk_spec};
use std::process::Command;

fn graphite_with_env(args: &[&str], variables: &[(&str, &str)]) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_graphite"));
    command.args(args);
    for (name, value) in variables {
        command.env(name, value);
    }
    command.output().expect("graphite binary must run")
}

#[test]
fn doctor_blocks_each_runtime_it_cannot_find() {
    let output = graphite_with_env(&["doctor"], &[("PATH", "/nonexistent-graphite-path")]);
    // A blocked command makes `doctor` fail, and says so on stderr.
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("some commands cannot run here"),
        "{}",
        stderr(&output)
    );
    let text = stdout(&output);
    assert!(
        text.contains("debug    blocked  needs a JavaScript runtime"),
        "{text}"
    );
    assert!(
        text.contains("debug    blocked  needs rustc to build the debugger"),
        "{text}"
    );
    assert!(text.contains("summary: blocked: "), "{text}");
}

#[test]
fn doctor_names_a_tsc_that_is_not_installed() {
    let output = graphite_with_env(&["doctor"], &[("GRAPHITE_TSC", "/nonexistent/tsc")]);
    assert!(stdout(&output).contains("missing  tsc for the TypeScript type-check test"));
}

#[test]
fn doctor_json_lists_every_front_end_with_its_status() {
    let output = graphite(["doctor", "--json"]);
    assert!(output.status.success(), "{}", stderr(&output));
    let text = stdout(&output);
    assert!(text.starts_with("{\"ok\":true"), "{text}");
    for key in ["\"typescript\"", "\"rust\"", "\"purescript\""] {
        assert!(text.contains(key), "{key} missing from {text}");
    }
}

#[test]
fn a_runner_that_cannot_start_is_named() {
    let output = graphite_with_env(
        &["run", walk_spec().to_str().unwrap()],
        &[("GRAPHITE_JS_RUNNER", "/nonexistent/graphite-runner")],
    );
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("cannot start GRAPHITE_JS_RUNNER `/nonexistent/graphite-runner`"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn a_tasks_file_without_a_tasks_table_is_refused() {
    let tasks = dsl("tasks_without_table.toml", "[other]\nx = 1\n");
    let output = graphite([
        "run".as_ref(),
        walk_spec().as_os_str(),
        "--tasks".as_ref(),
        tasks.as_os_str(),
    ]);
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("expected a [tasks] table of commands"),
        "{}",
        stderr(&output)
    );
    assert!(
        stderr(&output).contains("runs the commands in"),
        "the warning is printed first"
    );
}

#[test]
fn a_run_with_a_missing_allowlist_names_the_file() {
    let missing = scratch("no_such_allowlist.toml");
    let output = graphite([
        "run".as_ref(),
        walk_spec().as_os_str(),
        "--allow".as_ref(),
        missing.as_os_str(),
    ]);
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("no_such_allowlist.toml"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn doctor_json_marks_blocked_unavailable_and_missing_entries() {
    let output = graphite_with_env(
        &["doctor", "--json"],
        &[
            ("PATH", "/nonexistent-graphite-path"),
            ("GRAPHITE_TSC", "/nonexistent/tsc"),
        ],
    );
    assert!(!output.status.success());
    let text = stdout(&output);
    let text = text.trim_end();
    assert!(text.starts_with("{\"ok\":false,"), "{text}");
    assert!(text.contains("\"status\":\"blocked\""), "{text}");
    assert!(text.contains("\"status\":\"unavailable\""), "{text}");
    assert!(
        text.contains("\"test_tools\":[{\"status\":\"missing\""),
        "{text}"
    );
    assert!(
        text.ends_with("\"blocked\":[\"typescript debug\",\"typescript run\",\"rust debug\"]}"),
        "{text}"
    );
}
