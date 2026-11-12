//! The structure checks on a finished graph, run as a stage graph from `structure.graphite`: every node
//! has an exit, the adjacency arrays are built, every node is reachable from the entry, and a graph
//! that runs durable tasks routes every outcome and can always finish.

use std::sync::OnceLock;

use crate::bootstrap;
use crate::bootstrap::structure::{Task, task};
use crate::graph::{CompiledGraph, Edge, index};
use crate::parser::ParseError;
use crate::stage::{Stepper, task_of};

const OK: &[&str] = &["passed"];
const FAILED: &[&str] = &["failed"];

/// The `graph structure` description, rebuilt from its bootstrap form.
fn structure_graph() -> &'static CompiledGraph {
    static GRAPH: OnceLock<CompiledGraph> = OnceLock::new();
    GRAPH.get_or_init(bootstrap::structure::graph)
}

/// Checks a finished graph by running the structure graph over it.
pub(super) fn finish(graph: &mut CompiledGraph) -> Result<(), ParseError> {
    let mut work = Work { graph, error: None };
    let mut stepper = Stepper::new(structure_graph(), task)
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
    fn run(&mut self, task: Task) -> &'static [&'static str] {
        let result = match task {
            Task::Degrees => degrees(self.graph),
            Task::Offsets => offsets(self.graph),
            Task::Reach => reachability(self.graph),
            Task::Routes => routes(self.graph),
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

/// The condition ID the graph uses for `name`, if it declares one.
fn condition(graph: &CompiledGraph, name: &str) -> Option<u32> {
    graph
        .conditions
        .iter()
        .position(|&id| graph.text(id) == name)
        .map(|position| position as u32)
}

fn out_edges(graph: &CompiledGraph, node: usize) -> impl Iterator<Item = &Edge> {
    graph.outgoing[index(&graph.nodes[node].outgoing)]
        .iter()
        .map(|&id| &graph.edges[id as usize])
}

/// Checks a graph that runs durable tasks. Such a graph has `passed` and `failed` conditions, and
/// optionally `errored`, and its tasks route on those alone. Any other graph is left alone. For such
/// a graph, each task needs an edge for every outcome the graph declares, and every node a run can
/// reach must still reach an end.
fn routes(graph: &CompiledGraph) -> Result<(), ParseError> {
    let (Some(passed), Some(failed)) = (condition(graph, "passed"), condition(graph, "failed"))
    else {
        return Ok(());
    };
    let mut outcomes = vec![passed, failed];
    outcomes.extend(condition(graph, "errored"));
    let is_task = |node: usize| task_of(graph, node).is_some();
    let durable = (0..graph.nodes.len()).all(|node| is_task(node) || graph.nodes[node].end)
        && graph.edges.iter().all(|edge| {
            !is_task(edge.source as usize)
                || edge.condition.is_some_and(|id| outcomes.contains(&id))
        });
    if !durable || !(0..graph.nodes.len()).any(is_task) {
        return Ok(());
    }
    const MISSING: &str =
        "a task needs an edge for each outcome the graph declares (passed, failed, errored)";
    // An end node's task ends the run, so its outcome routes nowhere.
    for node in (0..graph.nodes.len()).filter(|&node| is_task(node) && !graph.nodes[node].end) {
        for &outcome in &outcomes {
            if !out_edges(graph, node).any(|edge| edge.condition == Some(outcome)) {
                return Err(ParseError::new(MISSING, graph.nodes[node].offset));
            }
        }
    }
    // A node can finish if some route from it reaches an end. Walk the edges backwards from the ends.
    let mut predecessors = vec![Vec::new(); graph.nodes.len()];
    for edge in &graph.edges {
        predecessors[edge.target as usize].push(edge.source as usize);
    }
    let mut finishes: Vec<bool> = graph.nodes.iter().map(|node| node.end).collect();
    let mut pending: Vec<usize> = (0..graph.nodes.len())
        .filter(|&node| finishes[node])
        .collect();
    while let Some(node) = pending.pop() {
        for &before in &predecessors[node] {
            if !finishes[before] {
                finishes[before] = true;
                pending.push(before);
            }
        }
    }
    // Walk forwards from the entry. Every node a run can reach must be able to finish.
    let mut seen = vec![false; graph.nodes.len()];
    let mut stack = vec![0usize];
    seen[0] = true;
    while let Some(node) = stack.pop() {
        if !finishes[node] {
            return Err(ParseError::new(
                "node can never reach an end, so a run that gets here never finishes",
                graph.nodes[node].offset,
            ));
        }
        for edge in out_edges(graph, node) {
            let target = edge.target as usize;
            if !seen[target] {
                seen[target] = true;
                stack.push(target);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::parser::Schema;
    use crate::pipeline::compile;

    const MISSING: &str =
        "a task needs an edge for each outcome the graph declares (passed, failed, errored)";
    const NEVER_ENDS: &str = "node can never reach an end, so a run that gets here never finishes";

    fn check(source: &str) -> Result<(), &'static str> {
        compile(source, &Schema::default())
            .map(|_| ())
            .map_err(|error| error.message)
    }

    #[test]
    fn durable_graph_with_every_outcome_routed_passes() {
        let source = "graph flow\n  node start @task:work\n  node check @task:verify\n  node done end\n  node stop end\n  edge start -> check when passed\n  edge start -> stop when failed\n  edge check -> done when passed\n  edge check -> stop when failed\n";
        assert_eq!(check(source), Ok(()));
    }

    #[test]
    fn task_missing_an_outcome_edge_is_an_error() {
        let source = "graph flow\n  node start @task:work\n  node check @task:verify\n  node done end\n  node stop end\n  edge start -> check when passed\n  edge check -> done when passed\n  edge check -> stop when failed\n";
        assert_eq!(check(source), Err(MISSING));
    }

    #[test]
    fn loop_with_no_way_to_an_end_is_an_error() {
        let source = "graph flow\n  node a @task:work\n  node b @task:verify\n  edge a -> b when passed\n  edge a -> b when failed\n  edge b -> a when passed\n  edge b -> a when failed\n";
        assert_eq!(check(source), Err(NEVER_ENDS));
    }

    #[test]
    fn graph_without_outcome_conditions_is_not_checked() {
        let source = "graph flow\n  node a @task:work\n  node b end\n  edge a -> b when more\n  edge a -> b when finished\n";
        assert_eq!(check(source), Ok(()));
    }
}
