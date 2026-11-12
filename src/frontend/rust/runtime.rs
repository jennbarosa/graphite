//! Runtime for generated graph modules: the data types, a `Run` that steps a graph, and the
//! debugger session behind `graphite debug`. Declare this file as `mod graphite;` in the crate
//! that uses the graphs. The stepping rules follow the TypeScript runtime, which is the spec.

#![allow(dead_code)]

use std::io::{self, BufRead, IsTerminal, Write};

/// A node's index in its graph. Node 0 is the entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NodeId(pub u32);

#[derive(Debug)]
pub struct Graph {
    pub name: &'static str,
    pub nodes: &'static [Node],
    /// Condition names, indexed by a condition's ID.
    pub conditions: &'static [&'static str],
    pub choices: &'static [Choice],
}

#[derive(Debug)]
pub struct Node {
    pub name: &'static str,
    /// Byte offset of the node in the source, for diagnostics.
    pub offset: usize,
    pub looping: bool,
    pub end: bool,
    /// The edge ID that runs once the node is finished, if the node has one.
    pub sequence: Option<u32>,
    pub flags: &'static [&'static str],
    pub annotations: &'static [Annotation],
    /// Outgoing edges, in source order.
    pub edges: &'static [Edge],
}

#[derive(Debug)]
pub struct Edge {
    /// The graph-wide edge ID.
    pub id: u32,
    pub target: NodeId,
    /// An index into the graph's `conditions`, or `None` for an unconditional edge.
    pub condition: Option<u32>,
    pub annotations: &'static [Annotation],
}

#[derive(Debug)]
pub struct Choice {
    pub name: &'static str,
    pub arms: &'static [Arm],
}

#[derive(Debug)]
pub struct Arm {
    /// An index into the graph's `conditions`; `None` is the else arm.
    pub condition: Option<u32>,
    pub tag: &'static str,
    pub annotations: &'static [Annotation],
}

#[derive(Debug)]
pub struct Annotation {
    pub key: &'static str,
    /// `None` for `@key`, with no value.
    pub value: Option<&'static str>,
}

impl Graph {
    /// The node called `name`, with its ID.
    pub fn node(&self, name: &str) -> Option<(NodeId, &Node)> {
        let index = self.nodes.iter().position(|node| node.name == name)?;
        Some((NodeId(index as u32), &self.nodes[index]))
    }

    /// The node with `id`. IDs come from this graph's own constants, so the index is in range.
    pub fn get(&self, id: NodeId) -> &Node {
        &self.nodes[id.0 as usize]
    }
}

/// Why a run stopped with an error. The names match the TypeScript runtime's codes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorCode {
    Cycle,
    OutputFull,
    Limit,
}

impl ErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            ErrorCode::Cycle => "cycle",
            ErrorCode::OutputFull => "output_full",
            ErrorCode::Limit => "limit",
        }
    }
}

/// The outcome of one `Run::step`.
#[derive(Debug)]
pub struct Step {
    pub node: &'static str,
    /// Transitions taken by the run since it started.
    pub transitions: usize,
    /// Transitions applied by this call.
    pub taken: usize,
    pub status: &'static str,
    pub error: Option<ErrorCode>,
}

/// One execution of a graph. Gates are read in source order, then the sequence edge runs only
/// once `finish` has been called for the current node. Conditions persist until `set` changes
/// them. Ended and errored runs take no further transitions.
pub struct Run {
    graph: &'static Graph,
    bits: Vec<bool>,
    node: usize,
    ended: bool,
    failure: Option<ErrorCode>,
    finished: bool,
    transitions: usize,
    max_transitions: usize,
    cycle: Cycle,
}

impl Run {
    pub fn start(graph: &'static Graph) -> Self {
        Self::with_limit(graph, usize::MAX)
    }

    /// A run that errors with `Limit` before taking transition number `max_transitions + 1`.
    pub fn with_limit(graph: &'static Graph, max_transitions: usize) -> Self {
        Run {
            graph,
            bits: vec![false; graph.conditions.len()],
            node: 0,
            ended: graph.nodes[0].end,
            failure: None,
            finished: false,
            transitions: 0,
            max_transitions,
            cycle: Cycle::default(),
        }
    }

