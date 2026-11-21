//! Valid graph syntax, checked through `graphite inspect --json`: each construct's facts must
//! come out as written, and each misuse must name its problem.

#[allow(dead_code)]
#[path = "support/cli.rs"]
mod cli;

use cli::{dsl, graphite, stderr, stdout};

fn inspect(name: &str, source: &str) -> (bool, String) {
    let file = dsl(name, source);
    let output = graphite(["inspect".as_ref(), file.as_os_str(), "--json".as_ref()]);
    let text = stdout(&output);
    let text = if text.is_empty() {
        stderr(&output)
    } else {
        text
    };
    (output.status.success(), text)
}

#[test]
fn defaults_and_annotations_reach_each_node_and_edge() {
    let source = "graph g @owner:ai\n  default @log:off\n  node a\n  node b end @anim:x\n  edge a -> b when x @k:v\n  edge a -> b\n";
    let (ok, json) = inspect("dsl_defaults.graphite", source);
    assert!(ok, "{json}");
    // A default reaches every node, and an own annotation stays alongside it.
    assert!(json.contains(r#"{"key":"log","value":"off"}"#), "{json}");
    assert!(
        json.contains(r#"{"key":"anim","value":"x"},{"key":"log","value":"off"}"#),
        "{json}"
    );
    // An edge keeps its own annotations and its condition.
    assert!(
        json.contains(r#""condition":"x","annotations":[{"key":"k","value":"v"}]"#),
        "{json}"
    );
    assert!(
        json.contains(r#""condition":null,"annotations":[]"#),
        "{json}"
    );
}

#[test]
fn a_loop_flag_and_an_else_arm_on_an_edge_are_accepted() {
    let (ok, json) = inspect(
        "dsl_loop.graphite",
        "graph g\n  node a loop\n  node b end\n  edge a -> b when x else b\n",
    );
    assert!(ok, "{json}");
    assert!(json.contains(r#""name":"a","loop":true"#), "{json}");
}

#[test]
fn a_flag_on_an_end_node_is_accepted_after_the_name() {
    let (ok, json) = inspect(
        "dsl_end_flag.graphite",
        "graph g\n  node a\n  node b end loop\n  edge a -> b\n",
    );
    assert!(ok, "{json}");
    assert!(
        json.contains(r#""name":"b","loop":true,"end":true"#),
        "{json}"
    );
}

#[test]
fn choose_arms_and_else_arms_are_listed_in_order() {
    let source = "graph g\n  node a\n  node b end\n  choose ch\n    when c -> b @x:y\n    else -> b\n  edge a -> b\n";
    let (ok, json) = inspect("dsl_choose.graphite", source);
    assert!(ok, "{json}");
    assert!(
        json.contains(r#""choices":[{"name":"ch","arms":["#),
        "{json}"
    );
    assert!(json.contains(r#""condition":"c","tag":"b""#), "{json}");
    assert!(json.contains(r#""condition":null,"tag":"b""#), "{json}");
}

/// (case name, source, the message the check must print)
const MISUSES: &[(&str, &str, &str)] = &[
    (
        "edge_else_without_a_target",
        "graph g\n  node a\n  node b end\n  edge a -> b when x else\n",
        "expected a node name after `->` or `edge`",
    ),
    (
        "edge_after_an_arrow_chain_with_a_condition",
        "graph g\n  node a\n  node b end\n  edge a -> b when x -> c\n",
        "expected the end of the line",
    ),
    (
        "end_node_with_an_outgoing_chain",
        "graph g\n  node a\n  node b end\n  node c end\n  edge a -> b -> c\n",
        "an `end` node cannot have outgoing edges",
    ),
    (
        "end_node_with_an_outgoing_edge",
        "graph g\n  node a end\n  node a2 end\n  edge a -> a2\n",
        "an `end` node cannot have outgoing edges",
    ),
    (
        "choose_annotations_outside_a_pure_file",
        "graph g\n  node a\n  node b end\n  choose ch @k:v\n    when c -> b\n    else -> b\n",
        "expected the end of the line",
    ),
    (
        "node_unreachable_from_the_entry",
        "graph g\n  node a\n  node b end\n  node c end\n  edge a -> b\n",
        "node is unreachable from the entry node",
    ),
];

#[test]
fn each_misuse_names_its_problem() {
    let mut failures = Vec::new();
    for (name, source, message) in MISUSES {
        let (ok, json) = inspect(&format!("dsl_misuse_{name}.graphite"), source);
        if ok || !json.contains(message) {
            failures.push(format!("{name}: expected `{message}`, got {json}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
