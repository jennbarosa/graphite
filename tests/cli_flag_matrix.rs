//! Each command refuses the flags it does not take, and names the flag and the command. The
//! tables list what the parser refuses; a flag a command does take is covered by its own tests.

#[allow(dead_code)]
#[path = "support/cli.rs"]
mod cli;

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
            "--pure",
            "--json",
            "--state",
            "--runner",
            "--tasks",
            "--input",
            "--each",
            "--key",
            "--seen",
            "--durable",
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
fn doctor_and_man_take_no_flags_but_their_own() {
    let flag = "--out";
    let doctor = graphite(["doctor", flag, "x"]);
    assert_eq!(doctor.status.code(), Some(2));
    assert!(stderr(&doctor).contains("doctor takes no arguments except --json, got \"--out x\""));
    let man = graphite(["man", flag]);
    assert_eq!(man.status.code(), Some(2));
    assert!(stderr(&man).contains("man takes no arguments, got \"--out\""));
}
