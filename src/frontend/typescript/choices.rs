//! Renders a choice chunk as a `Choices` type and a `ChoiceChunk` module. The rows come from the
//! shared layout, so each choice and its arms are in the same module.

use std::io::{self, Write};

use crate::frontend::layout::ChoiceChunk;
use crate::graph::CompiledGraph;

use super::shared::{annotation_row, optional, runtime_import};
use super::types::choice_type;

pub(super) fn write(
    graph: &CompiledGraph,
    chunk: &ChoiceChunk,
    output: &mut dyn Write,
) -> io::Result<()> {
    runtime_import("type { ChoiceChunk }", output)?;
    output.write_all(b"\nexport type Choices = {\n")?;
    for choice in &chunk.choices {
        choice_type(graph, chunk, choice, output)?;
    }
    output.write_all(b"};\n\nexport const chunk: ChoiceChunk = {\nchoices: [\n")?;
    for choice in &chunk.choices {
        writeln!(
            output,
            "{{name:{},arms:[{},{}]}},",
            choice.name.index(),
            choice.arms.start,
            choice.arms.end
        )?;
    }
    output.write_all(b"],\narms: [\n")?;
    for arm in &chunk.arms {
        output.write_all(b"{condition:")?;
        optional(arm.condition, output)?;
        writeln!(
            output,
            ",tag:{},annotations:[{},{}]}},",
            arm.tag.index(),
            arm.annotations.start,
            arm.annotations.end
        )?;
    }
    output.write_all(b"],\nannotations: [\n")?;
    for entry in &chunk.annotations {
        annotation_row(entry, output)?;
    }
    output.write_all(b"],\n};\n")
}
