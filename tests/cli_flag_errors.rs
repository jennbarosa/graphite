//! The command line's argument rules: each refused flag combination names the flag, and each
//! accepted one runs. Every case runs the real binary, so the message is what a user sees.

#[allow(dead_code)]
#[path = "support/cli.rs"]
mod cli;

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
            vec!["completions"],
            2,
            "completions needs a shell: bash, zsh or fish",
        ),
        (
            vec!["completions", "tcsh"],
            2,
            "unknown shell \"tcsh\"; expected bash, zsh or fish",
        ),
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