    pub fn node(&self) -> &'static str {
        self.graph.nodes[self.node].name
    }

    /// `error` when the run failed, else `ended` or `running`.
    pub fn status(&self) -> &'static str {
        if self.failure.is_some() {
            "error"
        } else if self.ended {
            "ended"
        } else {
            "running"
        }
    }

    pub fn error(&self) -> Option<ErrorCode> {
        self.failure
    }

    pub fn transitions(&self) -> usize {
        self.transitions
    }

    pub fn set(&mut self, condition: &str, value: bool) -> Result<(), String> {
        let id = self.condition_id(condition)?;
        self.bits[id] = value;
        Ok(())
    }

    pub fn holds(&self, condition: &str) -> Result<bool, String> {
        Ok(self.bits[self.condition_id(condition)?])
    }

    /// Marks the current stage finished, which enables its sequence edge. No effect once the run
    /// has ended or errored.
    pub fn finish(&mut self) {
        if self.is_running() {
            self.finished = true;
        }
    }

    /// The first arm in source order whose condition holds, else the else arm. `None` when no
    /// arm applies.
    pub fn choose(&self, choice: &str) -> Result<Option<&'static Arm>, String> {
        let graph = self.graph;
        let choice = graph
            .choices
            .iter()
            .find(|candidate| candidate.name == choice)
            .ok_or_else(|| format!("unknown choice \"{choice}\" in graph \"{}\"", graph.name))?;
        Ok(choice
            .arms
            .iter()
            .find(|arm| arm.condition.is_none_or(|id| self.bits[id as usize])))
    }

    /// Takes transitions until none applies. With `trace`, each transition's edge ID is written
    /// to it, and a full trace errors with `OutputFull` before the transition that does not fit.
    pub fn step(&mut self, mut trace: Option<&mut [u32]>) -> Step {
        let mut taken = 0;
        if self.is_running() {
            self.cycle.reset();
            while let Some(edge) = self.next_edge() {
                if let Some(trace) = trace.as_deref_mut() {
                    if taken >= trace.len() {
                        self.failure = Some(ErrorCode::OutputFull);
                        break;
                    }
                    trace[taken] = edge.id;
                }
                taken += 1;
                self.commit(edge);
                if self.ended {
                    break;
                }
                if self.cycle.visit(edge.target.0) {
                    self.failure = Some(ErrorCode::Cycle);
                    break;
                }
            }
        }
        Step {
            node: self.node(),
            transitions: self.transitions,
            taken,
            status: self.status(),
            error: self.failure,
        }
    }

    /// Takes at most one transition and returns its edge ID, or `None` when none applies. Unlike
    /// `step`, it stops on the node it reaches.
    pub fn advance(&mut self) -> Option<u32> {
        if !self.is_running() {
            return None;
        }
        let edge = self.next_edge()?;
        self.commit(edge);
        Some(edge.id)
    }

    /// The transition to take from the current node, or `None`. Sets `failure` at the limit.
    fn next_edge(&mut self) -> Option<&'static Edge> {
        let graph = self.graph;
        let node = &graph.nodes[self.node];
        let bits = &self.bits;
        let gated = node
            .edges
            .iter()
            .find(|edge| edge.condition.is_some_and(|id| bits[id as usize]));
        let edge = gated.or_else(|| {
            let sequence = node.sequence?;
            if !self.finished {
                return None;
            }
            node.edges.iter().find(|edge| edge.id == sequence)
        })?;
        if self.transitions >= self.max_transitions {
            self.failure = Some(ErrorCode::Limit);
            return None;
        }
        Some(edge)
    }

    /// Moves to the edge's target and clears the stage's completion.
    fn commit(&mut self, edge: &Edge) {
        self.transitions += 1;
        self.node = edge.target.0 as usize;
        self.finished = false;
        if self.graph.nodes[self.node].end {
            self.ended = true;
        }
    }

    fn is_running(&self) -> bool {
        !self.ended && self.failure.is_none()
    }

    fn condition_id(&self, condition: &str) -> Result<usize, String> {
        self.graph
            .conditions
            .iter()
            .position(|name| *name == condition)
            .ok_or_else(|| {
                format!(
                    "unknown condition \"{condition}\" in graph \"{}\"",
                    self.graph.name
                )
            })
    }
}

/// Brent's cycle detection over the nodes a run enters, as the TypeScript runtime does it.
#[derive(Default)]
struct Cycle {
    checkpoint: u32,
    window: u32,
    steps: u32,
    visits: u32,
}

impl Cycle {
    fn reset(&mut self) {
        *self = Cycle {
            checkpoint: 0,
            window: 1,
            steps: 0,
            visits: 0,
        };
    }

    /// Returns true when the walk returns to the current checkpoint, which means it cycles.
    fn visit(&mut self, node: u32) -> bool {
        self.visits += 1;
        if self.visits == 1 {
            self.checkpoint = node;
            return false;
        }
        self.steps += 1;
        if node == self.checkpoint {
            return true;
        }
        if self.steps == self.window {
            self.checkpoint = node;
            self.window *= 2;
            self.steps = 0;
        }
        false
    }
}

/// Edge IDs one `step` may record. A longer settle reports `output_full`.
const TRACE_CAPACITY: usize = 1024;

const HELP: &str = "commands:
  show                 node, status, transitions, true conditions
  set <cond> on|off    set a condition's value
  finish               mark the current stage finished
  step                 settle the graph once; prints the edges taken
  choose <choice>      show the arm the choice selects now
  history              list every edge taken since start or reset
  reset                start a new run; conditions go back to off
  help                 this list
  quit                 leave";

