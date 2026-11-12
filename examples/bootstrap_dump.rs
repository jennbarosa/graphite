//! Bootstrap tool: compiles one `.graphite` file and writes a Rust module that rebuilds the
//! compiled graph without a parser. `build.rs` runs it on the bundled rule files.
//!
//! Usage: `bootstrap_dump <input.graphite> <output.rs>`

use std::process::ExitCode;

use graphite::graph::{CompiledGraph, Span};
use graphite::interner::StringId;
use graphite::parser::Schema;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let (Some(input), Some(output), None) = (args.next(), args.next(), args.next()) else {
        eprintln!("usage: bootstrap_dump <input.graphite> <output.rs>");
        return ExitCode::from(2);
    };
    let source = match std::fs::read_to_string(&input) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("{input}: {error}");
            return ExitCode::FAILURE;
        }
    };
    let graphs = match graphite::pipeline::compile(&source, &Schema::default()) {
        Ok(graphs) => graphs,
        Err(error) => {
            eprintln!("{input}: {}", error.diagnostic(&source));
            return ExitCode::FAILURE;
        }
    };
    let [graph] = graphs.as_slice() else {
        eprintln!("{input}: expected exactly one graph");
        return ExitCode::FAILURE;
    };
    if let Err(error) = std::fs::write(&output, emit(graph)) {
        eprintln!("{output}: {error}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

/// Rust source for `fn graph()`, which rebuilds `graph` field by field.
fn emit(graph: &CompiledGraph) -> String {
    let strings: Vec<&str> = graph.strings.strings().collect();
    let bytes: usize = strings.iter().map(|text| text.len()).sum();
    let quoted: Vec<String> = strings.iter().map(|text| format!("{text:?}")).collect();
    let list = |items: Vec<String>| items.join(", ");

    let nodes = list(
        graph
            .nodes
            .iter()
            .map(|node| {
                format!(
                    "Node {{ name: {}, looping: {}, end: {}, flags: {}, annotations: {}, outgoing: {}, sequence: {:?}, offset: {} }}",
                    id(node.name),
                    node.looping,
                    node.end,
                    span(&node.flags),
                    span(&node.annotations),
                    span(&node.outgoing),
                    node.sequence,
                    node.offset,
                )
            })
            .collect(),
    );
    let edges = list(
        graph
            .edges
            .iter()
            .map(|edge| {
                format!(
                    "Edge {{ source: {}, target: {}, condition: {:?}, annotations: {} }}",
                    edge.source,
                    edge.target,
                    edge.condition,
                    span(&edge.annotations)
                )
            })
            .collect(),
    );
    let choices = list(
        graph
            .choices
            .iter()
            .map(|choice| {
                format!(
                    "Choice {{ name: {}, arms: {} }}",
                    id(choice.name),
                    span(&choice.arms)
                )
            })
            .collect(),
    );
    let arms = list(
        graph
            .arms
            .iter()
            .map(|arm| {
                format!(
                    "Arm {{ condition: {:?}, tag: {}, annotations: {} }}",
                    arm.condition,
                    id(arm.tag),
                    span(&arm.annotations)
                )
            })
            .collect(),
    );
    let annotations = list(
        graph
            .annotations
            .iter()
            .map(|annotation| {
                let value = match annotation.value {
                    Some(value) => format!("Some({})", id(value)),
                    None => "None".to_string(),
                };
                format!(
                    "Annotation {{ key: {}, value: {value} }}",
                    id(annotation.key)
                )
            })
            .collect(),
    );
    let conditions = list(graph.conditions.iter().map(|&name| id(name)).collect());
    let flags = list(graph.flags.iter().map(|&name| id(name)).collect());
    let outgoing = list(graph.outgoing.iter().map(u32::to_string).collect());

    // One line per field, so a change to the compiled form shows up as a small diff.
    let lines = [
        "pub(crate) fn graph() -> CompiledGraph {".to_string(),
        format!("    const STRINGS: &[&str] = &[{}];", list(quoted)),
        "    CompiledGraph {".to_string(),
        format!("        name: {},", id(graph.name)),
        format!("        offset: {},", graph.offset),
        format!("        strings: interner(STRINGS, {bytes}),"),
        format!("        nodes: vec![{nodes}],"),
        format!("        edges: vec![{edges}],"),
        format!("        outgoing: vec![{outgoing}],"),
        format!("        conditions: vec![{conditions}],"),
        format!("        choices: vec![{choices}],"),
        format!("        arms: vec![{arms}],"),
        format!("        annotations: vec![{annotations}],"),
        format!("        flags: vec![{flags}],"),
        "    }".to_string(),
        "}".to_string(),
    ];
    lines.join("\n") + "\n"
}

/// The generated code's name for a string: its zero-based index in the string table.
fn id(name: StringId) -> String {
    format!("id({})", name.index())
}

fn span(span: &Span) -> String {
    format!("{}..{}", span.start, span.end)
}
