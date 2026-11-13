//! Caller-supplied storage bounds. This module never reads or lexes source text.

use crate::graph::CompiledGraph;
use crate::interner::Interner;

use super::ParseError;

/// Upper bounds for one graph, supplied by its producer before parsing.
///
/// Strings count distinct interned words, including the graph name, domain flags,
/// annotation keys/values, and tags. String bytes count their total UTF-8 length.
/// Core keywords/flags and references to declared nodes need no additional strings.
#[derive(Clone, Copy, Debug, Default)]
pub struct Capacity {
    pub nodes: usize,
    pub edges: usize,
    pub conditions: usize,
    pub choices: usize,
    pub arms: usize,
    pub annotations: usize,
    pub flags: usize,
    pub strings: usize,
    pub string_bytes: usize,
}

impl Capacity {
    pub(super) fn allocate(self, name: &str, offset: usize) -> Result<CompiledGraph, ParseError> {
        for count in [
            self.nodes,
            self.edges,
            self.conditions,
            self.choices,
            self.arms,
            self.annotations,
            self.flags,
        ] {
            if u32::try_from(count).is_err() {
                return Err(ParseError::new(
                    "capacity exceeds u32 storage; counts must fit in 32 bits",
                    offset,
                ));
            }
        }
        let mut strings = Interner::with_capacity(self.strings, self.string_bytes)
            .map_err(|_| ParseError::new("could not reserve memory for graph strings", offset))?;
        let name = strings.intern(name).map_err(|_| {
            ParseError::new(
                "string capacity exhausted; `parse` callers raise the graph's `strings` hint, and `compile` callers report a sizing bug",
                offset,
            )
        })?;
        Ok(CompiledGraph {
            name,
            offset,
            strings,
            nodes: buffer(self.nodes)?,
            edges: buffer(self.edges)?,
            outgoing: buffer(self.edges)?,
            conditions: buffer(self.conditions)?,
            choices: buffer(self.choices)?,
            arms: buffer(self.arms)?,
            annotations: buffer(self.annotations)?,
            flags: buffer(self.flags)?,
            graph_annotations: 0..0,
        })
    }
}

pub(super) fn buffer<T>(capacity: usize) -> Result<Vec<T>, ParseError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .map_err(|_| ParseError::new("could not reserve memory for the graph", 0))?;
    Ok(values)
}

pub(super) fn push<T>(values: &mut Vec<T>, value: T, offset: usize) -> Result<(), ParseError> {
    if values.len() == values.capacity() {
        return Err(ParseError::new(
            "graph capacity exhausted; raise its count hints",
            offset,
        ));
    }
    values.push(value);
    Ok(())
}
