//! `graphite run --each`: one item graph run per item, the seen file, and the passing counter.

#[allow(dead_code)]
#[path = "support/cli.rs"]
mod cli;

use std::path::{Path, PathBuf};

use cli::{graphite, scratch, stderr, stdout};

/// A list graph that ends with an output, and an item graph that runs per item. Both use the same
/// tasks: `emit` makes the list, from `items.json`, and the others pass.
fn workdir(name: &str, items: &str) -> PathBuf {
    let dir = scratch(name);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("list.graphite"),
        "graph list\n  node find @task:discover\n  node done end @task:emit\n\n  edge find -> done when passed\n  edge find -> done when failed\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("item.graphite"),
        "graph item\n  node audit @task:audit\n  node save end @task:save\n  node skip end\n\n  edge audit -> save when passed\n  edge audit -> skip when failed\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("allow.toml"),
        "node_flags = []\n\n[annotations]\ntask = [\"discover\", \"emit\", \"audit\", \"save\"]\n",
    )
    .unwrap();
    std::fs::write(dir.join("items.json"), items).unwrap();
    std::fs::write(
        dir.join("pass.sh"),
        "cat > /dev/null\nprintf '{\"outcome\":\"pass\"}\\n'\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("emit.sh"),
        "cat > /dev/null\nprintf '{\"outcome\":\"pass\",\"output\":'\ncat items.json\nprintf '}\\n'\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("tasks.toml"),
        "[tasks]\ndiscover = [\"sh\", \"pass.sh\"]\nemit = [\"sh\", \"emit.sh\"]\naudit = [\"sh\", \"pass.sh\"]\nsave = [\"sh\", \"pass.sh\"]\n",
    )
    .unwrap();
    dir
}

/// `graphite run list.graphite --each item.graphite --key domain` in `dir`, with `extra` flags.
fn fan_out(dir: &Path, extra: &[&str]) -> std::process::Output {
    graphite(fan_out_args(dir, extra))
}

fn fan_out_args(dir: &Path, extra: &[&str]) -> Vec<std::ffi::OsString> {
    let mut args: Vec<std::ffi::OsString> = vec![
        "run".into(),
        dir.join("list.graphite").into_os_string(),
        "--each".into(),
        dir.join("item.graphite").into_os_string(),
        "--key".into(),
        "domain".into(),
        "--allow".into(),
        dir.join("allow.toml").into_os_string(),
        "--tasks".into(),
        dir.join("tasks.toml").into_os_string(),
        "--state".into(),
        dir.join("state.json").into_os_string(),
    ];
    args.extend(extra.iter().map(|arg| (*arg).into()));
    args
}

#[test]
fn each_item_runs_once_and_a_second_run_starts_none() {
    let dir = workdir(
        "cli-fanout-dedupe",
        r#"[{"domain":"a.com"},{"domain":"b.com"},{"domain":"a.com"}]"#,
    );
    let first = fan_out(&dir, &[]);
    assert!(first.status.success(), "{}", stderr(&first));
    assert_eq!(
        stdout(&first).trim(),
        r#"{"status":"ended","items":3,"started":2,"passed":2}"#
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("state.json.seen.json")).unwrap(),
        "a.com\nb.com\n"
    );

    // Nothing is started again, but the two items that passed still count as passed.
    let second = fan_out(&dir, &[]);
    assert!(second.status.success(), "{}", stderr(&second));
    assert_eq!(
        stdout(&second).trim(),
        r#"{"status":"ended","items":3,"started":0,"passed":2}"#
    );
}

