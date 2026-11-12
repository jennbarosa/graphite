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
fn doctor_reports_the_version_and_each_front_end() {
    let output = graphite(["doctor"]);
    let text = stdout(&output);
    assert!(
        text.starts_with(&format!("graphite {}", env!("CARGO_PKG_VERSION"))),
        "{text}"
    );
    assert!(text.contains("front ends:"), "{text}");
    assert!(text.contains("typescript (default)"), "{text}");
    assert!(text.contains("compile  ok"), "{text}");
    // Whether `debug` and `run` work depends on this machine's runtimes. The exit status must
    // agree with the summary: it fails exactly when a command is blocked.
    assert_eq!(output.status.success(), !text.contains("blocked"), "{text}");
}

/// On a machine with no JavaScript runtime, `doctor` names the blocked commands, fails, and says why.
#[test]
fn doctor_reports_blocked_commands_when_no_runtime_is_installed() {
    let empty = scratch("start-doctor-empty-path");
    std::fs::create_dir_all(&empty).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_graphite"))
        .arg("doctor")
        .env_clear()
        .env("PATH", &empty)
        .output()
        .unwrap();
    let text = stdout(&output);
    assert!(!output.status.success());
    assert!(text.contains("debug    blocked"), "{text}");
    assert!(
        text.contains("summary: blocked: typescript debug, typescript run"),
        "{text}"
    );
    assert!(stderr(&output).contains("some commands cannot run here"));
}
