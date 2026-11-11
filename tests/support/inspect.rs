//! Test-only decoded view of compiled storage, independent of the DSL parser.

use std::fmt::Write;
use std::ops::Range;

use graphite::graph::{CompiledGraph, index};
use graphite::interner::StringId;

pub fn graphs(graphs: &[CompiledGraph]) -> String {
    let mut output = String::new();
    for graph in graphs {
        write_graph(&mut output, graph);
    }
    output
}

fn string(graph: &CompiledGraph, id: StringId) -> &str {
    graph.strings.resolve(id).expect("valid interned string")
}

fn condition(graph: &CompiledGraph, id: Option<u32>) -> Option<(u32, &str)> {
    id.map(|id| (id, string(graph, graph.conditions[id as usize])))
}

fn annotations(graph: &CompiledGraph, range: Range<usize>) -> Vec<(&str, Option<&str>)> {
    graph.annotations[range]
        .iter()
        .map(|annotation| {
            (
                string(graph, annotation.key),
                annotation.value.map(|id| string(graph, id)),
            )
        })
        .collect()
}

fn write_graph(output: &mut String, graph: &CompiledGraph) {
    writeln!(output, "graph {} entry=0", string(graph, graph.name)).unwrap();
    writeln!(
        output,
        "storage nodes={} edges={} choices={} arms={} annotations={} flags={}",
        graph.nodes.len(),
        graph.edges.len(),
        graph.choices.len(),
        graph.arms.len(),
        graph.annotations.len(),
        graph.flags.len()
    )
    .unwrap();
    let conditions: Vec<_> = graph
        .conditions
        .iter()
        .map(|&id| string(graph, id))
        .collect();
    writeln!(output, "conditions {conditions:?}").unwrap();

    for (id, node) in graph.nodes.iter().enumerate() {
        let flags: Vec<_> = graph.flags[index(&node.flags)]
            .iter()
            .map(|&id| string(graph, id))
            .collect();
        writeln!(output,
            "node[{id}] {} loop={} end={} flags={:?} {flags:?} annotations={:?} {:?} outgoing={:?} {:?} sequence={:?}",
            string(graph, node.name), node.looping, node.end, node.flags, node.annotations,
            annotations(graph, index(&node.annotations)), node.outgoing,
            &graph.outgoing[index(&node.outgoing)], node.sequence).unwrap();
    }

    for (id, edge) in graph.edges.iter().enumerate() {
        writeln!(
            output,
            "edge[{id}] {}[{}] -> {}[{}] condition={:?} annotations={:?} {:?}",
            string(graph, graph.nodes[edge.source as usize].name),
            edge.source,
            string(graph, graph.nodes[edge.target as usize].name),
            edge.target,
            condition(graph, edge.condition),
            edge.annotations,
            annotations(graph, index(&edge.annotations))
        )
        .unwrap();
    }

    for (id, choice) in graph.choices.iter().enumerate() {
        writeln!(
            output,
            "choice[{id}] {} arms={:?}",
            string(graph, choice.name),
            choice.arms
        )
        .unwrap();
        for id in index(&choice.arms) {
            let arm = &graph.arms[id];
            writeln!(
                output,
                "arm[{id}] condition={:?} tag={} annotations={:?} {:?}",
                condition(graph, arm.condition),
                string(graph, arm.tag),
                arm.annotations,
                annotations(graph, index(&arm.annotations))
            )
            .unwrap();
        }
    }
}
