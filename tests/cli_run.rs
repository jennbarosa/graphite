//! Merged test file. Each section below was `tests/<name>.rs` before; see git history.

#[allow(dead_code)]
#[path = "support/cli.rs"]
mod cli;

// Merged from `tests/cli_run.rs`.
mod cli_run {
    //! `graphite run`: runs a graph under the durable engine, and resumes from saved state.
    #[allow(unused_imports)]
    use super::cli;

    use cli::{graphite, stderr, stdout, walk_spec};

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
}

// Merged from `tests/cli_start.rs`.
mod cli_start {
    //! First-run commands: help, `init`, default output directory, and `doctor`.
    #[allow(unused_imports)]
    use super::cli;

    use std::process::Command;

    use cli::{dsl, graphite, scratch, stderr, stdout};

    #[test]
    fn help_goes_to_stdout_and_succeeds_in_every_form() {
        let overview = graphite(["--help"]);
        assert!(overview.status.success());
        assert!(stdout(&overview).contains("graphite init <file.graphite>"));

        let per_command = graphite(["compile", "x.graphite", "--help"]);
        assert!(per_command.status.success());
        assert!(stdout(&per_command).contains("--out defaults to graphite-out/"));

        let named = graphite(["help", "run"]);
        assert!(named.status.success());
        assert!(stdout(&named).contains("--retry <key>"));
    }

    #[test]
    fn an_unknown_help_topic_fails_with_the_overview() {
        let output = graphite(["help", "bogus"]);
        assert!(!output.status.success());
        assert!(stderr(&output).contains("no help for \"bogus\""));
    }

    #[test]
    fn init_writes_a_starter_graph_that_checks_and_refuses_to_overwrite() {
        let path = scratch("start-init").join("hello.graphite");
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
        let source = dsl(
            "start-default-out/door.graphite",
            "graph door\n  node a end\n",
        );
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
            text.contains("summary: blocked: typescript debug, typescript run, rust debug"),
            "{text}"
        );
        assert!(stderr(&output).contains("some commands cannot run here"));

