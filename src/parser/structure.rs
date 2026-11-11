use crate::graph::{CompiledGraph, index};

use super::ParseError;

/// O(V + E). Degree counts become CSR ranges in place, then reachability is checked.
pub(super) fn finish(graph: &mut CompiledGraph) -> Result<(), ParseError> {
    let mut start = 0u32;
    for node in &mut graph.nodes {
        let degree = node.outgoing.end;
        if degree == 0 && !node.end {
            return Err(ParseError::new(
                "node needs an outgoing edge or the `end` flag",
                node.offset,
            ));
        }
        node.outgoing = start..start;
        start += degree;
    }
    graph.outgoing.resize(graph.edges.len(), 0);
    for (id, edge) in graph.edges.iter().enumerate() {
        let node = &mut graph.nodes[edge.source as usize];
        graph.outgoing[node.outgoing.end as usize] = id as u32;
        node.outgoing.end += 1;
    }
    check_reachability(graph)
}

/// Walks every edge, ignoring conditions, from the entry node. A node the walk misses is dead
/// code, so it is a build error. A graph with no reachable `end` is allowed: loops run on purpose.
/// Two pre-sized buffers, so the allocation count does not grow with the graph.
fn check_reachability(graph: &CompiledGraph) -> Result<(), ParseError> {
    let mut visited = vec![false; graph.nodes.len()];
    let mut pending = Vec::with_capacity(graph.nodes.len());
    visited[0] = true;
    pending.push(0u32);
    while let Some(id) = pending.pop() {
        let node = &graph.nodes[id as usize];
        for &edge_id in &graph.outgoing[index(&node.outgoing)] {
            let target = graph.edges[edge_id as usize].target;
            if !visited[target as usize] {
                visited[target as usize] = true;
                pending.push(target);
            }
        }
    }
    match visited.iter().position(|&seen| !seen) {
        Some(unreached) => Err(ParseError::new(
            "node is unreachable from the entry node",
            graph.nodes[unreached].offset,
        )),
        None => Ok(()),
    }
}
