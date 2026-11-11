//! Renders a node chunk as a `NodeChunk` module. The rows come from the shared layout. They are
//! packed into little-endian 32-bit columns and base64-encoded, so a module holds no per-node
//! objects. `decodeNodeChunk` in the runtime reads the same column order.

use std::io::{self, Write};

use crate::frontend::layout::{Bounds, NodeChunk};
use crate::interner::StringId;

use super::shared::runtime_import;

/// Stored in a column for `None`. The runtime reads it back as `null`.
const NONE: u32 = u32::MAX;

pub(super) fn write(chunk: &NodeChunk, output: &mut dyn Write) -> io::Result<()> {
    runtime_import("type { NodeChunk }", output)?;
    runtime_import("{ decodeNodeChunk }", output)?;
    write!(
        output,
        "\nexport const chunk: NodeChunk = decodeNodeChunk({{nodes:{},edges:{},annotations:{},flags:{}}}, \"",
        chunk.nodes.len(),
        chunk.edges.len(),
        chunk.annotations.len(),
        chunk.flags.len(),
    )?;
    let bytes: Vec<u8> = words(chunk)?
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect();
    output.write_all(base64(&bytes).as_bytes())?;
    output.write_all(b"\");\n")
}

/// The columns, in the order `decodeNodeChunk` reads them.
fn words(chunk: &NodeChunk) -> io::Result<Vec<u32>> {
    let mut out = Vec::new();
    for node in &chunk.nodes {
        out.push(word(node.name.index())?);
    }
    for node in &chunk.nodes {
        out.push(word(node.offset)?);
    }
    for node in &chunk.nodes {
        out.push(optional(node.sequence)?);
    }
    for node in &chunk.nodes {
        out.push(u32::from(node.looping) | (u32::from(node.end) << 1));
    }
    for node in &chunk.nodes {
        push_span(&mut out, node.flags)?;
    }
    for node in &chunk.nodes {
        push_span(&mut out, node.annotations)?;
    }
    for node in &chunk.nodes {
        push_span(&mut out, node.outgoing)?;
    }
    for edge in &chunk.edges {
        out.push(edge.id);
    }
    for edge in &chunk.edges {
        out.push(edge.target);
    }
    for edge in &chunk.edges {
        out.push(optional(edge.condition.map(|id| id as usize))?);
    }
    for edge in &chunk.edges {
        push_span(&mut out, edge.annotations)?;
    }
    for entry in &chunk.annotations {
        out.push(word(entry.key.index())?);
    }
    for entry in &chunk.annotations {
        out.push(optional(entry.value.map(StringId::index))?);
    }
    for id in &chunk.flags {
        out.push(word(id.index())?);
    }
    Ok(out)
}

fn push_span(out: &mut Vec<u32>, span: Bounds) -> io::Result<()> {
    out.push(word(span.start)?);
    out.push(word(span.end)?);
    Ok(())
}

fn word(value: usize) -> io::Result<u32> {
    u32::try_from(value).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "a chunk value does not fit a 32-bit column",
        )
    })
}

fn optional(value: Option<usize>) -> io::Result<u32> {
    value.map_or(Ok(NONE), word)
}

/// Standard base64 with `=` padding. The output needs no escaping inside a double-quoted string.
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for group in bytes.chunks(3) {
        let first = u32::from(group[0]);
        let second = u32::from(group.get(1).copied().unwrap_or(0));
        let third = u32::from(group.get(2).copied().unwrap_or(0));
        let value = (first << 16) | (second << 8) | third;
        // A group of n bytes yields n + 1 characters; the rest of the four are padding.
        for (position, shift) in [18u32, 12, 6, 0].into_iter().enumerate() {
            if position > group.len() {
                out.push('=');
            } else {
                let index = ((value >> shift) & 63) as usize;
                out.push(char::from(ALPHABET[index]));
            }
        }
    }
    out
}
