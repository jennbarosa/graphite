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
