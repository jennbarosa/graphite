//! The generic front end. A [`Syntax`] renders chunk rows in one language. [`Layered`] adds the
//! rest: the module plan, file names, and dispatch. A new front end implements `Syntax` only.

use std::io::{self, Write};

use super::layout::{self, ChoiceChunk, NodeChunk, chunk_count, chunk_range};
use super::{Frontend, Module};
use crate::graph::CompiledGraph;

/// What an index module needs to know about its graph's chunks.
pub struct GraphPlan {
    /// The graph's name, which prefixes every module's file name.
    pub stem: String,
    pub shift: u32,
    pub node_chunks: usize,
    pub choice_chunks: usize,
}

/// The module stem of node chunk `chunk` of the graph named `stem`, such as `door.nodes.0`.
pub fn node_stem(stem: &str, chunk: usize) -> String {
    format!("{stem}.nodes.{chunk}")
}

/// The module stem of choice chunk `chunk` of the graph named `stem`, such as `door.choices.0`.
pub fn choice_stem(stem: &str, chunk: usize) -> String {
    format!("{stem}.choices.{chunk}")
}

/// Renders one language. Every method writes text and nothing else; the layout is already done.
pub trait Syntax {
    /// The extension of every module, without the dot.
    fn extension(&self) -> &'static str;

    /// The file name of the runtime, which every index imports.
    fn runtime_file_name(&self) -> &'static str;

    /// The runtime's source, written once per output directory.
    fn runtime(&self) -> &'static str;

    /// The entry module of a graph, which names the graph and its chunks.
    fn index(
        &self,
        graph: &CompiledGraph,
        plan: &GraphPlan,
        output: &mut dyn Write,
    ) -> io::Result<()>;

    fn node_chunk(
        &self,
        graph: &CompiledGraph,
        chunk: &NodeChunk,
        output: &mut dyn Write,
    ) -> io::Result<()>;

    fn choice_chunk(
        &self,
        graph: &CompiledGraph,
        chunk: &ChoiceChunk,
        output: &mut dyn Write,
    ) -> io::Result<()>;
}

/// A front end built from a [`Syntax`]. `SHIFT` sets the chunk size to `1 << SHIFT` items.
pub struct Layered<S, const SHIFT: u32> {
    syntax: S,
}

impl<S, const SHIFT: u32> Layered<S, SHIFT> {
    pub const fn new(syntax: S) -> Self {
        Self { syntax }
    }
}

impl<S: Syntax, const SHIFT: u32> Layered<S, SHIFT> {
    fn plan(&self, graph: &CompiledGraph) -> GraphPlan {
        GraphPlan {
            stem: graph.text(graph.name).to_owned(),
            shift: SHIFT,
            node_chunks: chunk_count(graph.nodes.len(), SHIFT),
            choice_chunks: chunk_count(graph.choices.len(), SHIFT),
        }
    }
}

impl<S: Syntax, const SHIFT: u32> Frontend for Layered<S, SHIFT> {
    fn modules(&self, graph: &CompiledGraph) -> Vec<Module> {
        let plan = self.plan(graph);
        let mut modules = vec![Module::Index];
        modules.extend((0..plan.node_chunks).map(Module::Nodes));
        modules.extend((0..plan.choice_chunks).map(Module::Choices));
        modules
    }

    fn file_name(&self, graph: &CompiledGraph, module: Module) -> String {
        let stem = graph.text(graph.name);
        let file = match module {
            Module::Index => stem.to_owned(),
            Module::Nodes(chunk) => node_stem(stem, chunk),
            Module::Choices(chunk) => choice_stem(stem, chunk),
        };
        format!("{file}.{}", self.syntax.extension())
    }

    fn emit_module(
        &self,
        graph: &CompiledGraph,
        module: Module,
        output: &mut dyn Write,
    ) -> io::Result<()> {
        match module {
            Module::Index => self.syntax.index(graph, &self.plan(graph), output),
            Module::Nodes(chunk) => {
                let range = chunk_range(chunk, graph.nodes.len(), SHIFT);
                let rows = layout::node_chunk(graph, range);
                self.syntax.node_chunk(graph, &rows, output)
            }
            Module::Choices(chunk) => {
                let range = chunk_range(chunk, graph.choices.len(), SHIFT);
                let rows = layout::choice_chunk(graph, range);
                self.syntax.choice_chunk(graph, &rows, output)
            }
        }
    }

    fn emit_runtime(&self, output: &mut dyn Write) -> io::Result<()> {
        output.write_all(self.syntax.runtime().as_bytes())
    }

    fn runtime_file_name(&self) -> &'static str {
        self.syntax.runtime_file_name()
    }
}
