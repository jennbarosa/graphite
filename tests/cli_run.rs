//! Merged test file. Each section below was `tests/<name>.rs` before; see git history.

#[allow(dead_code)]
#[path = "support/bun.rs"]
mod bun;

#[allow(dead_code)]
#[path = "support/cli.rs"]
mod cli;

#[allow(dead_code)]
#[path = "support/tools.rs"]
mod tools;

// Merged from `tests/cli_run.rs`.
mod cli_run {
    //! `graphite run`: runs a graph under the durable engine, and resumes from saved state.
    #[allow(unused_imports)]
    use super::bun;
    #[allow(unused_imports)]
    use super::cli;
    #[allow(unused_imports)]
    use super::tools;

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

    #[test]
    fn every_task_runs_its_command_and_the_input_reaches_it() {
        let pass = r#"{"outcome":"pass"}"#;
        let shipped = r#"{"outcome":"pass","output":{"shipped":true}}"#;
        let tasks = format!(
            "[tasks]\nwrite = {}\nlint = {}\nfix = {}\ndeploy = {}\n",
            logged(pass),
            logged(pass),
            logged(pass),
            logged(shipped)
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
            r#"{"status":"ended","transitions":2,"node":"ship","outcome":"pass","output":{"shipped":true}}"#
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
        let lint = r#"import os
import sys
open("log.txt", "a").write(sys.stdin.read())
if os.path.exists("marker"):
    print('{"outcome":"pass"}')
else:
    open("marker", "w").close()
    print('{"outcome":"fail","detail":{"message":"2 checks failed","code":"lint"}}')
"#;
        let pass = r#"{"outcome":"pass"}"#;
        let tasks = format!(
            "[tasks]\nwrite = {}\nlint = [\"python3\", \"lint.py\"]\nfix = {}\ndeploy = {}\n",
            logged(r#"{"outcome":"none"}"#),
            logged(pass),
            logged(pass)
        );
        let dir = workdir("cli-tasks-fail", &tasks, &[("lint.py", lint)]);

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

    /// The manifest's task names are the allowed tasks, so an allowlist with only flags is enough.
    #[test]
    fn a_flags_only_allowlist_works_when_the_manifest_names_the_tasks() {
        let pass = r#"{"outcome":"pass"}"#;
        let tasks = format!(
            "[tasks]\nwrite = {}\nlint = {}\nfix = {}\ndeploy = {}\n",
            logged(pass),
            logged(pass),
            logged(pass),
            logged(pass)
        );
        let dir = workdir(
            "cli-tasks-flags-only",
            &tasks,
            &[("flags.toml", "node_flags = [\"retry\"]\n")],
        );
        let output = graphite([
            "run".as_ref(),
            dir.join("workflow.graphite").as_os_str(),
            "--allow".as_ref(),
            dir.join("flags.toml").as_os_str(),
            "--tasks".as_ref(),
            dir.join("tasks.toml").as_os_str(),
            "--state".as_ref(),
            dir.join("state.json").as_os_str(),
        ]);
        assert!(
            output.status.success(),
            "{}{}",
            stdout(&output),
            stderr(&output)
        );
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

    /// A task command reads the outputs directory from its request, and reads an earlier stage's output
    /// from it by node name.
    #[test]
    fn a_command_reads_an_earlier_stages_output_from_the_outputs_directory() {
        let read_draft = r#"import os
import re
import sys
text = sys.stdin.read()
directory = re.findall(r'"outputs":"([^"]*)"', text)[-1]
open("log.txt", "a").write(open(os.path.join(directory, "draft.json")).read())
print('{"outcome":"pass"}')
"#;
        let tasks = format!(
            "[tasks]\nwrite = {}\nlint = [\"python3\", \"read.py\"]\nfix = {}\ndeploy = {}\n",
            logged(r#"{"outcome":"none","output":{"draft":"hi"}}"#),
            logged(r#"{"outcome":"pass"}"#),
            logged(r#"{"outcome":"pass"}"#),
        );
        let dir = workdir("cli-tasks-outputs", &tasks, &[("read.py", read_draft)]);
        let output = run(&dir, &tasks_flag(&dir));
        assert!(
            output.status.success(),
            "{}{}",
            stdout(&output),
            stderr(&output)
        );
        let log = std::fs::read_to_string(dir.join("log.txt")).unwrap();
        assert!(log.contains(r#"{"draft":"hi"}"#), "{log}");
    }

    /// A graph that declares `errored` routes a task that throws, instead of stopping the run.
    #[test]
    fn a_thrown_error_routes_to_the_errored_edge_when_the_graph_declares_it() {
        let graph = "graph review\n  node draft @task:write\n  node check @task:lint\n  node fix @task:fix\n  node ship end @task:deploy\n\n  edge draft -> check\n  edge check -> ship when passed\n  edge check -> fix when failed\n  edge check -> fix when errored\n  edge fix -> check\n";
        // The first lint throws; every later one passes.
        let lint = r#"import os
import sys
open("log.txt", "a").write(sys.stdin.read())
if os.path.exists("marker"):
    print('{"outcome":"pass"}')
else:
    open("marker", "w").close()
    print("lint crashed", file=sys.stderr)
    sys.exit(3)
"#;
        let pass = r#"{"outcome":"pass"}"#;
        let tasks = format!(
            "[tasks]\nwrite = {}\nlint = [\"python3\", \"lint.py\"]\nfix = {}\ndeploy = {}\n",
            logged(r#"{"outcome":"none"}"#),
            logged(pass),
            logged(pass),
        );
        let dir = workdir(
            "cli-tasks-errored",
            &tasks,
            &[("lint.py", lint), ("errors.graphite", graph)],
        );
        let output = graphite([
            "run".as_ref(),
            dir.join("errors.graphite").as_os_str(),
            "--allow".as_ref(),
            dir.join("workflow.toml").as_os_str(),
            "--state".as_ref(),
            dir.join("state.json").as_os_str(),
            "--tasks".as_ref(),
            dir.join("tasks.toml").as_os_str(),
        ]);
        assert!(
            output.status.success(),
            "{}{}",
            stdout(&output),
            stderr(&output)
        );
        assert!(
            stdout(&output).contains(r#""node":"ship""#),
            "{}",
            stdout(&output)
        );
        let log = std::fs::read_to_string(dir.join("log.txt")).unwrap();
        assert!(log.contains(r#""outcome":"error""#), "{log}");
        assert!(log.contains("exited with status 3"), "{log}");
    }
}

// Merged from `tests/cli_fanout.rs`.
mod cli_fanout {
    //! `graphite run --each`: one item graph run per item, the seen file, and the passing counter.
    #[allow(unused_imports)]
    use super::cli;

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
            dir.join("audit.py"),
            "import os\nimport sys\nimport time\nsys.stdin.read()\nopen(\"audit-started\", \"w\").close()\nif os.path.exists(\"go\"):\n    print('{\"outcome\":\"pass\"}')\n    sys.exit(0)\ntime.sleep(30)\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("tasks.toml"),
            "[tasks]\ndiscover = [\"python3\", \"pass.py\"]\nemit = [\"python3\", \"emit.py\"]\naudit = [\"python3\", \"audit.py\"]\nsave = [\"python3\", \"pass.py\"]\n",
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
            // `--` before the negative group id: procps kill (Ubuntu) kills the group without it but exits 1.
            .args(["-KILL", "--"])
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

        // a.com is in the seen file but this state never started it, so it is skipped. b.com's torn line is
        // dropped, so b.com is started, and only once.
        let first = fan_out(&dir, &[]);
        assert!(first.status.success(), "{}", stderr(&first));
        assert_eq!(
            stdout(&first).trim(),
            r#"{"status":"ended","items":2,"started":1,"passed":1}"#
        );

        // b.com was started by the first run, so a second run starts nothing and b.com still counts as passed.
        let second = fan_out(&dir, &[]);
        assert!(second.status.success(), "{}", stderr(&second));
        assert_eq!(
            stdout(&second).trim(),
            r#"{"status":"ended","items":2,"started":0,"passed":1}"#
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

    #[test]
    fn a_new_state_with_the_same_seen_file_runs_no_item_again() {
        let dir = workdir(
            "cli-fanout-new-state",
            r#"[{"domain":"a.com"},{"domain":"b.com"}]"#,
        );
        // Every item's save task logs one line, so a second run that repeats an item shows as an extra line.
        std::fs::write(
            dir.join("save.py"),
            "import sys\nsys.stdin.read()\nopen(\"saves.log\", \"a\").write(\"saved\\n\")\nprint('{\"outcome\":\"pass\"}')\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("tasks.toml"),
            "[tasks]\ndiscover = [\"python3\", \"pass.py\"]\nemit = [\"python3\", \"emit.py\"]\naudit = [\"python3\", \"pass.py\"]\nsave = [\"python3\", \"save.py\"]\n",
        )
        .unwrap();
        // Two days: the same seen file, a new state file each day, as the docs describe.
        let day = |state: &str| {
            let mut args = fan_out_args(&dir, &["--seen", dir.join("seen.json").to_str().unwrap()]);
            let index = args.iter().position(|arg| arg == "--state").unwrap() + 1;
            args[index] = dir.join(state).into_os_string();
            args
        };

        let monday = graphite(day("monday.state.json"));
        assert!(monday.status.success(), "{}", stderr(&monday));
        let tuesday = graphite(day("tuesday.state.json"));
        assert!(tuesday.status.success(), "{}", stderr(&tuesday));

        // Each item was saved once, on Monday. Tuesday's run starts nothing.
        assert_eq!(
            std::fs::read_to_string(dir.join("saves.log"))
                .unwrap()
                .lines()
                .count(),
            2
        );
        assert_eq!(
            stdout(&tuesday).trim(),
            r#"{"status":"ended","items":2,"started":0,"passed":0}"#
        );
    }
}

// Merged from `tests/cli_retry.rs`.
mod cli_retry {
    //! `graphite run --retry <key>`: a task that threw stops the run, the next run refuses to run it
    //! again, and `--retry` clears its stored error so the run runs it once more.
    #[allow(unused_imports)]
    use super::cli;

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
        std::fs::write(
            dir.join("audit.py"),
            "import sys\nsys.stdin.read()\nsys.exit(1)\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("tasks.toml"),
            "[tasks]\naudit = [\"python3\", \"audit.py\"]\n",
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
}

// Merged from `tests/cli_debug.rs`.
mod cli_debug {
    //! `graphite debug`: runs the TypeScript debugger over a compiled graph.
    #[allow(unused_imports)]
    use super::bun;
    #[allow(unused_imports)]
    use super::cli;
    #[allow(unused_imports)]
    use super::tools;

    use cli::{debugging_front_ends, dsl, graphite, manifest, scratch, stderr, stdout, walk_spec};

    /// A graph where task `t` fails and nothing routes the failure: the run stalls at `a`.
    const STALLS_ON_FAILURE: &str = "graph g\n  node a @task:t\n  node b\n  node c end\n\n  edge a -> b when passed\n  edge b -> c when failed\n";

    /// The review example, with its allowlist, debugged with `extra` arguments.
    fn simulate_review(extra: &[&str]) -> std::process::Output {
        let example = manifest().join("examples/workflow/workflow.graphite");
        let allow = manifest().join("examples/workflow/workflow.toml");
        let mut args: Vec<std::ffi::OsString> = vec![
            "debug".into(),
            example.into_os_string(),
            "--allow".into(),
            allow.into_os_string(),
        ];
        args.extend(extra.iter().map(|arg| (*arg).into()));
        graphite(args)
    }

    #[test]
    fn simulate_walks_the_review_graph_to_its_end_without_running_tasks() {
        let output = simulate_review(&["--simulate", "lint=fail,pass"]);
        assert!(output.status.success(), "{}", stderr(&output));
        let text = stdout(&output);
        assert!(
            text.starts_with("no stub, so pass: write, fix, deploy\n"),
            "{text}"
        );
        // lint fails once, so the graph goes around the fix loop once: four edges in all, then ships.
        assert!(
            text.contains("enter fix\n  fix -> pass\nenter check\n  lint -> pass\nenter ship\n"),
            "{text}"
        );
        assert!(
            text.trim_end()
                .ends_with("ended at ship, outcome pass, after 4 transitions"),
            "{text}"
        );
    }

    #[test]
    fn simulate_reports_the_node_a_stall_leaves_behind() {
        let spec = dsl("cli-debug-stall", STALLS_ON_FAILURE);
        let output = graphite([
            "debug".as_ref(),
            spec.as_os_str(),
            "--simulate".as_ref(),
            "t=fail".as_ref(),
        ]);
        assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
        assert!(
            stdout(&output).contains("stalled at a: no edge from it is taken"),
            "{}",
            stdout(&output)
        );
    }

    #[test]
    fn simulate_stops_a_loop_that_never_ends_at_the_transition_limit() {
        let output = simulate_review(&["--simulate", "lint=fail"]);
        assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
        let text = stdout(&output);
        assert!(text.contains("... more events not shown"), "{text}");
        assert!(
            text.contains("run reached maxTransitions (10000)"),
            "{text}"
        );
    }

    #[test]
    fn simulate_refuses_an_unknown_task_and_an_unknown_outcome() {
        let spec = dsl("cli-debug-stubs", STALLS_ON_FAILURE);
        let unknown = graphite([
            "debug".as_ref(),
            spec.as_os_str(),
            "--simulate".as_ref(),
            "nope=pass".as_ref(),
        ]);
        assert_eq!(unknown.status.code(), Some(1));
        assert!(
            stderr(&unknown).contains("no task \"nope\" in the graph; its tasks are t"),
            "{}",
            stderr(&unknown)
        );

        let outcome = graphite([
            "debug".as_ref(),
            spec.as_os_str(),
            "--simulate".as_ref(),
            "t=maybe".as_ref(),
        ]);
        assert!(
            stderr(&outcome).contains("must be <task>=<outcomes>"),
            "{}",
            stderr(&outcome)
        );
    }

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
    fn runs_the_commands_against_the_first_graph() {
        let spec = walk_spec();
        for front_end in debugging_front_ends() {
            let output = graphite([
                "debug".as_ref(),
                spec.as_os_str(),
                "--frontend".as_ref(),
                front_end.as_ref(),
                "finish".as_ref(),
                "step".as_ref(),
            ]);
            assert!(output.status.success(), "{front_end}: {}", stderr(&output));
            let text = stdout(&output);
            assert!(
                text.starts_with("node windup  status running  transitions 0  true: none"),
                "{front_end}: {text}"
            );
            assert!(
                text.contains("node going  status running  transitions 1"),
                "{front_end}: {text}"
            );
        }
    }

    #[test]
    fn selects_a_graph_by_name_and_names_the_choices_when_it_is_missing() {
        let spec = walk_spec();
        for front_end in debugging_front_ends() {
            let output = graphite([
                "debug".as_ref(),
                spec.as_os_str(),
                "--frontend".as_ref(),
                front_end.as_ref(),
                "--graph".as_ref(),
                "walk".as_ref(),
                "choose facing".as_ref(),
            ]);
            assert!(output.status.success(), "{front_end}: {}", stderr(&output));
            assert!(
                stdout(&output).contains("facing: else -> down"),
                "{front_end}"
            );
        }

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
        let spec = walk_spec();
        for front_end in debugging_front_ends() {
            let out = scratch(&format!("cli-debug-out-{front_end}"));
            let output = graphite([
                "debug".as_ref(),
                spec.as_os_str(),
                "--frontend".as_ref(),
                front_end.as_ref(),
                "--out".as_ref(),
                out.as_os_str(),
                "quit".as_ref(),
            ]);
            assert!(output.status.success(), "{front_end}: {}", stderr(&output));
            let module = match front_end {
                "typescript" => "walk.ts",
                "rust" => "walk.rs",
                other => panic!("no generated module name is known for {other}"),
            };
            assert!(out.join(module).is_file(), "{front_end} keeps {module}");
        }
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
        assert!(
            stderr(&output).contains("cannot start GRAPHITE_JS_RUNNER `graphite-no-such-runner`")
        );
    }

    /// With no command arguments, the debugger reads one command per line from standard input, and
    /// prints what the same commands print as arguments.
    #[test]
    fn reads_commands_from_standard_input_when_none_are_given() {
        use std::io::Write;
        use std::process::{Command, Stdio};

        let spec = walk_spec();
        for front_end in debugging_front_ends() {
            let from_args = graphite([
                "debug".as_ref(),
                spec.as_os_str(),
                "--frontend".as_ref(),
                front_end.as_ref(),
                "finish".as_ref(),
                "step".as_ref(),
            ]);
            assert!(
                from_args.status.success(),
                "{front_end}: {}",
                stderr(&from_args)
            );

            let mut child = Command::new(env!("CARGO_BIN_EXE_graphite"))
                .args([
                    "debug".as_ref(),
                    spec.as_os_str(),
                    "--frontend".as_ref(),
                    front_end.as_ref(),
                ])
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
            child
                .stdin
                .take()
                .unwrap()
                .write_all(b"finish\nstep\n")
                .unwrap();
            let from_stdin = child.wait_with_output().unwrap();
            assert!(
                from_stdin.status.success(),
                "{front_end}: {}",
                stderr(&from_stdin)
            );
            assert_eq!(stdout(&from_stdin), stdout(&from_args), "{front_end}");
        }
    }

    /// `--allow` applies to `debug`: the walk fixture uses no domain flags, so an allowlist that
    /// names one is accepted; the workflow's `retry` flag is refused without its allowlist.
    #[test]
    fn debug_applies_the_allowlist() {
        let fixtures = manifest().join("tests/fixtures/runtime");
        let dsl = fixtures.join("workflow.graphite");

        let refused = graphite(["debug".as_ref(), dsl.as_os_str(), "show".as_ref()]);
        assert!(!refused.status.success());
        assert!(stderr(&refused).contains("unknown node flag"));

        let accepted = graphite([
            "debug".as_ref(),
            dsl.as_os_str(),
            "--allow".as_ref(),
            fixtures.join("workflow.toml").as_os_str(),
            "show".as_ref(),
        ]);
        assert!(accepted.status.success(), "{}", stderr(&accepted));
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

        let working = Command::new(env!("CARGO_BIN_EXE_graphite"))
            .args(["debug".as_ref(), spec.as_os_str(), "show".as_ref()])
            .env("GRAPHITE_JS_RUNNER", "bun")
            .env("PATH", tools::path_with_tools())
            .output()
            .unwrap();
        assert!(working.status.success(), "{}", stderr(&working));
        assert!(stdout(&working).contains("node windup"));
    }

    #[test]
    fn simulate_names_a_graph_whose_tasks_have_no_pass_and_fail_routes() {
        let spec = dsl(
            "cli-debug-no-routes",
            "graph g\n  node a @task:t\n  node b end\n\n  edge a -> b when passed\n",
        );
        let output = graphite([
            "debug".as_ref(),
            spec.as_os_str(),
            "--simulate".as_ref(),
            "t=pass".as_ref(),
        ]);
        assert!(!output.status.success());
        assert!(
            stderr(&output).contains(
                "simulate: the graph has tasks (node \"a\" among them), so it needs outcomeConditions"
            ),
            "{}",
            stderr(&output)
        );
    }

    #[test]
    fn a_thrown_task_takes_the_errored_route_when_the_graph_declares_it() {
        let spec = dsl(
            "cli-debug-errored",
            "graph g\n  node a @task:t\n  node b end\n  node c end\n\n  edge a -> b when passed\n  edge a -> c when failed\n  edge a -> c when errored\n",
        );
        let output = graphite([
            "debug".as_ref(),
            spec.as_os_str(),
            "--simulate".as_ref(),
            "t=throw".as_ref(),
        ]);
        assert!(output.status.success(), "{}", stderr(&output));
        let text = stdout(&output);
        assert!(text.contains("  t -> error\n"), "{text}");
        assert!(
            text.trim_end()
                .ends_with("ended at c, outcome none, after 1 transitions"),
            "{text}"
        );
    }

    #[test]
    fn simulate_writes_the_generated_modules_to_out_when_given() {
        let example = manifest().join("examples/workflow/workflow.graphite");
        let allow = manifest().join("examples/workflow/workflow.toml");
        let out = scratch("cli-debug-simulate-out");
        let output = graphite([
            "debug".as_ref(),
            example.as_os_str(),
            "--allow".as_ref(),
            allow.as_os_str(),
            "--out".as_ref(),
            out.as_os_str(),
            "--simulate".as_ref(),
            "lint=pass".as_ref(),
        ]);
        assert!(output.status.success(), "{}", stderr(&output));
        assert!(std::fs::read_dir(&out).unwrap().count() > 0);
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
