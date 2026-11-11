//! TypeScript type declarations. Names come from the graph's string table, and the choice types
//! come from a choice chunk's rows.

use std::collections::HashSet;
use std::io::{self, Write};

use foldhash::fast::FixedState;

use crate::frontend::layout::{ArmRow, ChoiceChunk, ChoiceRow};
use crate::graph::CompiledGraph;
use crate::interner::StringId;

use super::escape::string;

/// `never | "a" | "b"`, which is `never` for an empty list.
pub(super) fn names(
    graph: &CompiledGraph,
    ids: impl Iterator<Item = StringId>,
    output: &mut dyn Write,
) -> io::Result<()> {
    output.write_all(b"never")?;
    for id in ids {
        output.write_all(b" | ")?;
        name(graph, id, output)?;
    }
    Ok(())
}

/// Every distinct (key, value) pair in the graph, in first-appearance order.
pub(super) fn annotation_union(graph: &CompiledGraph, output: &mut dyn Write) -> io::Result<()> {
    // The fast hasher keeps this as cheap as the table it replaced. The set holds at most one
    // entry per annotation, which the parser already reserved.
    let mut seen =
        HashSet::with_capacity_and_hasher(graph.annotations.len(), FixedState::default());
    output.write_all(b"never")?;
    for entry in &graph.annotations {
        if !seen.insert((entry.key.index(), entry.value.map(StringId::index))) {
            continue;
        }
        output.write_all(b" | ")?;
        annotation(graph, entry.key, entry.value, output)?;
    }
    Ok(())
}

/// One choice's type: the union of its arms, so each arm keeps its own tag and annotations.
pub(super) fn choice_type(
    graph: &CompiledGraph,
    chunk: &ChoiceChunk,
    choice: &ChoiceRow,
    output: &mut dyn Write,
) -> io::Result<()> {
    output.write_all(b"readonly [")?;
    name(graph, choice.name, output)?;
    output.write_all(b"]: never")?;
    for arm in &chunk.arms[choice.arms.range()] {
        arm_type(graph, chunk, arm, output)?;
    }
    output.write_all(b";\n")
}

fn arm_type(
    graph: &CompiledGraph,
    chunk: &ChoiceChunk,
    arm: &ArmRow,
    output: &mut dyn Write,
) -> io::Result<()> {
    output.write_all(b" | { readonly condition: ")?;
    if let Some(condition) = arm.condition {
        name(graph, graph.conditions[condition as usize], output)?;
    } else {
        output.write_all(b"null")?;
    }
    output.write_all(b"; readonly tag: ")?;
    name(graph, arm.tag, output)?;
    output.write_all(b"; readonly annotations: readonly [")?;
    for entry in &chunk.annotations[arm.annotations.range()] {
        annotation(graph, entry.key, entry.value, output)?;
        output.write_all(b",")?;
    }
    output.write_all(b"] }")
}

fn name(graph: &CompiledGraph, id: StringId, output: &mut dyn Write) -> io::Result<()> {
    string(graph.text(id), output)
}

fn annotation(
    graph: &CompiledGraph,
    key: StringId,
    value: Option<StringId>,
    output: &mut dyn Write,
) -> io::Result<()> {
    output.write_all(b"{ readonly key: ")?;
    name(graph, key, output)?;
    output.write_all(b"; readonly value: ")?;
    if let Some(value) = value {
        name(graph, value, output)?;
    } else {
        output.write_all(b"null")?;
    }
    output.write_all(b" }")
}
