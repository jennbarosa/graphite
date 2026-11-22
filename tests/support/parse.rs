use graphite::graph::CompiledGraph;
use graphite::parser::{ParseError, Schema};

pub fn parse(source: &str, schema: &Schema<'_>) -> Result<Vec<CompiledGraph>, ParseError> {
    graphite::parser::parse(source, schema)
}
