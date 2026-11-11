//! Single-pass DSL compilation into caller-sized buffers for trusted input.

mod capacity;
mod compiler;
mod error;
mod structure;
mod tokens;

use std::collections::HashSet;

use crate::graph::CompiledGraph;
use crate::lexer::TokenKind;

pub use capacity::Capacity;
use capacity::{buffer, push};
use compiler::Builder;
pub use error::ParseError;
use tokens::Tokens;

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
    if source.len() > u32::MAX as usize {
        return Err(ParseError::new(
            "source is larger than 4 GiB; positions must fit in u32",
            0,
        ));
    }
    let mut graphs = buffer(capacities.len())?;
    let mut names = HashSet::new();
    names
        .try_reserve(capacities.len())
        .map_err(|_| ParseError::new("could not reserve memory for graph names", 0))?;
    let flags: HashSet<&str> = schema.node_flags.iter().copied().collect();
    let mut tokens = Tokens::new(source);
    while tokens.peek()?.is_some() {
        tokens.expect(TokenKind::Word(GRAPH_KEYWORD))?;
        let (name, offset) = tokens.name("expected a graph name after `graph`")?;
        let capacity = capacities.get(graphs.len()).ok_or_else(|| {
            ParseError::new(
                "no capacity hint for this graph; pass one per graph",
                offset,
            )
        })?;
        if !names.insert(name) {
            return Err(ParseError::new(
                "graph name is already used by an earlier graph in this source",
                offset,
            ));
        }
        tokens.expect(TokenKind::Newline)?;
        tokens.expect(TokenKind::Indent)?;
        let mut builder = Builder::new(*capacity, name, &mut tokens, &flags)?;
        builder.graph.offset = offset;
        builder.body()?;
        structure::finish(&mut builder.graph)?;
        push(&mut graphs, builder.graph, offset)?;
    }
    if graphs.is_empty() {
        return Err(tokens.error("expected a `graph` block; the source declares no graphs"));
    }
    Ok(graphs)
}
