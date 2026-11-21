//! Front ends turn checked graphs into host-facing output. The graph core knows nothing about
//! any of them: each front end lives in its own folder and implements [`Frontend`].

pub mod escape;
pub mod lang;
pub mod layered;
pub mod layout;
pub mod names;
pub mod pure;
pub mod registry;
pub mod rust;
pub mod typescript;

use std::io::{self, Write};

use crate::graph::CompiledGraph;
use crate::kind::Kind;

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

/// Two names in one output clash. The error travels as an `io::Error`, and carries the byte offset
/// of the declaration that clashes, so the pipeline can point at it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameClash {
    pub message: String,
    pub offset: usize,
}

impl std::fmt::Display for NameClash {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for NameClash {}

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

    /// The file name of the runtime, which every index imports, or `None` when the front end
    /// writes no runtime.
    fn runtime_file_name(&self) -> Option<&'static str>;

    /// The extension of this front end's files, without the dot. Inference matches on it.
    fn extension(&self) -> &'static str;

    /// The kind of graph this front end writes. Machine front ends are the default.
    fn kind(&self) -> Kind {
        Kind::Machine
    }

    /// Finds what would stop `graph` from being written, such as a name clash, without writing
    /// anything. The default renders every module into nowhere, so a check finds exactly what a
    /// write would. A front end whose check is cheaper overrides it.
    fn check_graph(&self, graph: &CompiledGraph) -> io::Result<()> {
        for module in self.modules(graph) {
            self.emit_module(graph, module, &mut io::sink())?;
        }
        Ok(())
    }
}
