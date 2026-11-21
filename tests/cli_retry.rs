//! `graphite run --retry <key>`: a task that threw stops the run, the next run refuses to run it
//! again, and `--retry` clears its stored error so the run runs it once more.

#[allow(dead_code)]
#[path = "support/cli.rs"]
mod cli;

use cli::{graphite, scratch, stderr, stdout};

/// A workdir with a one-node-task graph whose `audit` task fails until `ok` exists.
fn workdir(name: &str) -> std::path::PathBuf {
    let dir = scratch(name);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("item.graphite"),
        "graph item\n  node audit @task:audit\n  node save end @task:save\n  node skip end\n\n  edge audit -> save when passed\n  edge audit -> skip when failed\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("audit.sh"),
        "cat > /dev/null\nif [ -e ok ]; then printf '{\"outcome\":\"pass\"}\\n'; exit 0; fi\nexit 1\n",
    )
    .unwrap();
    std::fs::write(dir.join("pass.sh"), "cat > /dev/null\n").unwrap();
    std::fs::write(
        dir.join("tasks.toml"),
        "[tasks]\naudit = [\"sh\", \"audit.sh\"]\nsave = [\"sh\", \"pass.sh\"]\n",
    )
    .unwrap();
    dir
}

/// `graphite run item.graphite --tasks tasks.toml --state state.json` in `dir`, with `extra` flags.
fn run(dir: &std::path::Path, extra: &[&str]) -> std::process::Output {
    let mut args: Vec<std::ffi::OsString> = vec![
        "run".into(),
        dir.join("item.graphite").into_os_string(),
        "--tasks".into(),
        dir.join("tasks.toml").into_os_string(),
        "--state".into(),
        dir.join("state.json").into_os_string(),
    ];
    args.extend(extra.iter().map(|arg| (*arg).into()));
    graphite(args)
}

#[test]
fn a_stored_task_error_is_cleared_by_retry_and_the_run_then_ends() {
    let dir = workdir("cli-retry");

    let first = run(&dir, &[]);
    assert!(!first.status.success(), "{}", stderr(&first));
    assert!(
        stdout(&first).contains(r#""key":"0:audit""#),
        "{}",
        stdout(&first)
    );

    // Without --retry, the stored error stops the run again and the task does not run.
    std::fs::write(dir.join("ok"), "").unwrap();
    let again = run(&dir, &[]);
    assert!(!again.status.success());
    assert!(
        stdout(&again).contains("failed in an earlier run"),
        "{}",
        stdout(&again)
    );

    // A key with no stored error is refused, and nothing changes.
    let wrong = run(&dir, &["--retry", "0:save"]);
    assert!(!wrong.status.success());
    assert!(
        stderr(&wrong).contains("has no stored error"),
        "{}",
        stderr(&wrong)
    );

    let retried = run(&dir, &["--retry", "0:audit"]);
    assert_eq!(retried.status.code(), Some(0), "{}", stderr(&retried));
    assert!(
        stdout(&retried).contains(r#""status":"ended""#),
        "{}",
        stdout(&retried)
    );
}

#[test]
fn a_refused_retry_key_leaves_the_stored_error_in_place() {
    let dir = workdir("cli-retry-refused");
    let first = run(&dir, &[]);
    assert!(!first.status.success(), "{}", stderr(&first));

    // A key numbered after the failed stage is refused, and the failed task's error must survive it.
    std::fs::write(dir.join("ok"), "").unwrap();
    let wrong = run(&dir, &["--retry", "5:save"]);
    assert!(!wrong.status.success());
    assert!(
        stderr(&wrong).contains("has no stored error"),
        "{}",
        stderr(&wrong)
    );

    let again = run(&dir, &[]);
    assert!(
        stdout(&again).contains("failed in an earlier run"),
        "{}",
        stdout(&again)
    );
}

#[test]
fn retry_does_not_apply_to_each() {
    let dir = workdir("cli-retry-each");
    let output = graphite([
        "run".as_ref(),
        dir.join("item.graphite").as_os_str(),
        "--each".as_ref(),
        dir.join("item.graphite").as_os_str(),
        "--key".as_ref(),
        "domain".as_ref(),
        "--retry".as_ref(),
        "0:audit".as_ref(),
    ]);
    assert_eq!(output.status.code(), Some(2));
    assert!(
        stderr(&output).contains("does not apply to --each"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn retry_refuses_a_stage_the_run_has_already_passed() {
    // The errored edge routes the failure, so the run moves on past the stage that failed.
    let dir = scratch("cli-retry-past");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("item.graphite"),
        "graph item\n  node audit @task:audit\n  node skip end\n\n  edge audit -> skip when passed\n  edge audit -> skip when errored\n  edge audit -> skip when failed\n",
    )
    .unwrap();
    std::fs::write(dir.join("audit.sh"), "cat > /dev/null\nexit 1\n").unwrap();
    std::fs::write(
        dir.join("tasks.toml"),
        "[tasks]\naudit = [\"sh\", \"audit.sh\"]\n",
    )
    .unwrap();
    let allow = dir.join("allow.toml");
    std::fs::write(&allow, "node_flags = []\n").unwrap();
    let routed = |extra: &[&str]| {
        let mut args: Vec<std::ffi::OsString> = vec![
            "run".into(),
            dir.join("item.graphite").into_os_string(),
            "--tasks".into(),
            dir.join("tasks.toml").into_os_string(),
            "--state".into(),
            dir.join("state.json").into_os_string(),
        ];
        args.extend(extra.iter().map(|arg| (*arg).into()));
        graphite(args)
    };

    let first = routed(&[]);
    assert_eq!(first.status.code(), Some(0), "{}", stderr(&first));
    assert!(
        stdout(&first).contains(r#""status":"ended""#),
        "{}",
        stdout(&first)
    );

    // The stage is done and the run ended past it, so its error cannot be retried.
    let late = routed(&["--retry", "0:audit"]);
    assert!(!late.status.success(), "{}", stdout(&late));
    assert!(
        stderr(&late).contains("the run is past this stage"),
        "{}",
        stderr(&late)
    );
}
