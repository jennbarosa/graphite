//! Renders a graph's index module: its runtime import, chunk imports, name vocabulary, shared
//! tables, and chunk list.

use std::io::{self, Write};

use crate::frontend::layered::{GraphPlan, choice_stem, node_stem};
use crate::graph::{CompiledGraph, index};

use super::{escape::string, shared::runtime_import, types};

const ALIASES: &str = r#"export type NodeName = Vocab["nodes"];
export type ConditionName = Vocab["conditions"];
export type ChoiceName = keyof Vocab["choices"];
export type ChoiceArm<C extends ChoiceName> = Vocab["choices"][C];
export type AnnotationEntry = Vocab["annotations"];
"#;

pub(super) fn write(
    graph: &CompiledGraph,
    plan: &GraphPlan,
    output: &mut dyn Write,
) -> io::Result<()> {
    write_imports(plan, output)?;
    write_vocab(graph, plan, output)?;
    write_graph(graph, plan, output)
}

fn write_imports(plan: &GraphPlan, output: &mut dyn Write) -> io::Result<()> {
    runtime_import("{ defineGraph }", output)?;
    for chunk in 0..plan.node_chunks {
        write!(output, "import {{ chunk as nodes{chunk} }} from ")?;
        string(&format!("./{}", node_stem(&plan.stem, chunk)), output)?;
        output.write_all(b";\n")?;
    }
    for chunk in 0..plan.choice_chunks {
        write!(
            output,
            "import {{ chunk as choices{chunk}, type Choices as Choices{chunk} }} from "
        )?;
        string(&format!("./{}", choice_stem(&plan.stem, chunk)), output)?;
        output.write_all(b";\n")?;
    }
    Ok(())
}

/// The `Vocab` type that names every node, condition, flag, annotation, and choice, and the
/// aliases built from it.
// shortcut: the index holds every node name in one union, and TypeScript gives up on very large unions; split the vocabulary per chunk before a graph gets that big.
fn write_vocab(graph: &CompiledGraph, plan: &GraphPlan, output: &mut dyn Write) -> io::Result<()> {
    output.write_all(b"\nexport type Vocab = {\nreadonly nodes: ")?;
    types::names(graph, graph.nodes.iter().map(|node| node.name), output)?;
    output.write_all(b";\nreadonly conditions: ")?;
    types::names(graph, graph.conditions.iter().copied(), output)?;
    output.write_all(b";\nreadonly flags: ")?;
    types::names(graph, graph.flags.iter().copied(), output)?;
    output.write_all(b";\nreadonly annotations: ")?;
    types::annotation_union(graph, output)?;
    output.write_all(b";\nreadonly choices: ")?;
    if plan.choice_chunks == 0 {
        output.write_all(b"{}")?;
    }
    for chunk in 0..plan.choice_chunks {
        if chunk > 0 {
            output.write_all(b" & ")?;
        }
        write!(output, "Choices{chunk}")?;
    }
    output.write_all(b";\n};\n")?;
    output.write_all(ALIASES.as_bytes())
}

/// The exported `graph` literal: its name, shared tables, and chunk list.
fn write_graph(graph: &CompiledGraph, plan: &GraphPlan, output: &mut dyn Write) -> io::Result<()> {
    output.write_all(b"\nexport const graph = defineGraph<Vocab>({\nname:")?;
    string(&plan.stem, output)?;
    output.write_all(b",\nstrings: [")?;
    for value in graph.strings.strings() {
        string(value, output)?;
        output.write_all(b",")?;
    }
    output.write_all(b"],\nannotations: [")?;
    for entry in &graph.annotations[index(&graph.graph_annotations)] {
        write!(output, "[{},", entry.key.index())?;
        match entry.value {
            Some(value) => write!(output, "{}],", value.index())?,
            None => output.write_all(b"null],")?,
        }
    }
    output.write_all(b"],\nconditions: [")?;
    for id in &graph.conditions {
        write!(output, "{},", id.index())?;
    }
    output.write_all(b"],\nflags: [")?;
    for id in &graph.flags {
        write!(output, "{},", id.index())?;
    }
    write!(
        output,
        "],\nnodeShift: {},\nnodeCount: {},\nnodeChunks: [",
        plan.shift,
        graph.nodes.len()
    )?;
    for chunk in 0..plan.node_chunks {
        write!(output, "nodes{chunk},")?;
    }
    write!(
        output,
        "],\nchoiceShift: {},\nchoiceCount: {},\nchoiceChunks: [",
        plan.shift,
        graph.choices.len()
    )?;
    for chunk in 0..plan.choice_chunks {
        write!(output, "choices{chunk},")?;
    }
    output.write_all(b"],\n});\n")
}
