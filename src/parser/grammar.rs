//! The parser as a stage graph. `grammar.graphite` holds the states of the grammar and their edges.
//! Each task here reads the tokens of its state and calls the same builder methods the grammar
//! always used, so the graph decides what comes next and the builder decides what a line means.

use std::collections::HashSet;
use std::sync::OnceLock;

use crate::bootstrap;
use crate::bootstrap::grammar::{Task, task};
use crate::graph::CompiledGraph;
use crate::interner::StringId;
use crate::lexer::{Lexer, TokenKind};
use crate::parser::compiler::{Builder, CHOOSE_ARM, GRAPH_FORM};
use crate::parser::tokens::Tokens;
use crate::parser::{Capacity, GRAPH_KEYWORD, ParseError, Schema, structure};
use crate::stage::Stepper;

/// The `graph grammar` description, rebuilt from its bootstrap form.
fn grammar_graph() -> &'static CompiledGraph {
    static GRAPH: OnceLock<CompiledGraph> = OnceLock::new();
    GRAPH.get_or_init(bootstrap::grammar::graph)
}

/// Parses `source` by running the grammar graph over its tokens.
pub(crate) fn parse(
    source: &str,
    schema: &Schema<'_>,
    capacities: &[Capacity],
) -> Result<Vec<CompiledGraph>, ParseError> {
    let flags: HashSet<&str> = schema.node_flags.iter().copied().collect();
    let mut session = Session {
        tokens: Tokens::new(source, Lexer::new(source)),
        flags: &flags,
        capacities,
        names: HashSet::new(),
        graphs: crate::parser::capacity::buffer(capacities.len())?,
        builder: None,
        graph_offset: 0,
        linking: false,
        keyword_offset: 0,
        choice: None,
        fallback: false,
        pending_condition: None,
        error: None,
    };
    let mut stepper = Stepper::new(grammar_graph(), task)
        .unwrap_or_else(|error| panic!("the bundled grammar.graphite is wrong: {error}"));
    loop {
        match stepper.step(&mut |task| session.run(task)) {
            Ok(None) => {}
            Ok(Some("failed")) => {
                return Err(session.error.take().unwrap_or_else(|| {
                    ParseError::new("the grammar stopped without an error", 0)
                }));
            }
            Ok(Some(_)) => return Ok(session.graphs),
            Err(message) => panic!("the grammar graph stopped: {message}"),
        }
    }
}

const OK: &[&str] = &["ok"];
const FAILED: &[&str] = &["error"];

/// What the grammar's tasks share: the tokens, the graph being built, and what the last state read.
struct Session<'a, 's> {
    tokens: Tokens<'a>,
    flags: &'s HashSet<&'s str>,
    capacities: &'s [Capacity],
    names: HashSet<&'a str>,
    graphs: Vec<CompiledGraph>,
    builder: Option<Builder<'s>>,
    graph_offset: usize,
    /// Set once a graph has an edge or choose line, after which nodes may not be declared.
    linking: bool,
    /// Where the keyword of the line just read starts.
    keyword_offset: usize,
    /// The choose block being read: its name, the arms it starts at, and its offset.
    choice: Option<(StringId, usize, usize)>,
    fallback: bool,
    /// The condition of the `when` arm being read, or `None` for an `else` arm.
    pending_condition: Option<u32>,
    error: Option<ParseError>,
}

