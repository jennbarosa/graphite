//! `graphite inspect`: prints each graph's nodes, conditions, choices and edges as text.

use std::path::Path;

use graphite::graph::{Annotation, CompiledGraph, Span, Ty, index};
use graphite::kind::Kind;

use crate::resolve::Output;
use crate::{Failure, json, load};

pub fn run(input: &Path, json: bool) -> Result<(), Failure> {
    let loaded = load::read(input, None, None, Output::Unused)?;
    let graphs = loaded.graphs;
    if json {
        println!("{}", describe_json(&graphs));
    } else {
        print!("{}", describe(&graphs));
    }
    Ok(())
}

/// The same facts as [`describe`], as one JSON object: `{"ok":true,"graphs":[...]}`. Names are
/// strings, `null` stands for an `else` arm's condition or a bare annotation's value, and each
/// list keeps the order the text form prints.
pub fn describe_json(graphs: &[CompiledGraph]) -> String {
    let graphs: Vec<String> = graphs.iter().map(graph_json).collect();
    json::object(&[("ok", "true".to_string()), ("graphs", json::array(&graphs))])
}

fn graph_json(graph: &CompiledGraph) -> String {
    let pure = graph.kind == Kind::Pure;
    let mut fields = vec![
        ("name", json::quote(graph.text(graph.name))),
        (
            "annotations",
            annotations_json(graph, &graph.graph_annotations),
        ),
    ];
    // The pure fields appear only on pure graphs, so a machine graph's JSON is unchanged.
    if pure {
        fields.push(("kind", json::quote(graph.kind.label())));
    }
    fields.push(("nodes", json::array(&nodes_json(graph))));
    fields.push(("conditions", json::array(&conditions_json(graph))));
    if pure {
        fields.push(("enums", json::array(&enums_json(graph))));
        fields.push(("inputs", json::array(&inputs_json(graph))));
    }
    fields.push(("choices", json::array(&choices_json(graph, pure))));
    fields.push(("edges", json::array(&edges_json(graph))));
    json::object(&fields)
}

fn nodes_json(graph: &CompiledGraph) -> Vec<String> {
    graph
        .nodes
        .iter()
        .map(|node| {
            let flags: Vec<String> = graph.flags[index(&node.flags)]
                .iter()
                .map(|&flag| json::quote(graph.text(flag)))
                .collect();
            json::object(&[
                ("name", json::quote(graph.text(node.name))),
                ("loop", node.looping.to_string()),
                ("end", node.end.to_string()),
                ("flags", json::array(&flags)),
                ("annotations", annotations_json(graph, &node.annotations)),
            ])
        })
        .collect()
}

fn conditions_json(graph: &CompiledGraph) -> Vec<String> {
    graph
        .conditions
        .iter()
        .map(|&id| json::quote(graph.text(id)))
        .collect()
}

fn enums_json(graph: &CompiledGraph) -> Vec<String> {
    graph
        .enums
        .iter()
        .map(|item| {
            let members: Vec<String> = graph.members[index(&item.members)]
                .iter()
                .map(|member| {
                    json::object(&[
                        ("name", json::quote(graph.text(member.name))),
                        ("annotations", annotations_json(graph, &member.annotations)),
                    ])
                })
                .collect();
            json::object(&[
                ("name", json::quote(graph.text(item.name))),
                ("annotations", annotations_json(graph, &item.annotations)),
                ("members", json::array(&members)),
            ])
        })
        .collect()
}

/// The type of an input as written in the source: `bool`, or the name of its enum.
fn ty_name(graph: &CompiledGraph, ty: Ty) -> &str {
    match ty {
        Ty::Bool => "bool",
        Ty::Enum(index) => graph.text(graph.enums[index as usize].name),
    }
}

fn inputs_json(graph: &CompiledGraph) -> Vec<String> {
    graph
        .inputs
        .iter()
        .map(|input| {
            json::object(&[
                ("name", json::quote(graph.text(input.name))),
                ("type", json::quote(ty_name(graph, input.ty))),
                ("annotations", annotations_json(graph, &input.annotations)),
            ])
        })
        .collect()
}

/// A choice carries its enum and annotations only on a pure graph.
fn choices_json(graph: &CompiledGraph, pure: bool) -> Vec<String> {
    graph
        .choices
        .iter()
        .map(|choice| {
            let arms: Vec<String> = graph.arms[index(&choice.arms)]
                .iter()
                .map(|arm| {
                    let condition = arm.condition.map(|id| condition_name(graph, id));
                    json::object(&[
                        ("condition", json::optional(condition)),
                        ("tag", json::quote(graph.text(arm.tag))),
                        ("annotations", annotations_json(graph, &arm.annotations)),
                    ])
                })
                .collect();
            let mut fields = vec![("name", json::quote(graph.text(choice.name)))];
            if pure {
                fields.push((
                    "enum",
                    json::optional(
                        choice
                            .ty
                            .map(|ty| graph.text(graph.enums[ty as usize].name)),
                    ),
                ));
                fields.push(("annotations", annotations_json(graph, &choice.annotations)));
            }
            fields.push(("arms", json::array(&arms)));
            json::object(&fields)
        })
        .collect()
}

fn edges_json(graph: &CompiledGraph) -> Vec<String> {
    graph
        .edges
        .iter()
        .map(|edge| {
            let condition = edge.condition.map(|id| condition_name(graph, id));
            json::object(&[
                (
                    "source",
                    json::quote(graph.text(graph.nodes[edge.source as usize].name)),
                ),
                (
                    "target",
                    json::quote(graph.text(graph.nodes[edge.target as usize].name)),
                ),
                ("condition", json::optional(condition)),
                ("annotations", annotations_json(graph, &edge.annotations)),
            ])
        })
        .collect()
}

/// Annotations as `[{"key":..,"value":..}]`, with `value` null when the annotation has none.
fn annotations_json(graph: &CompiledGraph, span: &Span) -> String {
    let items: Vec<String> = graph.annotations[index(span)]
        .iter()
        .map(|Annotation { key, value }| {
            json::object(&[
                ("key", json::quote(graph.text(*key))),
                (
                    "value",
                    json::optional(value.map(|value| graph.text(value))),
                ),
            ])
        })
        .collect();
    json::array(&items)
}

pub fn describe(graphs: &[CompiledGraph]) -> String {
    let mut lines = Vec::new();
    for graph in graphs {
        lines.push(format!(
            "graph {}{}",
            graph.text(graph.name),
            annotations(graph, &graph.graph_annotations)
        ));
        if graph.kind == Kind::Pure {
            describe_pure(graph, &mut lines);
            describe_choices(graph, &mut lines);
            continue;
        }
        describe_nodes(graph, &mut lines);
        describe_conditions(graph, &mut lines);
        describe_choices(graph, &mut lines);
        describe_edges(graph, &mut lines);
    }
    lines.join("\n") + "\n"
}

/// A pure graph's enums and inputs, which a machine graph does not have. Its choices follow.
fn describe_pure(graph: &CompiledGraph, lines: &mut Vec<String>) {
    lines.push("  kind pure".to_string());
    lines.push("  enums".to_string());
    for item in &graph.enums {
        let members: Vec<&str> = graph.members[index(&item.members)]
            .iter()
            .map(|member| graph.text(member.name))
            .collect();
        lines.push(format!(
            "    {} = {}",
            graph.text(item.name),
            members.join(" | ")
        ));
    }
    lines.push("  inputs".to_string());
    for input in &graph.inputs {
        lines.push(format!(
            "    {} : {}",
            graph.text(input.name),
            ty_name(graph, input.ty)
        ));
    }
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
