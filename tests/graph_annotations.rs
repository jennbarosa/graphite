//! Annotations on the `graph` line, and `default` annotations copied onto nodes.

use graphite::graph::{CompiledGraph, index};
use graphite::parser::Schema;
use graphite::pipeline::compile;

fn graph(source: &str) -> CompiledGraph {
    compile(source, &Schema::default()).unwrap().remove(0)
}

fn pairs(graph: &CompiledGraph, span: &std::ops::Range<u32>) -> Vec<String> {
    graph.annotations[index(span)]
        .iter()
        .map(|entry| match entry.value {
            Some(value) => format!("{}:{}", graph.text(entry.key), graph.text(value)),
            None => graph.text(entry.key).to_owned(),
        })
        .collect()
}

#[test]
fn the_graph_line_carries_annotations() {
    let graph = graph("graph walk @choice:facing @idle\n  node a end\n");
    assert_eq!(
        pairs(&graph, &graph.graph_annotations),
        ["choice:facing", "idle"]
    );
    assert!(pairs(&graph, &graph.nodes[0].annotations).is_empty());
}

#[test]
fn a_default_reaches_each_node_unless_the_node_sets_the_key() {
    let graph = graph(
        "graph g\n  default @fps:8 @layer:1\n  node a @x:y\n  node b end @fps:12\n  edge a -> b\n",
    );
    assert_eq!(
        pairs(&graph, &graph.nodes[0].annotations),
        ["x:y", "fps:8", "layer:1"]
    );
    assert_eq!(
        pairs(&graph, &graph.nodes[1].annotations),
        ["fps:12", "layer:1"]
    );
}

#[test]
fn many_nodes_and_defaults_fit_the_inferred_capacity() {
    let mut source = String::from("graph g\n  default @a:1 @b:2 @c:3 @d:4\n");
    for node in 0..50 {
        let end = if node == 49 { " end" } else { "" };
        source.push_str(&format!("  node n{node}{end}\n"));
    }
    source.push_str(&format!(
        "  edge {}\n",
        (0..50)
            .map(|n| format!("n{n}"))
            .collect::<Vec<_>>()
            .join(" -> ")
    ));
    assert_eq!(graph(&source).nodes.len(), 50);
}

#[test]
fn a_default_must_come_before_the_first_node_and_name_an_annotation() {
    for (source, message) in [
        (
            "graph g\n  node a end\n  default @fps:8\n",
            "`default` must come before the first node",
        ),
        (
            "graph g\n  default\n  node a end\n",
            "expected an annotation after `default`",
        ),
    ] {
        let error = compile(source, &Schema::default()).err().unwrap();
        assert_eq!(error.message, message);
    }
}