impl<'a, 's> Session<'a, 's> {
    fn run(&mut self, task: Task) -> &'static [&'static str] {
        match task {
            Task::Next => self.next(),
            Task::Header => self.header(),
            Task::Body => self.body(),
            Task::NodeLine => self.node_line(),
            Task::EdgeLine => self.edge_line(),
            Task::Choose => self.choose(),
            Task::Arms => self.arms(),
            Task::WhenArm => self.when_arm(),
            Task::ElseArm => self.else_arm(),
            Task::Arm => self.arm(),
            Task::ArmsEnd => self.arms_end(),
            Task::BlockEnd => self.block_end(),
            Task::GraphEnd => self.graph_end(),
        }
    }

    /// Records `error` and routes the run to `failed`.
    fn fail(&mut self, error: ParseError) -> &'static [&'static str] {
        self.error = Some(error);
        FAILED
    }

    fn builder(&mut self) -> &mut Builder<'s> {
        self.builder
            .as_mut()
            .expect("a graph body is read only inside a graph")
    }

    /// Another graph if there are tokens left, else the end.
    fn next(&mut self) -> &'static [&'static str] {
        match self.tokens.peek() {
            Ok(Some(_)) => &["more"],
            Ok(None) if self.graphs.is_empty() => {
                let error = self
                    .tokens
                    .error("expected a `graph` block; the source declares no graphs");
                self.fail(error)
            }
            Ok(None) => &["finished"],
            Err(error) => self.fail(error),
        }
    }

    /// `graph NAME`, its newline, and the indented body.
    fn header(&mut self) -> &'static [&'static str] {
        let tokens = &mut self.tokens;
        let (name, offset) = match (|| {
            tokens.expect(TokenKind::Word(GRAPH_KEYWORD))?;
            tokens.name("expected a graph name after `graph`")
        })() {
            Ok(found) => found,
            Err(error) => return self.fail(error),
        };
        let capacity = match self.capacities.get(self.graphs.len()) {
            Some(capacity) => *capacity,
            None => {
                return self.fail(ParseError::new(
                    "no capacity hint for this graph; pass one per graph",
                    offset,
                ));
            }
        };
        if !self.names.insert(name) {
            return self.fail(ParseError::new(
                "graph name is already used by an earlier graph in this source",
                offset,
            ));
        }
        let opened = self
            .tokens
            .expect(TokenKind::Newline)
            .and_then(|()| self.tokens.expect(TokenKind::Indent));
        if let Err(error) = opened {
            return self.fail(error);
        }
        match Builder::new(capacity, name, &mut self.tokens, self.flags) {
            Ok(mut builder) => {
                builder.graph.offset = offset;
                self.builder = Some(builder);
                self.graph_offset = offset;
                self.linking = false;
                OK
            }
            Err(error) => self.fail(error),
        }
    }

    /// The next body line: a node, an edge, a choose, or the end of the body.
    fn body(&mut self) -> &'static [&'static str] {
        match self.tokens.at(TokenKind::Dedent) {
            Ok(true) => return &["line_end"],
            Ok(false) => {}
            Err(error) => return self.fail(error),
        }
        let (keyword, offset) = match self.tokens.word_or(GRAPH_FORM) {
            Ok(found) => found,
            Err(error) => return self.fail(error),
        };
        self.keyword_offset = offset;
        match keyword {
            "node" if !self.linking => &["line_node"],
            "edge" => {
                self.linking = true;
                &["line_edge"]
            }
            "choose" => {
                self.linking = true;
                &["line_choose"]
            }
            "node" => self.fail(ParseError::new(
                "nodes must be declared before edges and choices",
                offset,
            )),
            _ => self.fail(ParseError::new(GRAPH_FORM, offset)),
        }
    }

    fn node_line(&mut self) -> &'static [&'static str] {
        let tokens = &mut self.tokens;
        let builder = self
            .builder
            .as_mut()
            .expect("a node is read inside a graph");
        match builder.node(tokens) {
            Ok(()) => OK,
            Err(error) => self.fail(error),
        }
    }

    fn edge_line(&mut self) -> &'static [&'static str] {
        let tokens = &mut self.tokens;
        let builder = self
            .builder
            .as_mut()
            .expect("an edge is read inside a graph");
        match builder.edge(tokens) {
            Ok(()) => OK,
            Err(error) => self.fail(error),
        }
    }

    /// The head of a choose block. Its arms follow in the `arms` state.
    fn choose(&mut self) -> &'static [&'static str] {
        let tokens = &mut self.tokens;
        let builder = self
            .builder
            .as_mut()
            .expect("a choose is read inside a graph");
        match builder.choice_head(tokens) {
            Ok((name, offset)) => {
                self.choice = Some((name, self.builder().graph.arms.len(), offset));
                self.fallback = false;
                OK
            }
            Err(error) => self.fail(error),
        }
    }

    /// The next arm of a choose block: `when`, a final `else`, or the end of the block.
    fn arms(&mut self) -> &'static [&'static str] {
        match self.tokens.at(TokenKind::Dedent) {
            Ok(true) => return &["arm_end"],
            Ok(false) => {}
            Err(error) => return self.fail(error),
        }
        let (keyword, offset) = match self.tokens.word_or(CHOOSE_ARM) {
            Ok(found) => found,
            Err(error) => return self.fail(error),
        };
        self.keyword_offset = offset;
        let arms_so_far = self.builder().graph.arms.len();
        let start = self.choice.map_or(0, |(_, start, _)| start);
        match keyword {
            "when" if !self.fallback => &["arm_when"],
            "else" if !self.fallback && arms_so_far > start => {
                self.fallback = true;
                &["arm_else"]
            }
            _ => self.fail(ParseError::new(CHOOSE_ARM, offset)),
        }
    }

    fn when_arm(&mut self) -> &'static [&'static str] {
        let tokens = &mut self.tokens;
        let builder = self
            .builder
            .as_mut()
            .expect("an arm is read inside a graph");
        match builder.condition(tokens) {
            Ok(condition) => {
                self.pending_condition = Some(condition);
                OK
            }
            Err(error) => self.fail(error),
        }
    }

    fn else_arm(&mut self) -> &'static [&'static str] {
        self.pending_condition = None;
        OK
    }

    /// The arrow, tag and annotations of the arm being read.
    fn arm(&mut self) -> &'static [&'static str] {
        let tokens = &mut self.tokens;
        let builder = self
            .builder
            .as_mut()
            .expect("an arm is read inside a graph");
        let condition = self.pending_condition;
        let offset = self.keyword_offset;
        match builder.arm(tokens, condition, offset) {
            Ok(()) => OK,
            Err(error) => self.fail(error),
        }
    }

    fn arms_end(&mut self) -> &'static [&'static str] {
        let Some((name, start, offset)) = self.choice.take() else {
            return self.fail(ParseError::new("a choose block ended without starting", 0));
        };
        let tokens = &mut self.tokens;
        let builder = self
            .builder
            .as_mut()
            .expect("a choose is read inside a graph");
        match builder.choice_end(tokens, name, start, offset) {
            Ok(()) => OK,
            Err(error) => self.fail(error),
        }
    }

    fn block_end(&mut self) -> &'static [&'static str] {
        let tokens = &mut self.tokens;
        let builder = self.builder.as_mut().expect("a body ends inside a graph");
        match builder.body_end(tokens) {
            Ok(()) => OK,
            Err(error) => self.fail(error),
        }
    }

    /// Finishes the graph, checks its structure, and stores it.
    fn graph_end(&mut self) -> &'static [&'static str] {
        let Some(builder) = self.builder.take() else {
            return self.fail(ParseError::new("a graph ended without starting", 0));
        };
        let mut graph = builder.graph;
        let offset = self.graph_offset;
        let stored = structure::finish(&mut graph)
            .and_then(|()| crate::parser::capacity::push(&mut self.graphs, graph, offset));
        match stored {
            Ok(()) => OK,
            Err(error) => self.fail(error),
        }
    }
}
