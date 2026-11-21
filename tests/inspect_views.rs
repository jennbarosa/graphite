//! `graphite inspect` in its two views, text and `--json`, for a machine graph and for pure
//! graphs. Each assertion names a fact the view must show.

#[allow(dead_code)]
#[path = "support/cli.rs"]
mod cli;

use cli::{dsl, graphite, manifest, stderr, stdout};

fn inspect(args: &[&str]) -> String {
    let output = graphite(args);
    assert!(output.status.success(), "{}", stderr(&output));
    stdout(&output)
}

fn example(path: &str) -> String {
    manifest().join(path).to_str().unwrap().to_owned()
}

#[test]
fn text_view_of_a_machine_graph_lists_nodes_conditions_and_edges() {
    let text = inspect(&["inspect", &example("examples/door.graphite")]);
    assert!(text.starts_with("graph door\n  nodes\n"), "{text}");
    assert!(text.contains("    closed @anim:idle\n"), "{text}");
    assert!(text.contains("  conditions: request, timeout\n"), "{text}");
    assert!(
        text.contains("    closed -> opening when request\n"),
        "{text}"
    );
    assert!(
        text.contains("    closing -> closed @sfx:latch\n"),
        "{text}"
    );
    assert!(!text.contains("kind pure"), "a machine graph shows no kind");
}

#[test]
fn json_view_of_a_machine_graph_keeps_its_fields_and_omits_pure_ones() {
    let json = inspect(&["inspect", &example("examples/door.graphite"), "--json"]);
    assert!(
        json.starts_with(r#"{"ok":true,"graphs":[{"name":"door","nodes":["#),
        "{json}"
    );
    assert!(
        json.contains(r#"{"name":"closed","loop":false,"end":false,"flags":[]"#),
        "{json}"
    );
    assert!(
        json.contains(r#""conditions":["request","timeout"]"#),
        "{json}"
    );
    assert!(json.contains(r#""condition":"request""#), "{json}");
    assert!(
        !json.contains(r#""kind""#),
        "a machine graph has no kind field: {json}"
    );
    assert!(!json.contains(r#""enums""#), "{json}");
}

#[test]
fn text_view_of_a_pure_graph_lists_enums_inputs_and_choices() {
    let text = inspect(&["inspect", &example("examples/combat.pure.graphite")]);
    assert!(
        text.starts_with("graph combat\n  kind pure\n  enums\n"),
        "{text}"
    );
    assert!(text.contains("    Tactic = flee | hit | hold\n"), "{text}");
    assert!(text.contains("    dir : Dir\n"), "{text}");
    assert!(
        text.contains("    tactic\n      when lowHealth -> flee @anim:run @log:on\n"),
        "{text}"
    );
    assert!(text.contains("      else -> hold @log:off\n"), "{text}");
    assert!(
        text.contains("      when dir:east -> east @log:off\n"),
        "{text}"
    );
}

#[test]
fn json_view_of_a_pure_graph_carries_kind_enums_and_inputs() {
    let json = inspect(&[
        "inspect",
        &example("tests/fixtures/pure/notes.pure.graphite"),
        "--json",
    ]);
    assert!(json.contains(r#""kind":"pure""#), "{json}");
    assert!(json.contains(r#"{"name":"Level""#), "{json}");
    assert!(
        json.contains(r#""inputs":[{"name":"urgent","type":"bool""#),
        "{json}"
    );
    assert!(
        json.contains(r#""choices":[{"name":"alert","enum":"Level""#),
        "{json}"
    );
}

#[test]
fn the_pure_flag_reads_a_file_as_pure_and_refuses_nodes() {
    let file = dsl(
        "inspect_pure_flag.graphite",
        "graph door\n  node a\n  node b end\n  edge a -> b\n",
    );
    let output = graphite(["inspect".as_ref(), file.as_os_str(), "--pure".as_ref()]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("`node` is not allowed in a pure file (pure by `--pure`)"));
}
