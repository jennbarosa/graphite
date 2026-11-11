//! Front ends turn checked graphs into host-facing output. The graph core knows nothing about
//! any of them: each front end lives in its own folder and implements [`Frontend`].

pub mod layered;
pub mod layout;
pub mod registry;
pub mod typescript;

use std::io::{self, Write};

use crate::graph::CompiledGraph;

/// One file of a graph's output. Nodes and choices are split into fixed-size chunks. The index
/// is the one file that grows with the whole graph: it holds the graph's names and tables.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Module {
    /// The graph's entry module: names, tables shared by all chunks, and the chunk list.
    Index,
    /// Nodes `chunk << shift ..` with their outgoing edges, flags, and annotations.
    Nodes(usize),
    /// Choices `chunk << shift ..` with their arms and annotations.
    Choices(usize),
}

/// Emits unmodified output from a successful parser call.
///
/// A failure may leave partial output. The caller owns buffering, flushing, and file
/// placement; front ends neither flush nor close the supplied writer. Emission borrows the
/// graphs and never mutates them.
pub trait Frontend {
    /// Every module a graph produces, in a stable order. `Index` comes first.
    fn modules(&self, graph: &CompiledGraph) -> Vec<Module>;

    /// The file name for one of this graph's modules.
    fn file_name(&self, graph: &CompiledGraph, module: Module) -> String;

    fn emit_module(
        &self,
        graph: &CompiledGraph,
        module: Module,
        output: &mut dyn Write,
    ) -> io::Result<()>;

    /// The runtime that every graph's index imports.
    fn emit_runtime(&self, output: &mut dyn Write) -> io::Result<()>;

    /// The file name of the runtime, which every index imports.
    fn runtime_file_name(&self) -> &'static str;
}
