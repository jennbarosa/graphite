//! The parser as a stage graph. `grammar.graphite` holds the states of the grammar and their edges.
//! Each task here reads the tokens of its state and calls the same builder methods the grammar
//! always used, so the graph decides what comes next and the builder decides what a line means.

use std::collections::HashSet;
use std::sync::OnceLock;

use crate::graph::CompiledGraph;
use crate::interner::StringId;
use crate::lexer::{Lexer, TokenKind};
use crate::parser::compiler::{Builder, CHOOSE_ARM, GRAPH_FORM};
use crate::parser::tokens::{TokenSource, Tokens};
use crate::parser::{Capacity, GRAPH_KEYWORD, ParseError, Schema, structure};
use crate::pipeline::block_capacities;
use crate::stage::Stepper;

const GRAMMAR: &str = include_str!("grammar.graphite");

/// The `graph grammar` description, parsed once by the seed parser. The graph parser cannot read
/// its own description before it exists, so the seed reads this one.
fn grammar_graph() -> &'static CompiledGraph {
    static GRAPH: OnceLock<CompiledGraph> = OnceLock::new();
    GRAPH.get_or_init(|| {
        let schema = Schema::default();
        let capacities = block_capacities(GRAMMAR, &schema);
        let mut graphs =
            crate::parser::parse_seed(GRAMMAR, &schema, &capacities, Box::new(Lexer::new(GRAMMAR)))
                .unwrap_or_else(|error| {
                    panic!("the bundled grammar.graphite does not parse: {error:?}")
                });
        graphs.remove(0)
    })
}