/// The debugger session: one command per line, printed the way `graphite debug` prints them.
/// Commands are the words of a line. With arguments they run in order; without, they are read
/// from standard input.
pub fn debug(graph: &'static Graph) -> io::Result<()> {
    let mut session = Session::new(graph);
    let commands: Vec<String> = std::env::args().skip(1).collect();
    if !commands.is_empty() {
        for command in &commands {
            let Some(output) = session.execute(command) else {
                break;
            };
            println!("{output}");
        }
        return Ok(());
    }
    let prompt = if io::stdin().is_terminal() {
        "graphite> "
    } else {
        ""
    };
    let mut stdout = io::stdout();
    write!(stdout, "{prompt}")?;
    stdout.flush()?;
    for line in io::stdin().lock().lines() {
        let Some(output) = session.execute(&line?) else {
            break;
        };
        if !output.is_empty() {
            println!("{output}");
        }
        write!(stdout, "{prompt}")?;
        stdout.flush()?;
    }
    Ok(())
}

struct Session {
    graph: &'static Graph,
    run: Run,
    edges: Vec<u32>,
    trace: Vec<u32>,
}

impl Session {
    fn new(graph: &'static Graph) -> Self {
        Session {
            graph,
            run: Run::start(graph),
            edges: Vec::new(),
            trace: vec![0; TRACE_CAPACITY],
        }
    }

    /// Runs one command line. Returns the text to print, or `None` to quit.
    fn execute(&mut self, line: &str) -> Option<String> {
        let mut words = line.split_whitespace();
        let command = words.next().unwrap_or("");
        let args: Vec<&str> = words.collect();
        let output = match command {
            "" => String::new(),
            "help" => HELP.to_string(),
            "quit" | "exit" => return None,
            "show" => self.show(),
            "set" => self.set(&args),
            "finish" => {
                self.run.finish();
                self.show()
            }
            "step" => self.step(),
            "choose" => self.choose(&args),
            "history" => self.history(),
            "reset" => {
                self.run = Run::start(self.graph);
                self.edges.clear();
                format!("reset: {}", self.show())
            }
            other => format!("unknown command \"{other}\"; try help"),
        };
        Some(output)
    }

    fn show(&self) -> String {
        let truths: Vec<&str> = self
            .graph
            .conditions
            .iter()
            .copied()
            .filter(|name| self.run.holds(name).unwrap_or(false))
            .collect();
        let error = self
            .run
            .error()
            .map_or(String::new(), |code| format!(" ({})", code.as_str()));
        let list = if truths.is_empty() {
            "none".to_string()
        } else {
            truths.join(", ")
        };
        format!(
            "node {}  status {}{}  transitions {}  true: {}",
            self.run.node(),
            self.run.status(),
            error,
            self.run.transitions(),
            list
        )
    }

    fn set(&mut self, args: &[&str]) -> String {
        const USAGE: &str = "usage: set <condition> on|off";
        let [name, value] = args else {
            return USAGE.to_string();
        };
        let on = match *value {
            "on" => true,
            "off" => false,
            _ => return USAGE.to_string(),
        };
        match self.run.set(name, on) {
            Ok(()) => format!("{name} {value}"),
            Err(message) => format!("error: {message}"),
        }
    }

    fn step(&mut self) -> String {
        let result = self.run.step(Some(self.trace.as_mut_slice()));
        let taken = &self.trace[..result.taken];
        self.edges.extend_from_slice(taken);
        let edges = if taken.is_empty() {
            "no edges taken".to_string()
        } else {
            format!("edges {}", join_ids(taken))
        };
        format!("{}\n{edges}", self.show())
    }

    fn choose(&self, args: &[&str]) -> String {
        const USAGE: &str = "usage: choose <choice>";
        let [name] = args else {
            return USAGE.to_string();
        };
        match self.run.choose(name) {
            Err(message) => format!("error: {message}"),
            Ok(None) => format!("{name}: no arm selected"),
            Ok(Some(arm)) => {
                let label = match arm.condition {
                    None => "else".to_string(),
                    Some(id) => format!("when {}", self.graph.conditions[id as usize]),
                };
                format!(
                    "{name}: {label} -> {}{}",
                    arm.tag,
                    annotation_text(arm.annotations)
                )
            }
        }
    }

    fn history(&self) -> String {
        if self.edges.is_empty() {
            "no edges taken".to_string()
        } else {
            join_ids(&self.edges)
        }
    }
}

fn join_ids(ids: &[u32]) -> String {
    ids.iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(" ")
}

fn annotation_text(annotations: &[Annotation]) -> String {
    annotations
        .iter()
        .map(|entry| match entry.value {
            None => format!(" @{}", entry.key),
            Some(value) => format!(" @{}:{value}", entry.key),
        })
        .collect()
}
