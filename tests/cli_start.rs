//! First-run commands: help, `init`, default output directory, and `doctor`.

#[allow(dead_code)]
#[path = "support/cli.rs"]
mod cli;

use std::process::Command;

use cli::{dsl, graphite, scratch, stderr, stdout};

#[test]
fn help_goes_to_stdout_and_succeeds_in_every_form() {
    let overview = graphite(["--help"]);
    assert!(overview.status.success());
    assert!(stdout(&overview).contains("graphite init <file.dsl>"));

    let per_command = graphite(["compile", "x.dsl", "--help"]);
    assert!(per_command.status.success());
    assert!(stdout(&per_command).contains("--out defaults to graphite-out/"));

    let named = graphite(["help", "run"]);
    assert!(named.status.success());
    assert!(stdout(&named).contains("--durable is accepted"));
}

#[test]
fn an_unknown_help_topic_fails_with_the_overview() {
    let output = graphite(["help", "bogus"]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("no help for \"bogus\""));
}

#[test]
fn init_writes_a_starter_graph_that_checks_and_refuses_to_overwrite() {
    let path = scratch("start-init").join("hello.dsl");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let _ = std::fs::remove_file(&path);

    let created = graphite(["init".as_ref(), path.as_os_str()]);
    assert!(created.status.success(), "{}", stderr(&created));
    let checked = graphite(["check".as_ref(), path.as_os_str()]);
    assert!(checked.status.success(), "{}", stderr(&checked));
    assert_eq!(stdout(&checked).trim(), "ok: 1 graph");

    let again = graphite(["init".as_ref(), path.as_os_str()]);
    assert!(!again.status.success());
    assert!(stderr(&again).contains("already exists"));
}

#[test]
fn compile_without_out_writes_beside_the_working_directory() {
    let source = dsl("start-default-out/door.dsl", "graph door\n  node a end\n");
    let work = source.parent().unwrap().to_path_buf();
    let _ = std::fs::remove_dir_all(work.join("graphite-out"));
    let output = Command::new(env!("CARGO_BIN_EXE_graphite"))
        .current_dir(&work)
        .args(["compile".as_ref(), source.file_name().unwrap()])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(work.join("graphite-out/door/door.ts").exists());
    assert!(work.join("graphite-out/door/graphite.ts").exists());
}

#[test]
fn doctor_reports_the_runtime_and_fails_only_without_one() {
    let output = graphite(["doctor"]);
    let text = stdout(&output);
    assert!(text.contains("JavaScript runtime:"), "{text}");
    assert!(text.contains("TypeScript:"), "{text}");
    // The result depends on this machine's runtimes; the wording is what this checks.
    assert_eq!(
        output.status.success(),
        text.contains("ok       JavaScript runtime")
    );
}
