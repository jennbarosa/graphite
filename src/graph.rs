//! Owned compiled data. IDs are local to one graph; node zero is its entry.

use std::ops::Range;

use crate::interner::{Interner, StringId};
use crate::kind::Kind;

pub struct CompiledGraph {
    pub name: StringId,
    /// Byte offset of the graph's name in the source, for diagnostics.
    pub offset: usize,
    /// Whether this is a state machine or a pure decision table.
    pub kind: Kind,
    pub strings: Interner,
    pub nodes: Vec<Node>,
    /// Edges remain in source order. Nodes index them through `outgoing`.
    pub edges: Vec<Edge>,
    /// CSR adjacency: each node's range contains edge IDs in source order.
    pub outgoing: Vec<u32>,
    pub conditions: Vec<StringId>,
    /// What each condition tests, parallel to `conditions`. Empty for machine graphs.
    pub tests: Vec<Test>,
    pub choices: Vec<Choice>,
    pub arms: Vec<Arm>,
    /// Pure graphs only: the enums, their members, and the inputs.
    pub enums: Vec<Enum>,
    pub members: Vec<Member>,
    pub inputs: Vec<Input>,
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

/// The node annotation whose value names a stage's task.
pub const TASK_ANNOTATION: &str = "task";

/// The value of node `node`'s `task` annotation, if it has one.
pub fn task_of(graph: &CompiledGraph, node: usize) -> Option<&str> {
    graph.annotations[index(&graph.nodes[node].annotations)]
        .iter()
        .find(|entry| graph.text(entry.key) == TASK_ANNOTATION)
        .and_then(|entry| entry.value)
        .map(|value| graph.text(value))
}

/// Ranges into the graph's arrays, stored as `u32` to keep records small.
pub type Span = Range<u32>;

/// Narrows a span built from `usize` positions. Callers check counts against `u32` first.
pub fn narrow(range: Range<usize>) -> Span {
    range.start as u32..range.end as u32
}

/// Widens a stored span for slicing. Callers check that counts fit in `u32` as they grow.
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
    /// The enum the arms' tags come from, as an index into `enums`. `None` in machine graphs.
    pub ty: Option<u32>,
    /// The annotations on the `choose` line, a range into `annotations`.
    pub annotations: Span,
    /// Byte offset of the `choose` line, for diagnostics.
    pub offset: usize,
}

/// A pure file's enum: its members are a range into `members`.
#[derive(Debug)]
pub struct Enum {
    pub name: StringId,
    pub annotations: Span,
    pub members: Span,
    /// Byte offset of the enum's name, for diagnostics.
    pub offset: usize,
}

#[derive(Debug)]
pub struct Member {
    pub name: StringId,
    pub annotations: Span,
    /// Byte offset of the member's name, for diagnostics.
    pub offset: usize,
}

/// A pure file's input: a bool, or one enum.
#[derive(Debug)]
pub struct Input {
    pub name: StringId,
    pub ty: Ty,
    pub annotations: Span,
    /// Byte offset of the input's name, for diagnostics.
    pub offset: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ty {
    Bool,
    /// An index into `enums`.
    Enum(u32),
}

/// What a condition tests. `Flag` reads a bool input; `Is` compares an enum input with one of
/// its members, by the member's position in the enum.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Test {
    Flag(u32),
    Is(u32, u32),
}

#[derive(Debug)]
pub struct Arm {
    pub condition: Option<u32>,
    pub tag: StringId,
    pub annotations: Span,
    /// In a pure arm, the position of `tag` among its choose's enum members. Zero in machine
    /// graphs, which have no enums.
    pub member: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct Annotation {
    pub key: StringId,
    /// `None` distinguishes `@key` from an annotation with a value.
    pub value: Option<StringId>,
}
