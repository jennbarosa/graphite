//! `graphite debug`: runs the TypeScript debugger over a compiled graph.

#[allow(dead_code)]
#[path = "support/bun.rs"]
mod bun;
#[allow(dead_code)]
#[path = "support/cli.rs"]
mod cli;

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
    assert!(stderr(&output).contains("cannot start GRAPHITE_JS_RUNNER `graphite-no-such-runner`"));
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
