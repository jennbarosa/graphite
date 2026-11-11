//! `graphite inspect`: prints each graph's nodes, conditions, choices and edges as text.

use std::path::Path;

use graphite::graph::{Annotation, CompiledGraph, Span, index};

use crate::load;

pub fn run(input: &Path) -> Result<(), String> {
    let (_, graphs) = load::graphs(input, None)?;
    print!("{}", describe(&graphs));
    Ok(())
}

pub fn describe(graphs: &[CompiledGraph]) -> String {
    let mut lines = Vec::new();
    for graph in graphs {
        lines.push(format!("graph {}", graph.text(graph.name)));
        describe_nodes(graph, &mut lines);
        describe_conditions(graph, &mut lines);
        describe_choices(graph, &mut lines);
        describe_edges(graph, &mut lines);
    }
    lines.join("\n") + "\n"
}

fn describe_nodes(graph: &CompiledGraph, lines: &mut Vec<String>) {
    lines.push("  nodes".to_string());
    for node in &graph.nodes {
        let mut line = format!("    {}", graph.text(node.name));
        if node.looping {
            line += " loop";
        }
        if node.end {
            line += " end";
        }
        for &flag in &graph.flags[index(&node.flags)] {
            line += &format!(" {}", graph.text(flag));
        }
        line += &annotations(graph, &node.annotations);
        lines.push(line);
    }
}

fn describe_conditions(graph: &CompiledGraph, lines: &mut Vec<String>) {
    let names: Vec<&str> = graph.conditions.iter().map(|&id| graph.text(id)).collect();
    let names = if names.is_empty() {
        "none".to_string()
    } else {
        names.join(", ")
    };
    lines.push(format!("  conditions: {names}"));
}

fn describe_choices(graph: &CompiledGraph, lines: &mut Vec<String>) {
    lines.push("  choices".to_string());
    for choice in &graph.choices {
        lines.push(format!("    {}", graph.text(choice.name)));
        for arm in &graph.arms[index(&choice.arms)] {
            let label = match arm.condition {
                Some(condition) => format!("when {}", condition_name(graph, condition)),
                None => "else".to_string(),
            };
            let tag = graph.text(arm.tag);
            lines.push(format!(
                "      {label} -> {tag}{}",
                annotations(graph, &arm.annotations)
            ));
        }
    }
}

fn describe_edges(graph: &CompiledGraph, lines: &mut Vec<String>) {
    lines.push("  edges".to_string());
    for edge in &graph.edges {
        let source = graph.text(graph.nodes[edge.source as usize].name);
        let target = graph.text(graph.nodes[edge.target as usize].name);
        let mut line = format!("    {source} -> {target}");
        if let Some(condition) = edge.condition {
            line += &format!(" when {}", condition_name(graph, condition));
        }
        line += &annotations(graph, &edge.annotations);
        lines.push(line);
    }
}

fn condition_name(graph: &CompiledGraph, condition: u32) -> &str {
    graph.text(graph.conditions[condition as usize])
}

/// Renders a span of annotations as ` @key` or ` @key:value`, each with a leading space.
fn annotations(graph: &CompiledGraph, span: &Span) -> String {
    let mut rendered = String::new();
    for Annotation { key, value } in &graph.annotations[index(span)] {
        rendered += &format!(" @{}", graph.text(*key));
        if let Some(value) = value {
            rendered += &format!(":{}", graph.text(*value));
        }
    }
    rendered
}
