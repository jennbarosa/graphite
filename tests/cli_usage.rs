//! Merged test file. Each section below was `tests/<name>.rs` before; see git history.

#[allow(dead_code)]
#[path = "support/cli.rs"]
mod cli;

// Merged from `tests/cli_args.rs`.
mod cli_args {
    //! Argument handling shared by every command.
    #[allow(unused_imports)]
    use super::cli;

    use cli::{graphite, stderr, stdout, walk_spec};

    #[test]
    fn an_unknown_option_is_rejected() {
        let spec = walk_spec();
        let output = graphite(["check".as_ref(), spec.as_os_str(), "--verbose".as_ref()]);
        assert!(!output.status.success());
        assert_eq!(output.status.code(), Some(2));
        assert!(stderr(&output).contains("unknown option --verbose"));
    }

    #[test]
    fn no_arguments_prints_usage() {
        let output = graphite(Vec::<&str>::new());
        assert!(!output.status.success());
        assert_eq!(output.status.code(), Some(2));
        assert!(stderr(&output).contains("usage:"));
    }

    #[test]
    fn an_unknown_command_is_rejected() {
        let spec = walk_spec();
        let output = graphite(["frobnicate".as_ref(), spec.as_os_str()]);
        assert!(!output.status.success());
        assert_eq!(output.status.code(), Some(2));
        assert!(stderr(&output).contains("unknown command \"frobnicate\""));
    }

    #[test]
    fn an_option_without_its_value_is_rejected() {
        let spec = walk_spec();
        let output = graphite(["check".as_ref(), spec.as_os_str(), "--allow".as_ref()]);
        assert!(!output.status.success());
        assert_eq!(output.status.code(), Some(2));
        assert!(stderr(&output).contains("--allow needs a value"));
    }

    #[test]
    fn a_command_without_its_file_is_rejected() {
        let output = graphite(["check"]);
        assert!(!output.status.success());
        assert_eq!(output.status.code(), Some(2));
        assert!(stderr(&output).contains("check needs a file"));
    }

    #[test]
    fn doctor_takes_no_arguments() {
        let output = graphite(["doctor", "extra"]);
        assert!(!output.status.success());
        assert!(stderr(&output).contains("doctor takes no arguments"));
    }

    #[test]
    fn version_prints_the_package_version_for_both_flags() {
        let expected = format!("graphite {}\n", env!("CARGO_PKG_VERSION"));
        for flag in ["--version", "-V"] {
            let output = graphite([flag]);
            assert!(output.status.success());
            assert_eq!(stdout(&output), expected);
        }
    }

    /// `--json` belongs to `check`, `inspect` and `doctor`; other commands refuse it as a usage error.
    #[test]
    fn json_is_refused_by_commands_that_do_not_print_it() {
        let spec = walk_spec();
        let output = graphite(["compile".as_ref(), spec.as_os_str(), "--json".as_ref()]);
        assert_eq!(output.status.code(), Some(2));
        assert!(stderr(&output).contains("compile does not take --json"));
    }
}

// Merged from `tests/cli_commands.rs`.
mod cli_commands {
    //! The `debug`, `inspect --json` failure, and `run --each` paths that the other CLI tests do not
    //! reach: a chosen graph, an allowlist, a front end and output directory, a failure as JSON, and
    //! a missing item graph.
    #[allow(unused_imports)]
    use super::cli;

    use cli::{dsl, graphite, manifest, scratch, stderr, stdout};

    fn door() -> std::path::PathBuf {
        manifest().join("examples/door.graphite")
    }

    #[test]
    fn debug_runs_the_named_graph() {
        let output = graphite([
            "debug".as_ref(),
            door().as_os_str(),
            "--graph".as_ref(),
            "door".as_ref(),
            "show".as_ref(),
        ]);
        assert!(output.status.success(), "{}", stderr(&output));
        assert!(
            stdout(&output).starts_with("node closed  status running  transitions 0  true: none")
        );
    }

    #[test]
    fn debug_names_the_graphs_a_file_has_when_the_name_is_wrong() {
        let output = graphite([
            "debug".as_ref(),
            door().as_os_str(),
            "--graph".as_ref(),
            "nope".as_ref(),
            "show".as_ref(),
        ]);
        assert!(!output.status.success());
        assert!(
            stderr(&output).contains("no graph named \"nope\"; the file has door"),
            "{}",
            stderr(&output)
        );
    }