/// Parses `source` by running the grammar graph over its tokens.
pub(crate) fn parse<'a>(
    source: &'a str,
    schema: &Schema<'_>,
    capacities: &[Capacity],
    lexer: TokenSource<'a>,
) -> Result<Vec<CompiledGraph>, ParseError> {
    let flags: HashSet<&str> = schema.node_flags.iter().copied().collect();
    let mut session = Session {
        tokens: Tokens::new(source, lexer),
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
    let mut stepper = Stepper::new(grammar_graph(), GrammarTask::named)
        .unwrap_or_else(|error| panic!("the bundled grammar.graphite is wrong: {error}"));
    loop {
        match stepper.step(&mut |task| session.run(task)) {
            Ok(None) => {}
            Ok(Some(end)) if end == "failed" => {
                return Err(session.error.take().unwrap_or_else(|| {
                    ParseError::new("the grammar stopped without an error", 0)
                }));
            }
            Ok(Some(_)) => return Ok(session.graphs),
            Err(message) => panic!("the grammar graph stopped: {message}"),
        }
    }
}

/// The tasks of `grammar.graphite`, one per task node.
#[derive(Clone, Copy)]
enum GrammarTask {
    Next,
    Header,
    Body,
    NodeLine,
    EdgeLine,
    Choose,
    Arms,
    WhenArm,
    ElseArm,
    Arm,
    ArmsEnd,
    BlockEnd,
    GraphEnd,
}

impl GrammarTask {
    fn named(name: &str) -> Option<GrammarTask> {
        Some(match name {
            "next" => GrammarTask::Next,
            "header" => GrammarTask::Header,
            "body" => GrammarTask::Body,
            "node_line" => GrammarTask::NodeLine,
            "edge_line" => GrammarTask::EdgeLine,
            "choose" => GrammarTask::Choose,
            "arms" => GrammarTask::Arms,
            "when_arm" => GrammarTask::WhenArm,
            "else_arm" => GrammarTask::ElseArm,
            "arm" => GrammarTask::Arm,
            "arms_end" => GrammarTask::ArmsEnd,
            "block_end" => GrammarTask::BlockEnd,
            "graph_end" => GrammarTask::GraphEnd,
            _ => return None,
        })
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
    fn run(&mut self, task: GrammarTask) -> &'static [&'static str] {
        match task {
            GrammarTask::Next => self.next(),
            GrammarTask::Header => self.header(),
            GrammarTask::Body => self.body(),
            GrammarTask::NodeLine => self.node_line(),
            GrammarTask::EdgeLine => self.edge_line(),
            GrammarTask::Choose => self.choose(),
            GrammarTask::Arms => self.arms(),
            GrammarTask::WhenArm => self.when_arm(),
            GrammarTask::ElseArm => self.else_arm(),
            GrammarTask::Arm => self.arm(),
            GrammarTask::ArmsEnd => self.arms_end(),
            GrammarTask::BlockEnd => self.block_end(),
            GrammarTask::GraphEnd => self.graph_end(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontend::registry;
    use crate::lexer::Lexer;
    use crate::parser::parse_seed;
    use crate::pipeline::block_capacities;

    /// The generated Rust for each graph, or the error. Two parsers agree when these are equal.
    fn outcome(parsed: Result<Vec<CompiledGraph>, ParseError>) -> Result<Vec<u8>, ParseError> {
        let graphs = parsed?;
        let frontend = registry::lookup("rust").unwrap().frontend;
        let mut bytes = Vec::new();
        for graph in &graphs {
            for module in frontend.modules(graph) {
                frontend.emit_module(graph, module, &mut bytes).unwrap();
            }
        }
        Ok(bytes)
    }

    fn both(source: &str) -> (Result<Vec<u8>, ParseError>, Result<Vec<u8>, ParseError>) {
        let schema = Schema {
            node_flags: &["retry"],
        };
        let capacities = block_capacities(source, &schema);
        let seed = parse_seed(source, &schema, &capacities, Box::new(Lexer::new(source)));
        let graph = parse(
            source,
            &schema,
            &capacities,
            Box::new(crate::lexer::lex::GraphLexer::new(source)),
        );
        (outcome(seed), outcome(graph))
    }

    fn assert_agree(source: &str) {
        let (seed, graph) = both(source);
        assert_eq!(
            graph, seed,
            "graph parser differs from the seed on {source:?}"
        );
    }

    #[test]
    fn hand_written_sources_parse_the_same_either_way() {
        let cases = [
            "graph a\n  node b end\n",
            "graph a\n  node b\n  node c end\n  edge b -> c\n",
            "graph a\n  node b\n  edge b -> b when x else c\n",
            "graph a\n  node b end\n  node c end\n  choose k\n    when x -> t @f:y\n    else -> u\n",
            "graph a\n  node b end\n  choose k\n    else -> u\n",
            "graph a\n  node b end\n  choose k\n    when x -> t\n    else -> u\n    when y -> v\n",
            "graph a\n  node b end\n  choose k\n  node c\n",
            "graph a\n  node b end\n  node b end\n",
            "graph a\n  node b end\n  edge b -> c\n",
            "graph a\n  edge b -> c\n",
            "graph a\n  node b\n  node c end\n  edge b -> c\n  node d end\n",
            "graph a\n  node b @k @j:v\n  node c end\n  edge b -> c @x:y\n",
            "graph a\n  node b retry end\n",
            "graph a\n  node b bogus end\n",
            "graph a\n",
            "graph\n  node b end\n",
            "graph a\n  node b end\ngraph a\n  node c end\n",
            "graph a\n  node b end\ngraph c\n  node d end\n",
            "node floating end\n",
            "",
            "graph a\n  node b end\n  choose k\n    when x -> t\n  node z end\n",
            "graph a\n  node b end\n  choose k\n    when -> t\n",
            "graph a\n  node b end\n  choose k\n    when x y -> t\n",
            "graph a\n  node b end\n  choose k\n    when x -> t @\n",
            "graph a\n  node b\n    node c end\n",
        ];
        for case in cases {
            assert_agree(case);
        }
    }

    /// Random graph bodies from grammar fragments, indentation included, so the states are reached
    /// in many orders. Seeded, so a failure repeats.
    #[test]
    fn random_graph_bodies_parse_the_same_either_way() {
        const LINES: &[&str] = &[
            "node a end",
            "node b",
            "node c loop end",
            "node d retry",
            "node e @k:v",
            "node f @k.j",
            "node g bogus",
            "edge a -> b",
            "edge b -> c when x",
            "edge c -> a -> d when y else e",
            "edge a -> b @w:z",
            "edge a -> nowhere",
            "choose k",
            "when x -> t",
            "when y -> u @f:g",
            "else -> v",
            "when -> w",
            "@tag",
            "stray words here",
            "-> t",
        ];
        let mut state = 99u64;
        let mut next = |bound: usize| {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((state >> 33) % bound as u64) as usize
        };
        for _ in 0..1500 {
            let mut source = String::from("graph g\n");
            let mut depth = 1;
            for _ in 0..(1 + next(12)) {
                let indent = "  ".repeat(depth.max(1));
                let line = LINES[next(LINES.len())];
                source.push_str(&format!("{indent}{line}\n"));
                match next(5) {
                    0 if depth < 3 => depth += 1,
                    1 if depth > 1 => depth -= 1,
                    _ => {}
                }
            }
            assert_agree(&source);
        }
    }
}
