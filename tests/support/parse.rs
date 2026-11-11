use graphite::graph::CompiledGraph;
use graphite::parser::{Capacity, ParseError, Schema};

/// Fixed bounds for the small hand-written fixtures; no source census.
pub fn parse(source: &str, schema: &Schema<'_>) -> Result<Vec<CompiledGraph>, ParseError> {
    let capacity = Capacity {
        nodes: 64,
        edges: 64,
        conditions: 64,
        choices: 16,
        arms: 64,
        annotations: 128,
        flags: if schema.node_flags.is_empty() { 0 } else { 64 },
        strings: 256,
        string_bytes: source.len(),
    };
    graphite::parser::parse(source, schema, &[capacity; 16])
}