    #[test]
    fn debug_applies_an_allowlist_to_the_annotations_it_reads() {
        let allow = dsl("debug_allow.toml", "[annotations]\nanim = [\"idle\"]\n");
        let output = graphite([
            "debug".as_ref(),
            door().as_os_str(),
            "--allow".as_ref(),
            allow.as_os_str(),
            "show".as_ref(),
        ]);
        assert!(!output.status.success());
        assert!(
            stderr(&output)
                .contains("graph door, node opening: @anim:open is not allowed; allowed: idle"),
            "{}",
            stderr(&output)
        );
    }

    #[test]
    fn debug_writes_a_rust_debugger_into_the_output_directory() {
        let out = scratch("debug_rust_out");
        let output = graphite([
            "debug".as_ref(),
            door().as_os_str(),
            "--frontend".as_ref(),
            "rust".as_ref(),
            "--out".as_ref(),
            out.as_os_str(),
            "show".as_ref(),
        ]);
        assert!(output.status.success(), "{}", stderr(&output));
        assert!(
            stdout(&output).starts_with("node closed"),
            "{}",
            stdout(&output)
        );
    }

    #[test]
    fn inspect_reports_a_failure_as_json_on_stdout() {
        let file = dsl(
            "inspect_json_failure.graphite",
            "graph g\n  node a\n  edge a -> \n",
        );
        let output = graphite(["inspect".as_ref(), file.as_os_str(), "--json".as_ref()]);
        assert!(!output.status.success());
        let text = stdout(&output);
        assert!(
            text.starts_with("{\"ok\":false,\"error\":\"error: expected a node name"),
            "{text}"
        );
    }

    #[test]
    fn run_each_names_an_item_graph_that_is_missing() {
        let items = dsl("run_each_missing_items.json", "{\"items\":[1]}");
        let output = graphite([
            "run".as_ref(),
            door().as_os_str(),
            "--input".as_ref(),
            items.as_os_str(),
            "--each".as_ref(),
            "items".as_ref(),
            "--key".as_ref(),
            "id".as_ref(),
        ]);
        assert!(!output.status.success());
        assert!(
            stderr(&output).contains("items: No such file or directory"),
            "{}",
            stderr(&output)
        );
    }

    #[test]
    fn version_prints_the_crate_version() {
        let output = graphite(["--version"]);
        assert!(output.status.success());
        assert_eq!(
            stdout(&output),
            format!("graphite {}\n", env!("CARGO_PKG_VERSION"))
        );
    }
}

// Merged from `tests/cli_flag_errors.rs`.
mod cli_flag_errors {
    //! The command line's argument rules: each refused flag combination names the flag, and each
    //! accepted one runs. Every case runs the real binary, so the message is what a user sees.
    #[allow(unused_imports)]
    use super::cli;

    use cli::{graphite, manifest, stderr, stdout};
    use std::path::PathBuf;

    fn door() -> PathBuf {
        manifest().join("examples/door.graphite")
    }

