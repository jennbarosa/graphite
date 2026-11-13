//! Owned compiled data. IDs are local to one graph; node zero is its entry.

use std::ops::Range;

use crate::interner::{Interner, StringId};

pub struct CompiledGraph {
    pub name: StringId,
    /// Byte offset of the graph's name in the source, for diagnostics.
    pub offset: usize,
    pub strings: Interner,
    pub nodes: Vec<Node>,
    /// Edges remain in source order. Nodes index them through `outgoing`.
    pub edges: Vec<Edge>,
    /// CSR adjacency: each node's range contains edge IDs in source order.
    pub outgoing: Vec<u32>,
    pub conditions: Vec<StringId>,
    pub choices: Vec<Choice>,
    pub arms: Vec<Arm>,
    pub annotations: Vec<Annotation>,
    /// Domain flags only; core flags are represented directly on `Node`.
    pub flags: Vec<StringId>,
    /// The annotations on the `graph` line, a range into `annotations`.
    pub graph_annotations: Span,
}

impl CompiledGraph {
    /// The string behind an ID issued by this graph's parse.
    pub fn text(&self, id: StringId) -> &str {
        self.strings
            .resolve(id)
            .expect("parser string IDs resolve in their own graph")
    }
}

/// Ranges into the graph's arrays, stored as `u32` to keep records small.
pub type Span = Range<u32>;

/// Narrows a span built from `usize` positions. Callers check counts against `u32` first.
pub fn narrow(range: Range<usize>) -> Span {
    range.start as u32..range.end as u32
}

/// Widens a stored span for slicing. Counts are checked against `u32` at allocation.
pub fn index(span: &Span) -> Range<usize> {
    span.start as usize..span.end as usize
}

#[derive(Debug)]
pub struct Node {
    pub name: StringId,
    pub looping: bool,
    pub end: bool,
    pub flags: Span,
    pub annotations: Span,
    pub outgoing: Span,
    pub sequence: Option<u32>,
    pub offset: usize,
}

#[derive(Debug)]
pub struct Edge {
    pub source: u32,
    pub target: u32,
    pub condition: Option<u32>,
    pub annotations: Span,
}

#[derive(Debug)]
pub struct Choice {
    pub name: StringId,
    pub arms: Span,
}

#[derive(Debug)]
pub struct Arm {
    pub condition: Option<u32>,
    pub tag: StringId,
    pub annotations: Span,
}

#[derive(Clone, Copy, Debug)]
pub struct Annotation {
    pub key: StringId,
    /// `None` distinguishes `@key` from an annotation with a value.
    pub value: Option<StringId>,
}
