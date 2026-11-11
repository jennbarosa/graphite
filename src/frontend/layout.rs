//! Shared chunk layout. A chunk holds a contiguous range of nodes or choices, and every span in
//! it is local to the chunk. Node and edge targets, and condition indices, stay graph-global.
//!
//! Front ends render these rows in their own syntax, so the span bookkeeping lives here once.

use std::ops::Range;

use crate::graph::{Annotation, CompiledGraph, Node, Span, index};
use crate::interner::StringId;

/// A half-open range `start..end` into one of a chunk's arrays.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bounds {
    pub start: usize,
    pub end: usize,
}

impl Bounds {
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub fn range(self) -> Range<usize> {
        self.start..self.end
    }
}

/// Chunk `chunk` of `len` items, with `1 << shift` items per chunk.
pub fn chunk_range(chunk: usize, len: usize, shift: u32) -> Range<usize> {
    let size = 1 << shift;
    let start = chunk * size;
    start..(start + size).min(len)
}

/// How many chunks `len` items need. Zero items need no chunks.
pub fn chunk_count(len: usize, shift: u32) -> usize {
    len.div_ceil(1 << shift)
}

#[derive(Default)]
pub struct NodeChunk {
    pub nodes: Vec<NodeRow>,
    pub edges: Vec<EdgeRow>,
    pub annotations: Vec<AnnotationRow>,
    pub flags: Vec<StringId>,
}

pub struct NodeRow {
    pub name: StringId,
    pub looping: bool,
    pub end: bool,
    /// Into the chunk's `flags`.
    pub flags: Bounds,
    /// Into the chunk's `annotations`; a node's own annotations only.
    pub annotations: Bounds,
    /// Into the chunk's `edges`, in source order.
    pub outgoing: Bounds,
    /// Local index into the chunk's `edges`, for the node's sequence edge.
    pub sequence: Option<usize>,
    pub offset: usize,
}

pub struct EdgeRow {
    /// Graph-global edge ID.
    pub id: u32,
    /// Graph-global node ID.
    pub target: u32,
    /// Graph-global condition index.
    pub condition: Option<u32>,
    /// Into the chunk's `annotations`.
    pub annotations: Bounds,
}

pub struct AnnotationRow {
    pub key: StringId,
    pub value: Option<StringId>,
}

#[derive(Default)]
pub struct ChoiceChunk {
    pub choices: Vec<ChoiceRow>,
    pub arms: Vec<ArmRow>,
    pub annotations: Vec<AnnotationRow>,
}

pub struct ChoiceRow {
    pub name: StringId,
    /// Into the chunk's `arms`.
    pub arms: Bounds,
}

pub struct ArmRow {
    /// Graph-global condition index; `None` is the else arm.
    pub condition: Option<u32>,
    pub tag: StringId,
    /// Into the chunk's `annotations`.
    pub annotations: Bounds,
}

/// Lays out the nodes in `nodes`, with their outgoing edges, flags, and annotations. A node's
/// edges and their annotations follow the node's own annotations, so edge spans are in order.
pub fn node_chunk(graph: &CompiledGraph, nodes: Range<usize>) -> NodeChunk {
    let mut chunk = NodeChunk::default();
    for node in &graph.nodes[nodes] {
        let flags_start = chunk.flags.len();
        chunk
            .flags
            .extend_from_slice(&graph.flags[index(&node.flags)]);
        let flags = Bounds::new(flags_start, chunk.flags.len());
        let annotations = push_annotations(&mut chunk.annotations, graph, &node.annotations);
        let (outgoing, sequence) = push_edges(&mut chunk, graph, node);
        chunk.nodes.push(NodeRow {
            name: node.name,
            looping: node.looping,
            end: node.end,
            flags,
            annotations,
            outgoing,
            sequence,
            offset: node.offset,
        });
    }
    chunk
}

/// Appends `node`'s outgoing edges to `chunk`. Returns the edges' span and the local index of
/// the sequence edge, if the node has one.
fn push_edges(
    chunk: &mut NodeChunk,
    graph: &CompiledGraph,
    node: &Node,
) -> (Bounds, Option<usize>) {
    let start = chunk.edges.len();
    let mut sequence = None;
    for &edge_id in &graph.outgoing[index(&node.outgoing)] {
        let edge = &graph.edges[edge_id as usize];
        let annotations = push_annotations(&mut chunk.annotations, graph, &edge.annotations);
        if node.sequence == Some(edge_id) {
            sequence = Some(chunk.edges.len());
        }
        chunk.edges.push(EdgeRow {
            id: edge_id,
            target: edge.target,
            condition: edge.condition,
            annotations,
        });
    }
    (Bounds::new(start, chunk.edges.len()), sequence)
}

/// Lays out the choices in `choices`, with their arms and annotations. Each choice's arms are
/// contiguous, and each arm's annotations precede the arm's own span.
pub fn choice_chunk(graph: &CompiledGraph, choices: Range<usize>) -> ChoiceChunk {
    let mut chunk = ChoiceChunk::default();
    for choice in &graph.choices[choices] {
        let arms_start = chunk.arms.len();
        for arm in &graph.arms[index(&choice.arms)] {
            let annotations = push_annotations(&mut chunk.annotations, graph, &arm.annotations);
            chunk.arms.push(ArmRow {
                condition: arm.condition,
                tag: arm.tag,
                annotations,
            });
        }
        chunk.choices.push(ChoiceRow {
            name: choice.name,
            arms: Bounds::new(arms_start, chunk.arms.len()),
        });
    }
    chunk
}

/// Appends the annotations in `span` to `rows` and returns the span they occupy there.
fn push_annotations(rows: &mut Vec<AnnotationRow>, graph: &CompiledGraph, span: &Span) -> Bounds {
    let start = rows.len();
    for Annotation { key, value } in &graph.annotations[index(span)] {
        rows.push(AnnotationRow {
            key: *key,
            value: *value,
        });
    }
    Bounds::new(start, rows.len())
}