    /// Each refused combination exits 2 and names the problem, on stdout or stderr.
    #[test]
    fn refused_argument_combinations_name_the_flag() {
        let door = door();
        let door = door.to_str().unwrap();
        let cases: Vec<(Vec<&str>, i32, &str)> = vec![
            (vec!["help", "nonsense"], 2, "no help for \"nonsense\""),
            (
                vec!["doctor", "extra"],
                2,
                "doctor takes no arguments except --json, got \"extra\"",
            ),
            (vec!["init"], 2, "init needs a file"),
            (
                vec!["check", door, "--input", "x"],
                2,
                "check does not take --input",
            ),
            (
                vec!["inspect", door, "--tasks", "x"],
                2,
                "inspect does not take --tasks",
            ),
            (vec!["run", door, "--input"], 2, "--input needs a value"),
            (
                vec!["run", door, "--key", "k"],
                2,
                "--each and --key go together",
            ),
            (
                vec!["run", door, "--each", "e"],
                2,
                "--each and --key go together",
            ),
            (
                vec!["run", door, "--seen", "s", "--until-passed", "3"],
                2,
                "--seen and --until-passed apply to --each only",
            ),
            (
                vec!["debug", door, "--simulate"],
                2,
                "--simulate needs a value",
            ),
            (
                vec!["debug", door, "--simulate", "x"],
                1,
                "stub \"x\" must be <task>=<outcomes>",
            ),
        ];
        let mut failures = Vec::new();
        for (args, code, message) in cases {
            let output = graphite(&args);
            let text = format!("{}{}", stdout(&output), stderr(&output));
            if output.status.code() != Some(code) || !text.contains(message) {
                failures.push(format!(
                    "{args:?}: want exit {code} and `{message}`, got exit {:?}:\n{text}",
                    output.status.code()
                ));
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    #[test]
    fn help_prints_the_overview_or_a_topic() {
        for args in [vec!["help"], vec!["--help"], vec!["-h"]] {
            let output = graphite(&args);
            assert!(output.status.success(), "{args:?}");
            assert!(stdout(&output).starts_with("usage:"), "{args:?}");
        }
        let output = graphite(["help", "check"]);
        assert!(output.status.success());
        assert!(stdout(&output).starts_with("usage: graphite check"));
    }
}

// Merged from `tests/cli_flag_matrix.rs`.
mod cli_flag_matrix {
    //! Each command refuses the flags it does not take, and names the flag and the command. The
    //! tables list what the parser refuses; a flag a command does take is covered by its own tests.
    #[allow(unused_imports)]
    use super::cli;

    use cli::{graphite, manifest, stderr};

    /// (command, the flags it refuses), with the graph every case passes as its file.
    const REFUSED: &[(&str, &[&str])] = &[
        (
            "init",
            &[
                "--out",
                "--frontend",
                "--allow",
                "--json",
                "--state",
                "--runner",
                "--tasks",
                "--input",
                "--each",
                "--key",
                "--seen",
                "--simulate",
            ],
        ),
        (
            "check",
            &[
                "--out",
                "--state",
                "--runner",
                "--tasks",
                "--input",
                "--each",
                "--key",
                "--seen",
                "--simulate",
            ],
        ),
        (
            "compile",
            &[
                "--json",
                "--state",
                "--runner",
                "--tasks",
                "--input",
                "--each",
                "--key",
                "--seen",
                "--simulate",
            ],
        ),
        (
            "inspect",
            &[
                "--out",
                "--frontend",
                "--allow",
                "--state",
                "--runner",
                "--tasks",
                "--input",
                "--each",
                "--key",
                "--seen",
                "--simulate",
            ],
        ),
        ("run", &["--out", "--frontend", "--json", "--simulate"]),
        (
            "debug",
            &[
                "--json", "--state", "--runner", "--tasks", "--input", "--each", "--key", "--seen",
            ],
        ),
    ];

    #[test]
    fn each_command_names_each_flag_it_refuses() {
        let graph = manifest().join("examples/door.graphite");
        let mut failures = Vec::new();
        for (verb, flags) in REFUSED {
            for flag in *flags {
                let output = graphite([
                    verb.as_ref(),
                    graph.as_os_str(),
                    flag.as_ref(),
                    "x".as_ref(),
                ]);
                let message = format!("{verb} does not take {flag}");
                let text = stderr(&output);
                if output.status.code() != Some(2) || !text.contains(&message) {
                    failures.push(format!("{verb} {flag}: expected `{message}`, got {text}"));
                }
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    #[test]
    fn doctor_takes_no_flags_but_its_own() {
        let flag = "--out";
        let doctor = graphite(["doctor", flag, "x"]);
        assert_eq!(doctor.status.code(), Some(2));
        assert!(
            stderr(&doctor).contains("doctor takes no arguments except --json, got \"--out x\"")
        );
    }
}

// Merged from `tests/cli_environment.rs`.
mod cli_environment {
    //! What `doctor` and `run` report when a tool is missing or a file is wrong. Each case sets the
    //! environment the way a user would, so the message is the one they see.
    #[allow(unused_imports)]
    use super::cli;

    use cli::{dsl, graphite, scratch, stderr, stdout, walk_spec};
    use std::process::Command;

    fn graphite_with_env(args: &[&str], variables: &[(&str, &str)]) -> std::process::Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_graphite"));
        command.args(args);
        for (name, value) in variables {
            command.env(name, value);
        }
        command.output().expect("graphite binary must run")
    }

    #[test]
    fn doctor_blocks_each_runtime_it_cannot_find() {
        let output = graphite_with_env(&["doctor"], &[("PATH", "/nonexistent-graphite-path")]);
        // A blocked command makes `doctor` fail, and says so on stderr.
        assert!(!output.status.success());
        assert!(
            stderr(&output).contains("some commands cannot run here"),
            "{}",
            stderr(&output)
        );
        let text = stdout(&output);
        assert!(
            text.contains("debug    blocked  needs a JavaScript runtime"),
            "{text}"
        );
        assert!(
            text.contains("debug    blocked  needs rustc to build the debugger"),
            "{text}"
        );
        assert!(text.contains("summary: blocked: "), "{text}");
    }

    #[test]
    fn doctor_names_a_tsc_that_is_not_installed() {
        let output = graphite_with_env(&["doctor"], &[("GRAPHITE_TSC", "/nonexistent/tsc")]);
        assert!(stdout(&output).contains("missing  tsc for the TypeScript type-check test"));
    }

    #[test]
    fn doctor_json_lists_every_front_end_with_its_status() {
        let output = graphite(["doctor", "--json"]);
        assert!(output.status.success(), "{}", stderr(&output));
        let text = stdout(&output);
        assert!(text.starts_with("{\"ok\":true"), "{text}");
        for key in ["\"typescript\"", "\"rust\"", "\"purescript\""] {
            assert!(text.contains(key), "{key} missing from {text}");
        }
    }

    #[test]
    fn a_runner_that_cannot_start_is_named() {
        let output = graphite_with_env(
            &["run", walk_spec().to_str().unwrap()],
            &[("GRAPHITE_JS_RUNNER", "/nonexistent/graphite-runner")],
        );
        assert!(!output.status.success());
        assert!(
            stderr(&output)
                .contains("cannot start GRAPHITE_JS_RUNNER `/nonexistent/graphite-runner`"),
            "{}",
            stderr(&output)
        );
    }

    #[test]
    fn a_tasks_file_without_a_tasks_table_is_refused() {
        let tasks = dsl("tasks_without_table.toml", "[other]\nx = 1\n");
        let output = graphite([
            "run".as_ref(),
            walk_spec().as_os_str(),
            "--tasks".as_ref(),
            tasks.as_os_str(),
        ]);
        assert!(!output.status.success());
        assert!(
            stderr(&output).contains("expected a [tasks] table of commands"),
            "{}",
            stderr(&output)
        );
        assert!(
            stderr(&output).contains("runs the commands in"),
            "the warning is printed first"
        );
    }

    #[test]
    fn a_run_with_a_missing_allowlist_names_the_file() {
        let missing = scratch("no_such_allowlist.toml");
        let output = graphite([
            "run".as_ref(),
            walk_spec().as_os_str(),
            "--allow".as_ref(),
            missing.as_os_str(),
        ]);
        assert!(!output.status.success());
        assert!(
            stderr(&output).contains("no_such_allowlist.toml"),
            "{}",
            stderr(&output)
        );
    }

    #[test]
    fn doctor_json_marks_blocked_unavailable_and_missing_entries() {
        let output = graphite_with_env(
            &["doctor", "--json"],
            &[
                ("PATH", "/nonexistent-graphite-path"),
                ("GRAPHITE_TSC", "/nonexistent/tsc"),
            ],
        );
        assert!(!output.status.success());
        let text = stdout(&output);
        let text = text.trim_end();
        assert!(text.starts_with("{\"ok\":false,"), "{text}");
        assert!(text.contains("\"status\":\"blocked\""), "{text}");
        assert!(text.contains("\"status\":\"unavailable\""), "{text}");
        assert!(
            text.contains("\"test_tools\":[{\"status\":\"missing\""),
            "{text}"
        );
        assert!(
            text.ends_with("\"blocked\":[\"typescript debug\",\"typescript run\",\"rust debug\"]}"),
            "{text}"
        );
    }
}