        let json = Command::new(env!("CARGO_BIN_EXE_graphite"))
            .args(["doctor", "--json"])
            .env_clear()
            .env("PATH", &empty)
            .output()
            .unwrap();
        let line = stdout(&json);
        let line = line.trim_end();
        assert_eq!(json.status.code(), Some(1));
        assert!(line.starts_with(r#"{"ok":false,"#), "{line}");
        assert!(
            line.ends_with(r#""blocked":["typescript debug","typescript run","rust debug"]}"#),
            "{line}"
        );
        assert!(stderr(&json).is_empty());
    }
}

// Merged from `tests/cli_tasks.rs`.
mod cli_tasks {
    //! `graphite run --tasks` and `--input`: command-backed tasks, the input every task receives, and
    //! the refusals and errors around them.
    #[allow(unused_imports)]
    use super::cli;

    use std::ffi::OsString;
    use std::path::{Path, PathBuf};

    use cli::{graphite, manifest, scratch, stderr, stdout};

    /// A script that appends its stdin to `log.txt`, then prints its first argument: the report.
    const LOG_SCRIPT: &str = r#"import sys
open("log.txt", "a").write(sys.stdin.read())
print(sys.argv[1])
"#;

    /// A fresh working directory with the review workflow, its allowlist, `tasks.toml`, and any
    /// extra files as `(name, contents)`.
    fn workdir(name: &str, tasks: &str, files: &[(&str, &str)]) -> PathBuf {
        let dir = scratch(name);
        std::fs::create_dir_all(&dir).unwrap();
        let fixtures = manifest().join("tests/fixtures/runtime");
        std::fs::copy(
            fixtures.join("workflow.graphite"),
            dir.join("workflow.graphite"),
        )
        .unwrap();
        std::fs::copy(fixtures.join("workflow.toml"), dir.join("workflow.toml")).unwrap();
        std::fs::write(dir.join("tasks.toml"), tasks).unwrap();
        std::fs::write(dir.join("log.py"), LOG_SCRIPT).unwrap();
        for (file, contents) in files {
            std::fs::write(dir.join(file), contents).unwrap();
        }
        dir
    }

    /// A task that runs `log.py` and answers with `report`, a JSON line.
    fn logged(report: &str) -> String {
        format!("[\"python3\", \"log.py\", '{report}']")
    }

    /// `graphite run` on the workdir's workflow, with the allowlist and state file, plus `extra`.
    fn run(dir: &Path, extra: &[OsString]) -> std::process::Output {
        let mut args: Vec<OsString> = vec![
            "run".into(),
            dir.join("workflow.graphite").into_os_string(),
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

    /// The durable engine end to end: a task that throws stops the run and stores its error, a later
    /// run refuses to run it again, and `--retry` clears that error so the run finishes. It needs a
    /// JavaScript runtime, so it is skipped without one, and fails in CI, where Bun is installed.
    #[test]
    fn a_thrown_task_stops_the_run_and_retry_lets_it_finish() {
        const PASS: &str = "import sys\nsys.stdin.read()\nprint('{\"outcome\":\"pass\"}')\n";
        // The lint task throws until the `ok` file exists.
        const LINT: &str = "import os\nimport sys\nsys.stdin.read()\nif os.path.exists(sys.argv[1]):\n    print('{\"outcome\":\"pass\"}')\n    sys.exit(0)\nsys.exit(1)\n";
        let dir = workdir(
            "cli-tasks-retry",
            "[tasks]\n",
            &[("pass.py", PASS), ("lint.py", LINT)],
        );
        let path = |name: &str| format!("'{}'", dir.join(name).display());
        let tasks = format!(
            "[tasks]\nwrite = [\"python3\", {pass}]\nlint = [\"python3\", {lint}, {ok}]\nfix = [\"python3\", {pass}]\ndeploy = [\"python3\", {pass}]\n",
            pass = path("pass.py"),
            lint = path("lint.py"),
            ok = path("ok"),
        );
        std::fs::write(dir.join("tasks.toml"), tasks).unwrap();

        let first = run(&dir, &tasks_flag(&dir));
        if cli::skip_without_js_runtime(&first) {
            eprintln!("skipped: no JavaScript runtime (install Bun)");
            return;
        }
        assert!(!first.status.success(), "{}", stdout(&first));
        assert!(
            stdout(&first).contains("\"key\":\"1:check\""),
            "{}",
            stdout(&first)
        );

        let again = run(&dir, &tasks_flag(&dir));
        assert!(!again.status.success());
        assert!(
            stdout(&again).contains("failed in an earlier run"),
            "{}",
            stdout(&again)
        );

        std::fs::write(dir.join("ok"), "").unwrap();
        let mut extra = tasks_flag(&dir).to_vec();
        extra.extend(["--retry".into(), "1:check".into()]);
        let retried = run(&dir, &extra);
        assert!(retried.status.success(), "{}", stderr(&retried));
        assert!(
            stdout(&retried).contains("\"status\":\"ended\""),
            "{}",
            stdout(&retried)
        );
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
    fn a_task_missing_from_the_manifest_is_refused_before_anything_is_recorded() {
        let dir = workdir(
            "cli-tasks-missing",
            &format!("[tasks]\nwrite = {}\n", logged("{}")),
            &[],
        );
        let output = run(&dir, &tasks_flag(&dir));
        assert!(!output.status.success());
        assert!(
            stderr(&output).contains("@task:lint is not allowed; allowed: write"),
            "{}",
            stderr(&output)
        );
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
            dir.join("workflow.graphite").as_os_str(),
            "--input".as_ref(),
            dir.join("workflow.toml").as_os_str(),
        ]);
        assert_eq!(output.status.code(), Some(2));
        assert!(stderr(&output).contains("check does not take --input"));
    }
}

// Merged from `tests/cli_fanout.rs`.
mod cli_fanout {
    //! `graphite run --each`: one item graph run per item, the seen file, and the passing counter.
    #[allow(unused_imports)]
    use super::cli;

    use std::path::{Path, PathBuf};

    use cli::{graphite, scratch, stderr};

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
            dir.join("pass.py"),
            "import sys\nsys.stdin.read()\nprint('{\"outcome\":\"pass\"}')\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("emit.py"),
            "import sys\nsys.stdin.read()\nsys.stdout.write('{\"outcome\":\"pass\",\"output\":' + open(\"items.json\").read() + '}\\n')\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("tasks.toml"),
            "[tasks]\ndiscover = [\"python3\", \"pass.py\"]\nemit = [\"python3\", \"emit.py\"]\naudit = [\"python3\", \"pass.py\"]\nsave = [\"python3\", \"pass.py\"]\n",
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

    /// `--until-passed` counts the items that passed in earlier runs of the same state. The first run
    /// starts one item and stops at the limit; a second run with the same state starts no new item.
    #[test]
    fn until_passed_counts_items_that_passed_in_an_earlier_run() {
        let dir = workdir(
            "cli-fanout-counter",
            r#"[{"domain":"a.test"},{"domain":"b.test"}]"#,
        );
        let seen = dir.join("state.json.seen.json");

        let first = fan_out(&dir, &["--until-passed", "1"]);
        if cli::skip_without_js_runtime(&first) {
            eprintln!("skipped: no JavaScript runtime (install Bun)");
            return;
        }
        assert!(first.status.success(), "{}", stderr(&first));
        assert_eq!(std::fs::read_to_string(&seen).unwrap().lines().count(), 1);

        let second = fan_out(&dir, &["--until-passed", "1"]);
        assert!(second.status.success(), "{}", stderr(&second));
        assert_eq!(std::fs::read_to_string(&seen).unwrap().lines().count(), 1);
    }
}

// Merged from `tests/cli_retry.rs`.
mod cli_retry {
    //! `graphite run --retry <key>`: a task that threw stops the run, the next run refuses to run it
    //! again, and `--retry` clears its stored error so the run runs it once more.
    #[allow(unused_imports)]
    use super::cli;

    use cli::{graphite, scratch, stderr};

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
            dir.join("audit.py"),
            "import os\nimport sys\nsys.stdin.read()\nif os.path.exists(\"ok\"):\n    print('{\"outcome\":\"pass\"}')\n    sys.exit(0)\nsys.exit(1)\n",
        )
        .unwrap();
        std::fs::write(dir.join("pass.py"), "import sys\nsys.stdin.read()\n").unwrap();
        std::fs::write(
            dir.join("tasks.toml"),
            "[tasks]\naudit = [\"python3\", \"audit.py\"]\nsave = [\"python3\", \"pass.py\"]\n",
        )
        .unwrap();
        dir
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
}

// Merged from `tests/cli_debug.rs`.
mod cli_debug {
    //! `graphite debug`: runs the TypeScript debugger over a compiled graph.
    #[allow(unused_imports)]
    use super::cli;

    use cli::{dsl, graphite, manifest, scratch, stderr, stdout, walk_spec};

    /// The one end-to-end check of generated code: the door graph is compiled to TypeScript and
    /// stepped under a real JavaScript runtime. It is skipped on a machine with no runtime (Bun, or
    /// `npx tsx`), and fails in CI, where a runtime is installed, so the check cannot rot unseen.
    #[test]
    fn the_generated_typescript_runs_under_a_real_runtime() {
        let door = manifest().join("examples/door.graphite");
        let output = graphite([
            "debug".as_ref(),
            door.as_os_str(),
            "set request on".as_ref(),
            "step".as_ref(),
            "show".as_ref(),
        ]);
        if cli::skip_without_js_runtime(&output) {
            eprintln!("skipped: no JavaScript runtime (install Bun)");
            return;
        }
        assert!(output.status.success(), "{}", stderr(&output));
        assert!(
            stdout(&output).contains("node opening"),
            "{}",
            stdout(&output)
        );
    }

    /// A graph where task `t` fails and nothing routes the failure: the run stalls at `a`.
    const STALLS_ON_FAILURE: &str = "graph g\n  node a @task:t\n  node b\n  node c end\n\n  edge a -> b when passed\n  edge b -> c when failed\n";

    #[test]
    fn simulate_takes_no_commands_and_belongs_to_debug_alone() {
        let spec = dsl("cli-debug-flags", STALLS_ON_FAILURE);
        let with_commands = graphite([
            "debug".as_ref(),
            spec.as_os_str(),
            "--simulate".as_ref(),
            "t=pass".as_ref(),
            "finish".as_ref(),
        ]);
        assert_eq!(with_commands.status.code(), Some(2));
        assert!(stderr(&with_commands).contains("debug --simulate takes no commands"));

        let elsewhere = graphite([
            "check".as_ref(),
            spec.as_os_str(),
            "--simulate".as_ref(),
            "t=pass".as_ref(),
        ]);
        assert_eq!(elsewhere.status.code(), Some(2));
        assert!(stderr(&elsewhere).contains("check does not take --simulate"));
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
        assert!(
            stderr(&output).contains("cannot start GRAPHITE_JS_RUNNER `graphite-no-such-runner`")
        );
    }

    /// `GRAPHITE_JS_RUNNER` chooses the JavaScript runtime. A runtime that cannot start is named in
    /// the error, and a working one runs the debugger.
    #[test]
    fn graphite_js_runner_selects_the_runtime() {
        use std::process::Command;

        let spec = walk_spec();
        let missing = Command::new(env!("CARGO_BIN_EXE_graphite"))
            .args(["debug".as_ref(), spec.as_os_str(), "show".as_ref()])
            .env("GRAPHITE_JS_RUNNER", "graphite-no-such-runtime")
            .output()
            .unwrap();
        assert!(!missing.status.success());
        assert!(
            stderr(&missing).contains("cannot start GRAPHITE_JS_RUNNER `graphite-no-such-runtime`")
        );
    }

    #[test]
    fn simulate_needs_a_value() {
        let spec = dsl("cli-debug-no-value", STALLS_ON_FAILURE);
        let output = graphite(["debug".as_ref(), spec.as_os_str(), "--simulate".as_ref()]);
        assert_eq!(output.status.code(), Some(2));
        assert!(stderr(&output).contains("--simulate needs a value"));
    }

    /// The Rust debugger writes two helper files beside the graph. An existing file there is the user's,
    /// so the debugger must refuse rather than replace it.
    #[test]
    fn the_rust_debugger_refuses_to_replace_an_existing_file() {
        let out = scratch("cli-debug-keep");
        std::fs::create_dir_all(&out).unwrap();
        std::fs::write(out.join("graphite_debugger.rs"), "keep").unwrap();
        let spec = dsl("cli-debug-keep-src", "graph door\n  node a end\n");
        let output = graphite([
            "debug".as_ref(),
            spec.as_os_str(),
            "--frontend".as_ref(),
            "rust".as_ref(),
            "--out".as_ref(),
            out.as_os_str(),
            "quit".as_ref(),
        ]);
        assert!(!output.status.success());
        assert!(
            stderr(&output).contains("cannot write"),
            "{}",
            stderr(&output)
        );
        assert_eq!(
            std::fs::read_to_string(out.join("graphite_debugger.rs")).unwrap(),
            "keep"
        );
    }
}
