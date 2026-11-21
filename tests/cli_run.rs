//! `graphite run`: runs a graph under the durable engine, and resumes from saved state.

#[allow(dead_code)]
#[path = "support/bun.rs"]
mod bun;
#[allow(dead_code)]
#[path = "support/cli.rs"]
mod cli;
#[allow(dead_code)]
#[path = "support/tools.rs"]
mod tools;

use cli::{graphite, manifest, scratch, stderr, stdout, walk_spec};

#[test]
fn runs_the_graph_to_its_end_and_resumes_from_saved_state() {
    let fixtures = manifest().join("tests/fixtures/runtime");
    let dsl = fixtures.join("workflow.graphite");
    let allow = fixtures.join("workflow.toml");
    let runner = fixtures.join("pass_runner.ts");
    let state = scratch("cli-run").join("state.json");
    let args = [
        "run".as_ref(),
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
    let dsl = fixtures.join("workflow.graphite");
    let allow = fixtures.join("workflow.toml");
    let dir = scratch("cli-run-no-runner");
    let state = dir.join("state.json");
    let output = graphite([
        "run".as_ref(),
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

/// `--durable` did nothing, so it was removed. An old script that passes it fails loudly.
#[test]
fn the_removed_durable_flag_is_a_usage_error() {
    let output = graphite([
        "run".as_ref(),
        walk_spec().as_os_str(),
        "--durable".as_ref(),
    ]);
    assert_eq!(output.status.code(), Some(2), "{}", stdout(&output));
    assert!(
        stderr(&output).contains("unknown option --durable"),
        "{}",
        stderr(&output)
    );
}

/// `run` is durable on its own.
#[test]
fn runs_durably_without_any_flag() {
    let fixtures = manifest().join("tests/fixtures/runtime");
    let state = scratch("cli-run-without-flag").join("state.json");
    let output = graphite([
        "run".as_ref(),
        fixtures.join("workflow.graphite").as_os_str(),
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
    let dsl = dir.join("workflow.graphite");
    std::fs::copy(fixtures.join("workflow.graphite"), &dsl).unwrap();
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

#[test]
fn exits_when_a_runner_leaves_a_timer_open() {
    let fixtures = manifest().join("tests/fixtures/runtime");
    let state = scratch("cli-run-held").join("state.json");
    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_graphite"))
        .args(["run"])
        .arg(fixtures.join("workflow.graphite"))
        .arg("--allow")
        .arg(fixtures.join("workflow.toml"))
        .arg("--state")
        .arg(&state)
        .arg("--runner")
        .arg(fixtures.join("held_runner.ts"))
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    // A watchdog: a CLI that does not exit would otherwise hang the whole test run.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if std::time::Instant::now() > deadline {
            let _ = child.kill();
            panic!("graphite run did not exit after the graph ended");
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    };
    let output = child.wait_with_output().unwrap();
    assert!(status.success(), "{}", stderr(&output));
    assert!(stdout(&output).contains(r#""status":"ended""#));
}
