//! The structure checks on a finished graph, run as a stage graph from `structure.graphite`: every node
//! has an exit, the adjacency arrays are built, and every node is reachable from the entry.

use std::sync::OnceLock;

use crate::bootstrap;
use crate::graph::{CompiledGraph, index};
use crate::parser::ParseError;
use crate::stage::Stepper;

/// The tasks of `structure.graphite`, one per task node.
#[derive(Clone, Copy)]
enum StructureTask {
    Degrees,
    Offsets,
    Reach,
}

impl StructureTask {
    fn named(name: &str) -> Option<StructureTask> {
        Some(match name {
            "degrees" => StructureTask::Degrees,
            "offsets" => StructureTask::Offsets,
            "reach" => StructureTask::Reach,
            _ => return None,
        })
    }
}

const OK: &[&str] = &["ok"];
const FAILED: &[&str] = &["error"];

/// The `graph structure` description, rebuilt from its bootstrap form.
fn structure_graph() -> &'static CompiledGraph {
    static GRAPH: OnceLock<CompiledGraph> = OnceLock::new();
    GRAPH.get_or_init(bootstrap::structure::graph)
}

/// Checks a finished graph by running the structure graph over it.
pub(super) fn finish(graph: &mut CompiledGraph) -> Result<(), ParseError> {
    let mut work = Work { graph, error: None };
    let mut stepper = Stepper::new(structure_graph(), StructureTask::named)
        .unwrap_or_else(|error| panic!("the bundled structure.graphite is wrong: {error}"));
    loop {
        match stepper.step(&mut |task| work.run(task)) {
            Ok(None) => {}
            Ok(Some("failed")) => {
                return Err(work.error.take().unwrap_or_else(|| {
                    ParseError::new("the structure checks stopped without an error", 0)
                }));
            }
            Ok(Some(_)) => return Ok(()),
            Err(message) => panic!("the structure graph stopped: {message}"),
        }
    }
}

/// The graph under check, and the first error any of its checks reports.
struct Work<'g> {
    graph: &'g mut CompiledGraph,
    error: Option<ParseError>,
}

impl Work<'_> {
    fn run(&mut self, task: StructureTask) -> &'static [&'static str] {
        let result = match task {
            StructureTask::Degrees => degrees(self.graph),
            StructureTask::Offsets => offsets(self.graph),
            StructureTask::Reach => reachability(self.graph),
        };
        match result {
            Ok(()) => OK,
            Err(error) => {
                self.error = Some(error);
                FAILED
            }
        }
    }
}

/// Every node needs an outgoing edge or the `end` flag. Each node's degree becomes its start
/// position in the adjacency arrays, and the running total is the next node's start.
fn degrees(graph: &mut CompiledGraph) -> Result<(), ParseError> {
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
    Ok(())
}

/// Fills the adjacency arrays: each node's edge IDs, in source order, in its range.
fn offsets(graph: &mut CompiledGraph) -> Result<(), ParseError> {
    graph.outgoing.resize(graph.edges.len(), 0);
    for (id, edge) in graph.edges.iter().enumerate() {
        let node = &mut graph.nodes[edge.source as usize];
        graph.outgoing[node.outgoing.end as usize] = id as u32;
        node.outgoing.end += 1;
    }
    Ok(())
}

/// Walks every edge, ignoring conditions, from the entry node. A node the walk misses is dead
/// code, so it is a build error. A graph with no reachable `end` is allowed: loops run on purpose.
/// Two pre-sized buffers, so the allocation count does not grow with the graph.
fn reachability(graph: &CompiledGraph) -> Result<(), ParseError> {
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
