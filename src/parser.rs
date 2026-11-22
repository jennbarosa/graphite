//! Single-pass DSL compilation: the lexer's tokens are read straight into checked graph arrays.

mod compiler;
mod error;
mod grammar;
mod structure;
mod tokens;

use crate::graph::CompiledGraph;

pub use error::ParseError;

/// The keyword that opens each graph block.
pub const GRAPH_KEYWORD: &str = "graph";

/// Additional legal node flags. Core `loop` and `end` are always available. `pure` makes the
/// graphs pure, for signals the library cannot read from the text (flags, names, targets).
#[derive(Default)]
pub struct Schema<'a> {
    pub node_flags: &'a [&'a str],
    pub pure: bool,
}

/// Compile one lexer stream directly into owned, checked graph arrays. Each graph block in
/// the source yields one graph, in source order.
pub fn parse(source: &str, schema: &Schema<'_>) -> Result<Vec<CompiledGraph>, ParseError> {
    parse_body(source, 0, schema)
}

/// [`parse`] from byte `body`, past the directives that [`crate::preprocess`] reads.
pub(crate) fn parse_body(
    source: &str,
    body: usize,
    schema: &Schema<'_>,
) -> Result<Vec<CompiledGraph>, ParseError> {
    check_source_len(source)?;
    grammar::parse(source, body, schema)
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
