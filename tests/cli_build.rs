//! Merged test file. Each section below was `tests/<name>.rs` before; see git history.

#[allow(dead_code)]
#[path = "support/cli.rs"]
mod cli;

// Merged from `tests/cli_check.rs`.
mod cli_check {
    //! `graphite check`: validates a file and its output names, and writes nothing.
    #[allow(unused_imports)]
    use super::cli;

    use cli::{dsl, graphite, scratch, stderr, stdout, walk_spec};

    #[test]
    fn reports_the_graph_count() {
        let spec = walk_spec();
        let output = graphite(["check".as_ref(), spec.as_os_str()]);
        assert!(output.status.success(), "{}", stderr(&output));
        assert_eq!(stdout(&output), "ok: 1 graph\n");
    }

    #[test]
    fn rejects_two_graphs_whose_outputs_collide() {
        let source = dsl(
            "cli-collide/collide.graphite",
            "graph Walk\n  node a end\n\ngraph walk\n  node a end\n",
        );
        let output = graphite(["check".as_ref(), source.as_os_str()]);
        assert!(!output.status.success());
        assert!(stderr(&output).starts_with("graphite:"));
    }

    #[test]
    fn rejects_a_dead_end_with_its_source_location() {
        let source = dsl("cli-dead-end/dead.graphite", "graph bad\n  node a\n");
        let output = graphite(["check".as_ref(), source.as_os_str()]);
        assert!(!output.status.success());
        assert_eq!(output.status.code(), Some(1));
        assert!(stderr(&output).starts_with("graphite:"));
    }

    #[test]
    fn takes_exactly_one_file() {
        let spec = walk_spec();
        let output = graphite([
            "check".as_ref(),
            spec.as_os_str(),
            "extra.graphite".as_ref(),
        ]);
        assert!(!output.status.success());
        assert!(stderr(&output).contains("check takes one file"));
    }

    const REVIEW: &str = "graph review\n  node draft @task:write\n  node check @task:lint\n  node ship end @task:deploy\n  edge draft -> check\n  edge check -> ship when passed @report:ok\n";

    #[test]
    fn allow_accepts_values_on_the_list() {
        let source = dsl("cli-allow-ok/review.graphite", REVIEW);
        let allow = dsl(
            "cli-allow-ok/allow.toml",
            "[annotations]\ntask = [\"write\", \"lint\", \"deploy\"]\nreport = [\"ok\"]\n",
        );
        let output = graphite([
            "check".as_ref(),
            source.as_os_str(),
            "--allow".as_ref(),
            allow.as_os_str(),
        ]);
        assert!(output.status.success(), "{}", stderr(&output));
        assert_eq!(stdout(&output), "ok: 1 graph\n");
    }

    #[test]
    fn allow_names_the_node_holding_a_value_off_the_list() {
        let source = dsl("cli-allow-bad/review.graphite", REVIEW);
        let allow = dsl(
            "cli-allow-bad/allow.toml",
            "[annotations]\ntask = [\"write\", \"lint\"]\n",
        );
        let output = graphite([
            "check".as_ref(),
            source.as_os_str(),
            "--allow".as_ref(),
            allow.as_os_str(),
        ]);
        assert!(!output.status.success());
        assert!(stderr(&output).contains("graph review, node ship: @task:deploy is not allowed"));
    }

    #[test]
    fn allow_refuses_a_bare_annotation_under_a_listed_key() {
        let source = dsl(
            "cli-allow-bare/bare.graphite",
            "graph bare\n  node draft @task\n  node done end\n  edge draft -> done\n",
        );
        let allow = dsl(
            "cli-allow-bare/allow.toml",
            "[annotations]\ntask = [\"write\"]\n",
        );
        let output = graphite([
            "check".as_ref(),
            source.as_os_str(),
            "--allow".as_ref(),
            allow.as_os_str(),
        ]);
        assert!(!output.status.success());
        assert!(stderr(&output).contains("@task needs a value"));
    }

    #[test]
    fn allow_rejects_sections_it_does_not_know() {
        let source = dsl("cli-allow-section/review.graphite", REVIEW);
        let allow = dsl(
            "cli-allow-section/allow.toml",
            "[annotations]\ntask = [\"write\"]\n[extra]\nx = 1\n",
        );
        let output = graphite([
            "check".as_ref(),
            source.as_os_str(),
            "--allow".as_ref(),
            allow.as_os_str(),
        ]);
        assert!(!output.status.success());
        assert!(stderr(&output).contains("unknown key [extra]"));
    }

