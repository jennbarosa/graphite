//! `graphite run --tasks` and `--input`: command-backed tasks, the input every task receives, and
//! the refusals and errors around them.

#[allow(dead_code)]
#[path = "support/cli.rs"]
mod cli;

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use cli::{graphite, manifest, scratch, stderr, stdout};

/// A script that appends its stdin to `log.txt`, then prints its first argument: the report.
const LOG_SCRIPT: &str = "cat >> log.txt\nprintf '%s\\n' \"$1\"\n";

/// A fresh working directory with the review workflow, its allowlist, `tasks.toml`, and any
/// extra files as `(name, contents)`.
fn workdir(name: &str, tasks: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir = scratch(name);
    std::fs::create_dir_all(&dir).unwrap();
    let fixtures = manifest().join("tests/fixtures/runtime");
    std::fs::copy(fixtures.join("workflow.dsl"), dir.join("workflow.dsl")).unwrap();
    std::fs::copy(fixtures.join("workflow.toml"), dir.join("workflow.toml")).unwrap();
    std::fs::write(dir.join("tasks.toml"), tasks).unwrap();
    std::fs::write(dir.join("log.sh"), LOG_SCRIPT).unwrap();
    for (file, contents) in files {
        std::fs::write(dir.join(file), contents).unwrap();
    }
    dir
}

/// A task that runs `log.sh` and answers with `report`, a JSON line.
fn logged(report: &str) -> String {
    format!("[\"sh\", \"log.sh\", '{report}']")
}

/// `graphite run` on the workdir's workflow, with the allowlist and state file, plus `extra`.
fn run(dir: &Path, extra: &[OsString]) -> std::process::Output {
    let mut args: Vec<OsString> = vec![
        "run".into(),
        dir.join("workflow.dsl").into_os_string(),
        "--allow".into(),
        dir.join("workflow.toml").into_os_string(),
        "--state".into(),
        dir.join("state.json").into_os_string(),
    ];
    args.extend_from_slice(extra);
    graphite(args)
}

fn tasks_flag(dir: &Path) -> [OsString; 2] {
    ["--tasks".into(), dir.join("tasks.toml").into_os_string()]
}

#[test]
fn every_task_runs_its_command_and_the_input_reaches_it() {
    let pass = r#"{"outcome":"pass"}"#;
    let tasks = format!(
        "[tasks]\nwrite = {}\nlint = {}\nfix = {}\ndeploy = {}\n",
        logged(pass),
        logged(pass),
        logged(pass),
        logged(pass)
    );
    let dir = workdir(
        "cli-tasks-input",
        &tasks,
        &[("input.json", r#"{"target":"prospect-42"}"#)],
    );

    let mut extra = tasks_flag(&dir).to_vec();
    extra.extend(["--input".into(), dir.join("input.json").into_os_string()]);
    let output = run(&dir, &extra);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(
        stdout(&output).trim(),
        r#"{"status":"ended","transitions":2}"#
    );

    let log = std::fs::read_to_string(dir.join("log.txt")).unwrap();
    assert!(log.contains(r#""task":"lint""#), "{log}");
    assert_eq!(
        log.matches(r#""input":{"target":"prospect-42"}"#).count(),
        3,
        "{log}"
    );
}

#[test]
fn a_failing_command_routes_to_fix_and_its_detail_reaches_fix() {
    // The first lint fails with a detail; every later lint passes.
    let lint = "if [ -f marker ]; then printf '%s\\n' '{\"outcome\":\"pass\"}'; \
                else touch marker; \
                printf '%s\\n' '{\"outcome\":\"fail\",\"detail\":{\"message\":\"2 checks failed\",\"code\":\"lint\"}}'; fi\n";
    let pass = r#"{"outcome":"pass"}"#;
    let tasks = format!(
        "[tasks]\nwrite = {}\nlint = [\"sh\", \"lint.sh\"]\nfix = {}\ndeploy = {}\n",
        logged(r#"{"outcome":"none"}"#),
        logged(pass),
        logged(pass)
    );
    let dir = workdir(
        "cli-tasks-fail",
        &tasks,
        &[("lint.sh", &format!("cat >> log.txt\n{lint}"))],
    );

    let output = run(&dir, &tasks_flag(&dir));
    assert!(
        output.status.success(),
        "{}{}",
        stdout(&output),
        stderr(&output)
    );

    let log = std::fs::read_to_string(dir.join("log.txt")).unwrap();
    assert!(log.contains(r#""message":"2 checks failed""#), "{log}");
    assert!(log.contains(r#""code":"lint""#), "{log}");
}

#[test]
fn runner_and_tasks_together_are_refused_as_a_usage_error() {
    let dir = workdir("cli-tasks-both", "[tasks]\n", &[]);
    let mut extra = tasks_flag(&dir).to_vec();
    extra.extend(["--runner".into(), "x.ts".into()]);
    let output = run(&dir, &extra);
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("pass one"), "{}", stderr(&output));
}

#[test]
fn a_task_without_a_command_is_named_before_anything_is_recorded() {
    let dir = workdir(
        "cli-tasks-missing",
        &format!("[tasks]\nwrite = {}\n", logged("{}")),
        &[],
    );
    let output = run(&dir, &tasks_flag(&dir));
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("no command in"),
        "{}",
        stderr(&output)
    );
    assert!(stderr(&output).contains("lint"), "{}", stderr(&output));
    assert!(!dir.join("state.json").exists());
}

#[test]
fn a_manifest_entry_that_is_not_a_command_list_is_named() {
    let dir = workdir("cli-tasks-bad", "[tasks]\nlint = []\n", &[]);
    let output = run(&dir, &tasks_flag(&dir));
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("task lint must be a non-empty list of strings"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn input_is_refused_for_commands_that_do_not_run_tasks() {
    let dir = workdir("cli-tasks-refused", "[tasks]\n", &[]);
    let output = graphite([
        "check".as_ref(),
        dir.join("workflow.dsl").as_os_str(),
        "--input".as_ref(),
        dir.join("workflow.toml").as_os_str(),
    ]);
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("check does not take --input"));
}