#[test]
#[cfg(unix)]
fn an_item_a_crash_interrupted_is_finished_by_the_next_run() {
    use std::os::unix::process::CommandExt;

    let dir = workdir("cli-fanout-crash", r#"[{"domain":"a.com"}]"#);
    // The audit task waits until `go` exists. Until then it sleeps, so the run is still in progress.
    std::fs::write(
        dir.join("audit.sh"),
        "cat > /dev/null\ntouch audit-started\nif [ -e go ]; then printf '{\"outcome\":\"pass\"}\\n'; exit 0; fi\nexec sleep 30\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("tasks.toml"),
        "[tasks]\ndiscover = [\"sh\", \"pass.sh\"]\nemit = [\"sh\", \"emit.sh\"]\naudit = [\"sh\", \"audit.sh\"]\nsave = [\"sh\", \"pass.sh\"]\n",
    )
    .unwrap();

    // The run gets a process group of its own, so the kill below also stops the waiting task.
    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_graphite"))
        .args(fan_out_args(&dir, &[]))
        .process_group(0)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let started = dir.join("audit-started");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while !started.exists() {
        assert!(
            std::time::Instant::now() < deadline,
            "the audit task never started"
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let killed = std::process::Command::new("kill")
        .arg("-KILL")
        .arg(format!("-{}", child.id()))
        .status()
        .unwrap();
    assert!(killed.success());
    child.wait().unwrap();

    std::fs::write(dir.join("go"), "").unwrap();
    let resumed = fan_out(&dir, &[]);
    assert!(resumed.status.success(), "{}", stderr(&resumed));
    assert_eq!(
        stdout(&resumed).trim(),
        r#"{"status":"ended","items":1,"started":0,"passed":1}"#
    );
}

#[test]
fn until_passed_stops_starting_items_once_enough_have_passed() {
    let dir = workdir(
        "cli-fanout-until",
        r#"[{"domain":"a.com"},{"domain":"b.com"},{"domain":"c.com"}]"#,
    );
    let output = fan_out(&dir, &["--until-passed", "1"]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(
        stdout(&output).trim(),
        r#"{"status":"ended","items":3,"started":1,"passed":1}"#
    );
}

#[test]
fn a_seen_line_cut_short_by_a_crash_does_not_start_its_item_twice() {
    let dir = workdir(
        "cli-fanout-torn-seen",
        r#"[{"domain":"a.com"},{"domain":"b.com"}]"#,
    );
    // A crash left "b.com" written without its newline: b.com was never started.
    std::fs::write(dir.join("state.json.seen.json"), "a.com\nb.com").unwrap();

    // a.com is seen but has no state, so it never ran: the run resumes it. b.com is started once.
    let first = fan_out(&dir, &[]);
    assert!(first.status.success(), "{}", stderr(&first));
    assert_eq!(
        stdout(&first).trim(),
        r#"{"status":"ended","items":2,"started":1,"passed":2}"#
    );

    // b.com was started by the first run, so a second run starts nothing, and both items still passed.
    let second = fan_out(&dir, &[]);
    assert!(second.status.success(), "{}", stderr(&second));
    assert_eq!(
        stdout(&second).trim(),
        r#"{"status":"ended","items":2,"started":0,"passed":2}"#
    );
}

#[test]
fn each_and_key_go_together_and_fan_out_flags_need_each() {
    let dir = workdir("cli-fanout-flags", "[]");
    let no_key = graphite([
        "run".as_ref(),
        dir.join("list.graphite").as_os_str(),
        "--each".as_ref(),
        dir.join("item.graphite").as_os_str(),
    ]);
    assert_eq!(no_key.status.code(), Some(2));
    assert!(stderr(&no_key).contains("--each and --key go together"));

    let no_each = graphite([
        "run".as_ref(),
        dir.join("list.graphite").as_os_str(),
        "--until-passed".as_ref(),
        "2".as_ref(),
    ]);
    assert_eq!(no_each.status.code(), Some(2));
    assert!(stderr(&no_each).contains("apply to --each only"));

    let not_a_number = fan_out(&dir, &["--until-passed", "many"]);
    assert_eq!(not_a_number.status.code(), Some(2));
    assert!(stderr(&not_a_number).contains("whole number"));
}

#[test]
fn an_item_key_that_is_not_a_plain_name_is_refused_before_it_runs() {
    let dir = workdir("cli-fanout-key", r#"[{"domain":"../escape"}]"#);
    let output = fan_out(&dir, &[]);
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("each item needs a string \"domain\""),
        "{}",
        stderr(&output)
    );
    // Refused before any item state is written.
    assert!(
        !dir.join("state.json.items")
            .join("../escape.state.json")
            .exists()
    );
}
