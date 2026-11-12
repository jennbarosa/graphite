//! `graphite completions` and `graphite man`: the scripts and the manual page the binary prints.

#[allow(dead_code)]
#[path = "support/cli.rs"]
mod cli;

use cli::{graphite, stderr, stdout};

const COMMANDS: &[&str] = &[
    "init",
    "check",
    "compile",
    "inspect",
    "debug",
    "run",
    "doctor",
    "help",
    "completions",
    "man",
];

#[test]
fn completion_scripts_name_every_command_for_each_shell() {
    for shell in ["bash", "zsh", "fish"] {
        let output = graphite(["completions", shell]);
        assert!(output.status.success(), "{shell}: {}", stderr(&output));
        let script = stdout(&output);
        for command in COMMANDS {
            assert!(
                script.contains(command),
                "{shell} script does not mention {command}"
            );
        }
        assert!(
            script.contains("json"),
            "{shell} script does not complete --json"
        );
    }
}

#[test]
fn completions_refuse_an_unknown_or_missing_shell() {
    let unknown = graphite(["completions", "tcsh"]);
    assert_eq!(unknown.status.code(), Some(2));
    assert!(stderr(&unknown).contains("unknown shell \"tcsh\""));

    let missing = graphite(["completions"]);
    assert_eq!(missing.status.code(), Some(2));
    assert!(stderr(&missing).contains("completions needs a shell"));
}

#[test]
fn the_manual_page_covers_each_command_and_the_exit_status() {
    let output = graphite(["man"]);
    assert!(output.status.success(), "{}", stderr(&output));
    let page = stdout(&output);
    assert!(page.starts_with(".TH GRAPHITE 1"), "{page}");
    assert!(page.contains(".SH EXIT STATUS"), "{page}");
    assert!(page.contains(".SS graphite doctor"), "{page}");
    assert!(page.contains(r"\-\-json"), "{page}");
}
