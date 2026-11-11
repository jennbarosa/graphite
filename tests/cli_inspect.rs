//! `graphite inspect`: prints a graph's structure as text.

#[allow(dead_code)]
#[path = "support/cli.rs"]
mod cli;

use cli::{graphite, stderr, stdout, walk_spec};

#[test]
fn lists_nodes_conditions_choices_and_edges() {
    let spec = walk_spec();
    let output = graphite(["inspect".as_ref(), spec.as_os_str()]);
    assert!(output.status.success(), "{}", stderr(&output));
    let text = stdout(&output);
    for expected in [
        "graph walk",
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
fn refuses_target_even_when_it_is_the_default() {
    let spec = walk_spec();
    let output = graphite([
        "inspect".as_ref(),
        spec.as_os_str(),
        "--target".as_ref(),
        "typescript".as_ref(),
    ]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("inspect does not take --target"));
}
