//! TypeScript syntax for the shared layer. Each graph becomes an index module (`<graph>.ts`) plus
//! node and choice chunk modules (`<graph>.nodes.<k>.ts`, `<graph>.choices.<k>.ts`). The layout
//! and the module plan come from `frontend::layered`; this module only renders them.

mod choices;
mod escape;
mod index;
mod nodes;
mod shared;
mod types;

use std::io::{self, Write};

use crate::frontend::layered::{GraphPlan, Layered, Syntax};
use crate::frontend::layout::{ChoiceChunk, NodeChunk};
use crate::frontend::registry::Target;
use crate::graph::CompiledGraph;

/// File name of the runtime module. Every index imports it as `./graphite`.
pub const RUNTIME_FILE: &str = "graphite.ts";

/// Extension of every generated module, without the dot.
const EXTENSION: &str = "ts";

const RUNTIME: &str = include_str!("runtime.ts");

/// Nodes and choices per chunk, as a power of two: `1 << SHIFT`.
pub const DEFAULT_SHIFT: u32 = 12;

/// The TypeScript front end with chunks of `1 << SHIFT` items.
pub type TypeScript<const SHIFT: u32 = DEFAULT_SHIFT> = Layered<TypeScriptSyntax, SHIFT>;

/// The default TypeScript front end.
pub const TYPESCRIPT: TypeScript = Layered::new(TypeScriptSyntax);

/// A TypeScript front end with `1 << SHIFT` items per chunk.
pub const fn chunked<const SHIFT: u32>() -> TypeScript<SHIFT> {
    Layered::new(TypeScriptSyntax)
}

pub const TARGET: Target = Target {
    key: "typescript",
    frontend: &TYPESCRIPT,
    debugger: Some("cli/debug.ts"),
};

/// TypeScript's rendering of the shared layout.
pub struct TypeScriptSyntax;

impl Syntax for TypeScriptSyntax {
    fn extension(&self) -> &'static str {
        EXTENSION
    }

    fn runtime_file_name(&self) -> &'static str {
        RUNTIME_FILE
    }

    fn runtime(&self) -> &'static str {
        RUNTIME
    }

    fn index(
        &self,
        graph: &CompiledGraph,
        plan: &GraphPlan,
        output: &mut dyn Write,
    ) -> io::Result<()> {
        index::write(graph, plan, output)
    }

    fn node_chunk(
        &self,
        _: &CompiledGraph,
        chunk: &NodeChunk,
        output: &mut dyn Write,
    ) -> io::Result<()> {
        nodes::write(chunk, output)
    }

    fn choice_chunk(
        &self,
        graph: &CompiledGraph,
        chunk: &ChoiceChunk,
        output: &mut dyn Write,
    ) -> io::Result<()> {
        choices::write(graph, chunk, output)
    }
}
