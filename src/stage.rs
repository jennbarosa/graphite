//! Runs a stage graph. Each node with a `task` annotation calls its task, which returns the
//! conditions that hold once it has run. The first edge whose condition holds is taken, in source
//! order. The run stops at an end node and names it.

use std::collections::HashMap;

use crate::graph::{CompiledGraph, index};

/// Runs `graph` from its entry node. `lookup` turns each task name in the graph into the caller's
/// task type, so a graph naming a task the caller does not know is an error. `task` gets each task
/// node's task and returns the conditions that hold after it; every other condition is false for
/// that step.
///
/// Returns the end node's name. A node with no edge whose condition holds stalls the run, which is
/// an error that names the node. A task that returns a condition the graph does not declare is an
/// error too.
pub fn drive<T: Copy>(
    graph: &CompiledGraph,
    lookup: impl Fn(&str) -> Option<T>,
    mut task: impl FnMut(T) -> &'static [&'static str],
) -> Result<String, String> {
    let mut stepper = Stepper::new(graph, lookup)?;
    loop {
        if let Some(end) = stepper.step(&mut task)? {
            return Ok(end);
        }
    }
}

/// One node at a time. Each node's task and each condition name are resolved once, when the stepper
/// is made. A step then uses those resolved values; it does not scan the condition list.
pub struct Stepper<'g, T> {
    graph: &'g CompiledGraph,
    node: usize,
    holds: Vec<bool>,
    tasks: Vec<Option<T>>,
    condition_ids: HashMap<&'g str, usize>,
}

impl<'g, T: Copy> Stepper<'g, T> {
    /// Looks up every task in `graph`. Fails on a task name that `lookup` does not know.
    pub fn new(
        graph: &'g CompiledGraph,
        lookup: impl Fn(&str) -> Option<T>,
    ) -> Result<Self, String> {
        let tasks = (0..graph.nodes.len())
            .map(|node| match task_of(graph, node) {
                None => Ok(None),
                Some(name) => lookup(name)
                    .map(Some)
                    .ok_or_else(|| format!("the stage graph has no task named \"{name}\"")),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let condition_ids = graph
            .conditions
            .iter()
            .enumerate()
            .map(|(id, name)| (graph.text(*name), id))
            .collect();
        Ok(Stepper {
            graph,
            node: 0,
            holds: vec![false; graph.conditions.len()],
            tasks,
            condition_ids,
        })
    }

    /// Runs the current node's task, then moves along the first edge whose condition holds. Returns
    /// the end node's name when the step reaches an end node, and `None` when the run goes on.
    pub fn step(
        &mut self,
        task: &mut impl FnMut(T) -> &'static [&'static str],
    ) -> Result<Option<String>, String> {
        let graph = self.graph;
        let current = &graph.nodes[self.node];
        let name = graph.text(current.name);
        self.holds.fill(false);
        if let Some(node_task) = self.tasks[self.node] {
            for condition in task(node_task) {
                let id = *self.condition_ids.get(condition).ok_or_else(|| {
                    format!("the task at {name} reports the undeclared condition \"{condition}\"")
                })?;
                self.holds[id] = true;
            }
        }
        if current.end {
            return Ok(Some(name.to_owned()));
        }
        let holds = &self.holds;
        let next = graph.outgoing[index(&current.outgoing)]
            .iter()
            .map(|&id| &graph.edges[id as usize])
            .find(|edge| edge.condition.is_some_and(|id| holds[id as usize]));
        match next {
            Some(edge) => {
                self.node = edge.target as usize;
                Ok(None)
            }
            None => Err(format!("the stage graph stalls at {name}")),
        }
    }
}

/// The value of node `node`'s `task` annotation, if it has one.
pub(crate) fn task_of(graph: &CompiledGraph, node: usize) -> Option<&str> {
    graph.annotations[index(&graph.nodes[node].annotations)]
        .iter()
        .find(|entry| graph.text(entry.key) == "task")
        .and_then(|entry| entry.value)
        .map(|value| graph.text(value))
}
