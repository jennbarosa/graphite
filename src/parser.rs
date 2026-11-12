//! Single-pass DSL compilation into caller-sized buffers for trusted input.

mod capacity;
mod compiler;
mod error;
mod grammar;
mod structure;
mod tokens;

use crate::graph::CompiledGraph;

pub use capacity::Capacity;
pub use error::ParseError;

/// The keyword that opens each graph block.
pub const GRAPH_KEYWORD: &str = "graph";

/// Additional legal node flags. Core `loop` and `end` are always available.
#[derive(Default)]
pub struct Schema<'a> {
    pub node_flags: &'a [&'a str],
}

/// Compile one lexer stream directly into owned, checked graph arrays.
///
/// Each graph consumes one capacity hint in source order; extra hints are unused.
/// Missing or insufficient hints return an error, without growing buffers or
/// scanning the source again. No tokens or intermediate syntax tree are retained.
/// Expected O(source bytes + reserved strings + V + E), with fixed allocations per graph.
pub fn parse(
    source: &str,
    schema: &Schema<'_>,
    capacities: &[Capacity],
) -> Result<Vec<CompiledGraph>, ParseError> {
    check_source_len(source)?;
    grammar::parse(source, schema, capacities)
}

/// Positions are `u32`, so a longer source cannot be parsed.
fn check_source_len(source: &str) -> Result<(), ParseError> {
    if source.len() > u32::MAX as usize {
        return Err(ParseError::new(
            "source is larger than 4 GiB; positions must fit in u32",
            0,
        ));
    }
    Ok(())
}
