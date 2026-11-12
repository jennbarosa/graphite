//! `graphite run --durable`: runs a graph under the durable engine, and resumes from saved state.

#[allow(dead_code)]
#[path = "support/bun.rs"]
mod bun;
#[allow(dead_code)]
#[path = "support/cli.rs"]
mod cli;

use cli::{graphite, manifest, scratch, stderr, stdout};

#[test]
fn runs_the_graph_to_its_end_and_resumes_from_saved_state() {
    let fixtures = manifest().join("tests/fixtures/runtime");
    let dsl = fixtures.join("workflow.dsl");
    let allow = fixtures.join("workflow.toml");
    let runner = fixtures.join("pass_runner.ts");
    let state = scratch("cli-run").join("state.json");
    let args = [
        "run".as_ref(),
        "--durable".as_ref(),
        dsl.as_os_str(),
        "--allow".as_ref(),
        allow.as_os_str(),
        "--state".as_ref(),
        state.as_os_str(),
        "--runner".as_ref(),
        runner.as_os_str(),
    ];

    let first = graphite(args);
    assert!(first.status.success(), "{}", stderr(&first));
    assert!(
        stderr(&first).contains("warning: --runner runs"),
        "{}",
        stderr(&first)
    );
    assert_eq!(
        stdout(&first).trim(),
        r#"{"status":"ended","transitions":2,"node":"ship","outcome":"pass"}"#
    );
    assert!(stderr(&first).contains("[run] started fresh"));

    let second = graphite(args);
    assert!(second.status.success(), "{}", stderr(&second));
    assert!(stderr(&second).contains("[run] started resumed"));
    assert!(stderr(&second).contains("(stored, key 2:ship)"));
}

#[test]
fn refuses_a_graph_with_tasks_when_no_runner_is_given() {
    let fixtures = manifest().join("tests/fixtures/runtime");
    let dsl = fixtures.join("workflow.dsl");
    let allow = fixtures.join("workflow.toml");
    let dir = scratch("cli-run-no-runner");
    let state = dir.join("state.json");
    let output = graphite([
        "run".as_ref(),
        "--durable".as_ref(),
        dsl.as_os_str(),
        "--allow".as_ref(),
        allow.as_os_str(),
        "--state".as_ref(),
        state.as_os_str(),
    ]);
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("but no runner"),
        "{}",
        stderr(&output)
    );
    // Refused before the run starts, so no state or lock is written.
    assert!(!state.exists());
    assert!(!dir.join("state.json.lock").exists());
}

#[test]
fn runs_a_graph_without_tasks_or_outcome_conditions() {
    let source = "graph plain\n  node a\n  node b end\n  edge a -> b\n";
    let dsl = cli::dsl("plain", source);
    let state = scratch("cli-run-plain").join("state.json");
    let output = graphite([
        "run".as_ref(),
        "--durable".as_ref(),
        dsl.as_os_str(),
        "--state".as_ref(),
        state.as_os_str(),
    ]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(
        stdout(&output).trim(),
        r#"{"status":"ended","transitions":1,"node":"b","outcome":"none"}"#
    );
}

/// `run` is durable on its own; the flag is accepted and changes nothing.
#[test]
fn runs_durably_without_the_durable_flag() {
    let fixtures = manifest().join("tests/fixtures/runtime");
    let state = scratch("cli-run-without-flag").join("state.json");
    let output = graphite([
        "run".as_ref(),
        fixtures.join("workflow.dsl").as_os_str(),
        "--allow".as_ref(),
        fixtures.join("workflow.toml").as_os_str(),
        "--state".as_ref(),
        state.as_os_str(),
        "--runner".as_ref(),
        fixtures.join("pass_runner.ts").as_os_str(),
    ]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(
        stdout(&output).trim(),
        r#"{"status":"ended","transitions":2,"node":"ship","outcome":"pass"}"#
    );
}

/// Without `--state`, the checkpoint goes beside the graph file, named `<stem>.state.json`.
#[test]
fn the_state_file_defaults_beside_the_graph() {
    let fixtures = manifest().join("tests/fixtures/runtime");
    let dir = scratch("cli-run-default-state");
    std::fs::create_dir_all(&dir).unwrap();
    let dsl = dir.join("workflow.dsl");
    std::fs::copy(fixtures.join("workflow.dsl"), &dsl).unwrap();
    let output = graphite([
        "run".as_ref(),
        dsl.as_os_str(),
        "--allow".as_ref(),
        fixtures.join("workflow.toml").as_os_str(),
        "--runner".as_ref(),
        fixtures.join("pass_runner.ts").as_os_str(),
    ]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(dir.join("workflow.state.json").exists());
}