    /// `check --json` prints `{"ok":true,"graphs":N}` when the graph checks, and
    /// `{"ok":false,"error":...}` on stdout, with exit status 1, when it does not.
    #[test]
    fn check_json_reports_the_graph_count_or_the_error() {
        let spec = walk_spec();
        let good = graphite(["check".as_ref(), spec.as_os_str(), "--json".as_ref()]);
        assert!(good.status.success());
        assert_eq!(stdout(&good).trim(), r#"{"ok":true,"graphs":1}"#);

        let missing = graphite(["check", "no-such-file.graphite", "--json"]);
        assert_eq!(missing.status.code(), Some(1));
        assert!(stderr(&missing).is_empty());
        let line = stdout(&missing);
        assert!(
            line.starts_with(r#"{"ok":false,"error":"no-such-file.graphite"#),
            "{line}"
        );
    }

    #[test]
    fn an_allowlist_also_refuses_a_value_on_the_graph_line() {
        let dir = scratch("cli-check-graph-line-allow");
        std::fs::create_dir_all(&dir).unwrap();
        let graph = dir.join("door.graphite");
        std::fs::write(
            &graph,
            "graph door @anim:bogus\n  node closed @anim:idle\n  node open end @anim:idle\n\n  edge closed -> open\n",
        )
        .unwrap();
        let allow = dir.join("allow.toml");
        std::fs::write(&allow, "[annotations]\nanim = [\"idle\"]\n").unwrap();

        let output = graphite([
            "check".as_ref(),
            graph.as_os_str(),
            "--allow".as_ref(),
            allow.as_os_str(),
        ]);
        assert_eq!(output.status.code(), Some(1), "{}", stdout(&output));
        assert!(
            stderr(&output).contains("graph door: @anim:bogus is not allowed"),
            "{}",
            stderr(&output)
        );
    }

    #[test]
    fn an_allowlist_also_refuses_values_on_choice_arms_and_edges() {
        let dir = scratch("cli-check-arm-edge-allow");
        std::fs::create_dir_all(&dir).unwrap();
        let allow = dir.join("allow.toml");
        std::fs::write(&allow, "[annotations]\nanim = [\"idle\"]\n").unwrap();
        // Each case puts one bad value in a different place; the check must name that place.
        let cases = [
            (
                "choice",
                "graph door\n  node a @anim:idle\n  node b end @anim:idle\n\n  edge a -> b\n\n  choose facing\n    when left -> b @anim:bogus\n    else -> b\n",
                "graph door, choice facing: @anim:bogus is not allowed",
            ),
            (
                "edge",
                "graph door\n  node a @anim:idle\n  node b end @anim:idle\n\n  edge a -> b @anim:bogus\n",
                "graph door, edge a -> b: @anim:bogus is not allowed",
            ),
        ];
        for (name, source, expected) in cases {
            let graph = dir.join(format!("{name}.graphite"));
            std::fs::write(&graph, source).unwrap();
            let output = graphite([
                "check".as_ref(),
                graph.as_os_str(),
                "--allow".as_ref(),
                allow.as_os_str(),
            ]);
            assert_eq!(output.status.code(), Some(1), "{name}: {}", stdout(&output));
            assert!(
                stderr(&output).contains(expected),
                "{name}: {}",
                stderr(&output)
            );
        }
    }
}

// Merged from `tests/cli_compile.rs`.
mod cli_compile {
    //! `graphite compile`: writes the runtime and graph modules, and never overwrites.
    #[allow(unused_imports)]
    use super::cli;

    use cli::{graphite, manifest, scratch, stderr, walk_spec};

    #[test]
    fn writes_the_runtime_and_the_graph_modules() {
        let out = scratch("cli-compile");
        let spec = walk_spec();
        let output = graphite([
            "compile".as_ref(),
            spec.as_os_str(),
            "--out".as_ref(),
            out.as_os_str(),
        ]);
        assert!(output.status.success(), "{}", stderr(&output));
        assert!(out.join("graphite.ts").is_file());
        assert!(out.join("walk.ts").is_file());
    }

    #[test]
    fn refuses_to_overwrite_an_existing_output() {
        let out = scratch("cli-overwrite");
        let spec = walk_spec();
        let args = [
            "compile".as_ref(),
            spec.as_os_str(),
            "--out".as_ref(),
            out.as_os_str(),
        ];
        assert!(graphite(args).status.success());
        assert!(!graphite(args).status.success());
    }

    #[test]
    fn rejects_an_unknown_front_end_without_writing() {
        let out = scratch("cli-unknown-front-end");
        let spec = walk_spec();
        let output = graphite([
            "compile".as_ref(),
            spec.as_os_str(),
            "--frontend".as_ref(),
            "cobol".as_ref(),
            "--out".as_ref(),
            out.as_os_str(),
        ]);
        assert!(!output.status.success());
        assert!(stderr(&output).contains("unknown front end \"cobol\""));
        assert!(!out.exists());
    }

    /// `--allow` applies to `compile`: a graph that uses an unlisted flag is refused, and the same graph
    /// compiles once the allowlist names the flag. `--frontend typescript` names the default explicitly.
    #[test]
    fn compile_applies_the_allowlist_and_accepts_the_front_end_by_name() {
        let fixtures = manifest().join("tests/fixtures/runtime");
        let dsl = fixtures.join("workflow.graphite");
        let allow = fixtures.join("workflow.toml");
        let out = scratch("cli-compile-allow");

        let refused = graphite([
            "compile".as_ref(),
            dsl.as_os_str(),
            "--out".as_ref(),
            out.as_os_str(),
        ]);
        assert!(!refused.status.success());
        assert!(stderr(&refused).contains("unknown node flag"));
        assert!(!out.exists());

        let accepted = graphite([
            "compile".as_ref(),
            dsl.as_os_str(),
            "--frontend".as_ref(),
            "typescript".as_ref(),
            "--allow".as_ref(),
            allow.as_os_str(),
            "--out".as_ref(),
            out.as_os_str(),
        ]);
        assert!(accepted.status.success(), "{}", stderr(&accepted));
        assert!(out.join("review.ts").exists());
    }
}

// Merged from `tests/cli_inspect.rs`.
mod cli_inspect {
    //! `graphite inspect`: prints a graph's structure as text.
    #[allow(unused_imports)]
    use super::cli;

    use cli::{dsl, graphite, stderr, stdout, walk_spec};

    #[test]
    fn lists_nodes_conditions_choices_and_edges() {
        let spec = walk_spec();
        let output = graphite(["inspect".as_ref(), spec.as_os_str()]);
        assert!(output.status.success(), "{}", stderr(&output));
        let text = stdout(&output);
        for expected in [
            "graph walk @choice:facing @idle:stopped",
            "    going loop @anim:walk @group:locomotion",
            "  conditions: keyup, left, right, up",
            "      when left -> side @flip:true",
            "      else -> down",
            "    windup -> stopped when keyup",
            "    windup -> going",
        ] {
            assert!(
                text.lines().any(|line| line == expected),
                "missing {expected:?} in:\n{text}"
            );
        }
    }

    #[test]
    fn refuses_out() {
        let spec = walk_spec();
        let output = graphite([
            "inspect".as_ref(),
            spec.as_os_str(),
            "--out".as_ref(),
            "x".as_ref(),
        ]);
        assert!(!output.status.success());
        assert!(stderr(&output).contains("inspect does not take --out"));
    }

    #[test]
    fn refuses_frontend_even_when_it_is_the_default() {
        let spec = walk_spec();
        let output = graphite([
            "inspect".as_ref(),
            spec.as_os_str(),
            "--frontend".as_ref(),
            "typescript".as_ref(),
        ]);
        assert!(!output.status.success());
        assert!(stderr(&output).contains("inspect does not take --frontend"));
    }

    /// `inspect --json` carries the same facts as the text form, and an `else` arm's condition is `null`.
    #[test]
    fn inspect_json_describes_the_graph_with_null_for_else() {
        let spec = walk_spec();
        let output = graphite(["inspect".as_ref(), spec.as_os_str(), "--json".as_ref()]);
        assert!(output.status.success(), "{}", stderr(&output));
        let text = stdout(&output);
        assert!(
            text.starts_with(r#"{"ok":true,"graphs":[{"name":"walk","#),
            "{text}"
        );
        assert!(
            text.contains(r#"{"condition":null,"tag":"down","annotations":[]}"#),
            "{text}"
        );
        assert!(
            text.contains(r#""conditions":["keyup","left","right","up"]"#),
            "{text}"
        );
    }

    #[test]
    fn inspect_shows_annotations_on_the_graph_line() {
        let file = dsl(
            "cli-inspect-graph-line",
            "graph door @anim:idle\n  node closed @anim:open\n  node open end\n\n  edge closed -> open\n",
        );
        let text = graphite(["inspect".as_ref(), file.as_os_str()]);
        assert!(text.status.success(), "{}", stderr(&text));
        assert!(
            stdout(&text).starts_with("graph door @anim:idle\n"),
            "{}",
            stdout(&text)
        );

        let json = graphite(["inspect".as_ref(), file.as_os_str(), "--json".as_ref()]);
        assert!(json.status.success(), "{}", stderr(&json));
        assert!(
            stdout(&json)
                .contains(r#""name":"door","annotations":[{"key":"anim","value":"idle"}],"nodes""#),
            "{}",
            stdout(&json)
        );
    }
}

// Merged from `tests/cli_coverage.rs`.
mod cli_coverage {
    //! Covers the command-line paths the other CLI tests leave out: every command's help, argument
    //! errors, the shell and manual outputs, and the debugger's commands.
    #[allow(unused_imports)]
    use super::cli;

    use cli::{dsl, graphite, scratch, stderr, stdout, walk_spec};
    use std::ffi::OsStr;

    fn fixture(path: &str) -> std::path::PathBuf {
        cli::manifest().join(path)
    }

    fn run_ok<I, S>(args: I) -> String
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let output = graphite(args);
        assert!(output.status.success(), "stderr: {}", stderr(&output));
        stdout(&output)
    }

    fn run_err<I, S>(args: I, code: i32, message: &str)
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let output = graphite(args);
        assert_eq!(
            output.status.code(),
            Some(code),
            "stdout: {}",
            stdout(&output)
        );
        assert!(
            stderr(&output).contains(message),
            "stderr: {}",
            stderr(&output)
        );
    }

    #[test]
    fn every_command_has_help_text() {
        for command in [
            "init",
            "check",
            "compile",
            "run",
            "debug",
            "inspect",
            "doctor",
            "completions",
            "man",
        ] {
            let text = run_ok(["help", command]);
            assert!(text.contains("usage:"), "{command}: {text}");
        }
    }

    #[test]
    fn an_unknown_help_topic_names_it() {
        let output = graphite(["help", "bogus"]);
        let text = format!("{}{}", stdout(&output), stderr(&output));
        assert!(text.contains("no help for \"bogus\""), "{text}");
    }

    #[test]
    fn version_flag_prints_the_crate_version() {
        assert_eq!(
            run_ok(["--version"]).trim(),
            format!("graphite {}", env!("CARGO_PKG_VERSION"))
        );
    }

    #[test]
    fn completions_cover_each_shell_and_reject_others() {
        for shell in ["bash", "zsh", "fish"] {
            assert!(!run_ok(["completions", shell]).is_empty(), "{shell}");
        }
        run_err(["completions", "nope"], 2, "unknown shell \"nope\"");
        run_err(["completions"], 2, "completions needs a shell");
    }

    #[test]
    fn man_prints_the_manual_page() {
        assert!(run_ok(["man"]).starts_with(".TH GRAPHITE 1"));
    }

    #[test]
    fn init_writes_a_starter_and_refuses_to_overwrite() {
        let path = scratch("init/starter.graphite");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let _ = std::fs::remove_file(&path);
        let text = run_ok(["init".as_ref(), path.as_os_str()]);
        assert!(text.contains("wrote"), "{text}");
        assert!(path.exists());
        run_err(["init".as_ref(), path.as_os_str()], 1, "");
    }

    #[test]
    fn missing_file_arguments_are_usage_errors() {
        for command in ["init", "check", "compile", "inspect", "run", "debug"] {
            run_err([command], 2, "needs a file");
        }
    }

    #[test]
    fn a_flag_that_needs_a_value_says_so() {
        let spec = walk_spec();
        run_err(
            ["compile".as_ref(), spec.as_os_str(), "--out".as_ref()],
            2,
            "--out needs a value",
        );
        run_err(
            ["run".as_ref(), spec.as_os_str(), "--each".as_ref()],
            2,
            "--each needs a value",
        );
        run_err(
            [
                "run".as_ref(),
                spec.as_os_str(),
                "--until-passed".as_ref(),
                "x".as_ref(),
            ],
            2,
            "--until-passed needs a whole number, got x",
        );
    }

    #[test]
    fn a_missing_input_file_is_reported() {
        run_err(
            ["check", "/no/such/file.graphite"],
            1,
            "No such file or directory",
        );
        run_err(
            ["inspect", "/no/such/file.graphite"],
            1,
            "No such file or directory",
        );
    }

    #[test]
    fn check_and_inspect_accept_json_and_pure_flags() {
        let spec = walk_spec();
        assert!(
            run_ok(["check".as_ref(), spec.as_os_str(), "--json".as_ref()]).contains("\"ok\":true")
        );
        let inspected = run_ok(["inspect".as_ref(), spec.as_os_str(), "--json".as_ref()]);
        assert!(inspected.contains("\"name\":\"walk\""), "{inspected}");
        let text = run_ok(["inspect".as_ref(), spec.as_os_str()]);
        assert!(!text.is_empty());
    }

    #[test]
    fn check_takes_a_frontend_and_pure_mode() {
        let pure = fixture("tests/fixtures/pure/lamp_gate.purs.graphite");
        run_ok(["check".as_ref(), pure.as_os_str()]);
        let spec = walk_spec();
        run_ok([
            "check".as_ref(),
            spec.as_os_str(),
            "--frontend".as_ref(),
            "typescript".as_ref(),
        ]);
        run_err(
            [
                "check".as_ref(),
                spec.as_os_str(),
                "--frontend".as_ref(),
                "cobol".as_ref(),
            ],
            1,
            "cobol",
        );
    }

    #[test]
    fn compile_writes_modules_to_the_out_directory() {
        let spec = walk_spec();
        let out = scratch("compile/out");
        let listing = run_ok([
            "compile".as_ref(),
            spec.as_os_str(),
            "--out".as_ref(),
            out.as_os_str(),
        ]);
        assert!(listing.contains("graphite.ts"), "{listing}");
        assert!(out.join("graphite.ts").exists());
    }

    #[test]
    fn compile_writes_purescript_modules_for_a_pure_file() {
        let pure = fixture("tests/fixtures/pure/lamp_gate.purs.graphite");
        let out = scratch("compile/purs");
        run_ok([
            "compile".as_ref(),
            pure.as_os_str(),
            "--frontend".as_ref(),
            "purescript".as_ref(),
            "--out".as_ref(),
            out.as_os_str(),
        ]);
        assert!(std::fs::read_dir(&out).unwrap().count() > 0);
    }

    #[test]
    fn an_unknown_option_on_doctor_is_rejected() {
        run_err(
            ["doctor", "--bogus"],
            2,
            "doctor takes no arguments except --json",
        );
    }

    #[test]
    fn doctor_reports_each_front_end() {
        let text = run_ok(["doctor"]);
        assert!(text.contains("typescript"), "{text}");
        assert!(text.contains("purescript"), "{text}");
        let json = run_ok(["doctor", "--json"]);
        assert!(json.starts_with("{\"ok\""), "{json}");
    }

    #[test]
    fn run_takes_a_runner_and_checkpoints_its_state() {
        let fixtures = cli::manifest().join("tests/fixtures/runtime");
        let dsl = fixtures.join("workflow.graphite");
        let allow = fixtures.join("workflow.toml");
        let runner = fixtures.join("pass_runner.ts");
        let state = scratch("coverage-run").join("state.json");
        std::fs::create_dir_all(state.parent().unwrap()).unwrap();
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
        assert!(state.exists(), "state file was not written");
    }

    #[test]
    fn run_refuses_a_task_list_and_a_runner_together() {
        let fixtures = cli::manifest().join("tests/fixtures/runtime");
        let dsl = fixtures.join("workflow.graphite");
        let tasks = fixtures.join("workflow.toml");
        let runner = fixtures.join("pass_runner.ts");
        let output = graphite([
            "run".as_ref(),
            dsl.as_os_str(),
            "--tasks".as_ref(),
            tasks.as_os_str(),
            "--runner".as_ref(),
            runner.as_os_str(),
        ]);
        assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
    }

    #[test]
    fn debug_commands_report_their_state_and_usage() {
        let spec = walk_spec();
        let session = |commands: &[&str]| {
            let mut args: Vec<&OsStr> = vec!["debug".as_ref(), spec.as_os_str()];
            args.extend(commands.iter().map(OsStr::new));
            graphite(args)
        };
        let show = session(&["show"]);
        assert!(stdout(&show).contains("node windup"), "{}", stdout(&show));
        let step = session(&["step"]);
        assert!(
            stdout(&step).contains("no edges taken"),
            "{}",
            stdout(&step)
        );
        let set_usage = session(&["set"]);
        assert!(
            stdout(&set_usage).contains("usage: set <condition> on|off"),
            "{}",
            stdout(&set_usage)
        );
        let choose_usage = session(&["choose"]);
        assert!(
            stdout(&choose_usage).contains("usage: choose <choice>"),
            "{}",
            stdout(&choose_usage)
        );
        let unknown = session(&["bogus"]);
        assert!(
            stdout(&unknown).contains("unknown command \"bogus\""),
            "{}",
            stdout(&unknown)
        );
    }

    #[test]
    fn debug_simulate_reports_a_graph_that_has_no_such_name() {
        let fixtures = cli::manifest().join("examples/workflow");
        let dsl = fixtures.join("workflow.graphite");
        let allow = fixtures.join("workflow.toml");
        let output = graphite([
            "debug".as_ref(),
            dsl.as_os_str(),
            "--allow".as_ref(),
            allow.as_os_str(),
            "--graph".as_ref(),
            "nope".as_ref(),
            "--simulate".as_ref(),
            "lint=pass".as_ref(),
        ]);
        assert!(!output.status.success());
        assert!(
            stderr(&output).contains("no graph named \"nope\""),
            "{}",
            stderr(&output)
        );
        let _ = dsl_unused();
    }

    fn dsl_unused() -> std::path::PathBuf {
        dsl("unused-coverage.graphite", "")
    }

    #[test]
    fn commands_without_arguments_reject_extras() {
        run_err(
            ["--version", "extra"],
            2,
            "--version takes no arguments, got \"extra\"",
        );
        run_err(
            ["help", "check", "extra"],
            2,
            "help takes at most one command name",
        );
        run_err(["man", "extra"], 2, "man takes no arguments, got \"extra\"");
        run_err(
            ["doctor", "--json", "extra"],
            2,
            "doctor takes no arguments except --json",
        );
        assert!(stdout(&graphite(["man", "--help"])).contains("usage: graphite man"));
    }

    #[test]
    fn a_flag_that_needs_a_value_is_named_for_every_command() {
        let spec = walk_spec();
        let spec = spec.as_os_str();
        run_err(
            ["run".as_ref(), spec, "--input".as_ref()],
            2,
            "--input needs a value",
        );
        run_err(
            ["check".as_ref(), spec, "--allow".as_ref()],
            2,
            "--allow needs a value",
        );
        run_err(
            ["debug".as_ref(), spec, "--simulate".as_ref()],
            2,
            "--simulate needs a value",
        );
        run_err(
            ["debug".as_ref(), spec, "--graph".as_ref()],
            2,
            "--graph needs a value",
        );
        run_err(
            ["check".as_ref(), spec, "--graph".as_ref()],
            2,
            "--graph needs a value",
        );
        run_err(
            ["compile".as_ref(), spec, "--frontend".as_ref()],
            2,
            "--frontend needs a value",
        );
    }

    #[test]
    fn flags_are_refused_on_commands_that_do_not_take_them() {
        let spec = walk_spec();
        run_err(
            [
                "check".as_ref(),
                spec.as_os_str(),
                "--out".as_ref(),
                "x".as_ref(),
            ],
            2,
            "check does not take --out",
        );
        run_err(
            ["inspect".as_ref(), spec.as_os_str(), "--bogus".as_ref()],
            2,
            "unknown option --bogus",
        );
    }

    #[test]
    fn each_seen_and_until_passed_pair_up_as_documented() {
        let spec = walk_spec();
        let spec = spec.as_os_str();
        run_err(
            ["run".as_ref(), spec, "--each".as_ref(), spec],
            2,
            "--each and --key go together",
        );
        run_err(
            ["run".as_ref(), spec, "--seen".as_ref(), "s.json".as_ref()],
            2,
            "--seen and --until-passed apply to --each only",
        );
    }

    #[test]
    fn doctor_reports_blocked_commands_when_no_tools_are_installed() {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_graphite"))
            .arg("doctor")
            .env_clear()
            .env("PATH", "/nonexistent")
            .output()
            .expect("graphite binary must run");
        assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
        let text = stdout(&output);
        assert!(
            text.contains("blocked  needs a JavaScript runtime"),
            "{text}"
        );
        assert!(
            text.contains("summary: blocked: typescript debug, typescript run, rust debug"),
            "{text}"
        );
        assert!(
            stderr(&output).contains("some commands cannot run here"),
            "{}",
            stderr(&output)
        );

        let json = std::process::Command::new(env!("CARGO_BIN_EXE_graphite"))
            .args(["doctor", "--json"])
            .env_clear()
            .env("PATH", "/nonexistent")
            .output()
            .expect("graphite binary must run");
        assert_eq!(json.status.code(), Some(1));
        assert!(
            stdout(&json).starts_with("{\"ok\":false"),
            "{}",
            stdout(&json)
        );
    }

    /// `check` of the walk graph with `allow` written as the allowlist.
    fn check_with_allow(name: &str, allow: &str) -> std::process::Output {
        let spec = walk_spec();
        let list = scratch(&format!("allow/{name}.toml"));
        std::fs::create_dir_all(list.parent().unwrap()).unwrap();
        std::fs::write(&list, allow).unwrap();
        graphite([
            "check".as_ref(),
            spec.as_os_str(),
            "--allow".as_ref(),
            list.as_os_str(),
        ])
    }

    #[test]
    fn an_allowlist_with_an_unknown_section_is_refused() {
        let output = check_with_allow("unknown-section", "[bogus]\nx = 1\n");
        assert!(
            stderr(&output).contains("unknown key [bogus]"),
            "{}",
            stderr(&output)
        );
    }

    #[test]
    fn an_allowlist_with_malformed_toml_is_refused() {
        let output = check_with_allow("malformed", "[annotations\n");
        assert_eq!(output.status.code(), Some(1));
        assert!(
            stderr(&output).contains("allow/malformed.toml"),
            "{}",
            stderr(&output)
        );
    }

    #[test]
    fn node_flags_must_be_a_list_of_strings() {
        let output = check_with_allow("flags-type", "node_flags = \"loop\"\n");
        assert!(
            stderr(&output).contains("node_flags must be an array of strings"),
            "{}",
            stderr(&output)
        );
    }

    #[test]
    fn annotations_must_be_a_table_of_string_lists() {
        let not_a_table = check_with_allow("annotations-type", "annotations = [\"x\"]\n");
        assert!(
            stderr(&not_a_table).contains("[annotations] must be a table"),
            "{}",
            stderr(&not_a_table)
        );
        let not_a_list = check_with_allow("annotations-list", "[annotations]\nanim = \"walk\"\n");
        assert!(
            stderr(&not_a_list).contains("[annotations] anim must be an array of strings"),
            "{}",
            stderr(&not_a_list)
        );
    }

    #[test]
    fn an_annotation_value_outside_the_allowlist_names_its_place() {
        // The walk graph uses anim=windup, anim=walk, anim=winddown and anim=idle on its nodes.
        let output = check_with_allow("anim-missing", "[annotations]\nanim = [\"walk\"]\n");
        assert!(!output.status.success());
        let text = stderr(&output);
        assert!(text.contains("graph walk"), "{text}");
    }

    #[test]
    fn an_arm_annotation_outside_the_allowlist_names_the_choice() {
        // The `left` arm sets flip=true. A listed key only admits the values it names.
        let output = check_with_allow("arm-missing", "[annotations]\nflip = [\"false\"]\n");
        assert!(!output.status.success());
        assert!(
            stderr(&output).contains("choice facing"),
            "{}",
            stderr(&output)
        );
    }

    #[test]
    fn an_allowlist_that_permits_every_annotation_passes() {
        let output = check_with_allow(
            "everything",
            "[annotations]\nanim = [\"windup\", \"walk\", \"winddown\", \"idle\"]\ngroup = [\"locomotion\"]\nflip = [\"true\"]\n",
        );
        assert!(output.status.success(), "{}", stderr(&output));
    }
}
