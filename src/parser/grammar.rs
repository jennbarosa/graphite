//! The grammar: reads graph blocks from the token stream and hands each line to the builder, which
//! decides what the line means. The order of the checks is the order of the grammar.

use std::collections::HashSet;

use crate::graph::CompiledGraph;
use crate::interner::StringId;
use crate::kind::{Kind, Line};
use crate::lexer::{Lexer, TokenKind};
use crate::parser::compiler::{Builder, CHOOSE_ARM, CHOOSE_NEEDS_WHEN, GRAPH_FORM};
use crate::parser::tokens::Tokens;
use crate::parser::{GRAPH_KEYWORD, ParseError, Schema, structure};

/// Parses `source` from byte `start`.
pub(crate) fn parse(
    source: &str,
    start: usize,
    schema: &Schema<'_>,
) -> Result<Vec<CompiledGraph>, ParseError> {
    let flags: HashSet<&str> = schema.node_flags.iter().copied().collect();
    let kind = if schema.pure {
        Kind::Pure
    } else {
        Kind::Machine
    };
    let mut session = Session {
        tokens: Tokens::new(source, Lexer::at(source, start)),
        flags: &flags,
        kind,
        names: HashSet::new(),
        graphs: Vec::new(),
        builder: None,
        graph_offset: 0,
        linking: false,
        keyword_offset: 0,
        choice: None,
        else_seen: false,
        pending_condition: None,
    };
    session.run()
}

/// A choose block that has started: its name, the index of its first arm, and its offset.
struct ChoiceHead {
    name: StringId,
    first_arm: usize,
    offset: usize,
}

/// What the grammar reads: the tokens, the graph being built, and what the last line read.
struct Session<'a, 's> {
    tokens: Tokens<'a>,
    flags: &'s HashSet<&'s str>,
    /// Whether the file is a machine or a pure file, which decides the lines it may hold.
    kind: Kind,
    names: HashSet<&'a str>,
    graphs: Vec<CompiledGraph>,
    builder: Option<Builder<'s>>,
    graph_offset: usize,
    /// Set once a graph has an edge or choose line, after which nodes may not be declared.
    linking: bool,
    /// Where the keyword of the line just read starts.
    keyword_offset: usize,
    /// The choose block being read, if any.
    choice: Option<ChoiceHead>,
    /// Set once the choose block being read has its `else` arm, after which only its end may follow.
    else_seen: bool,
    /// The condition of the `when` arm being read, or `None` for an `else` arm.
    pending_condition: Option<u32>,
}

impl<'a, 's> Session<'a, 's> {
    /// Reads every graph block in the source.
    fn run(&mut self) -> Result<Vec<CompiledGraph>, ParseError> {
        while self.more()? {
            self.header()?;
            self.graph_body()?;
        }
        Ok(std::mem::take(&mut self.graphs))
    }

    fn builder(&mut self) -> &mut Builder<'s> {
        self.builder
            .as_mut()
            .expect("a graph body is read only inside a graph")
    }

    /// Runs one builder call with the graph being built and the tokens.
    fn with_builder<T>(
        &mut self,
        read: impl FnOnce(&mut Builder<'s>, &mut Tokens<'a>) -> Result<T, ParseError>,
    ) -> Result<T, ParseError> {
        let builder = self
            .builder
            .as_mut()
            .expect("a graph body is read only inside a graph");
        read(builder, &mut self.tokens)
    }

    /// Whether another graph block follows. Without one, the source must have had a graph.
    fn more(&mut self) -> Result<bool, ParseError> {
        match self.tokens.peek()? {
            Some(_) => Ok(true),
            None if self.graphs.is_empty() => Err(self
                .tokens
                .error("expected a `graph` block; the source declares no graphs")),
            None => Ok(false),
        }
    }

    /// `graph NAME [@key:value ...]`, its newline, and the indented body.
    fn header(&mut self) -> Result<(), ParseError> {
        let tokens = &mut self.tokens;
        let (name, offset) = {
            tokens.expect(TokenKind::Word(GRAPH_KEYWORD))?;
            tokens.name("expected a graph name after `graph`")?
        };
        if !self.names.insert(name) {
            return Err(ParseError::new(
                "graph name is already used by an earlier graph in this source",
                offset,
            ));
        }
        let mut builder = Builder::new(name, &mut self.tokens, self.flags, self.kind)
            .and_then(|builder| builder.opened(&mut self.tokens))?;
        builder.graph.offset = offset;
        self.builder = Some(builder);
        self.graph_offset = offset;
        self.linking = false;
        Ok(())
    }

    /// Reads the lines of the open graph up to its end, then finishes it.
    fn graph_body(&mut self) -> Result<(), ParseError> {
        while let Some(line) = self.body()? {
            match line {
                Line::Default => self.with_builder(|b, t| b.default(t))?,
                Line::Node => self.with_builder(|b, t| b.node(t))?,
                Line::Edge => self.with_builder(|b, t| b.edge(t))?,
                Line::Choose => self.choose()?,
                Line::Enum => self.enum_line()?,
                Line::Input => self.with_builder(|b, t| b.input(t))?,
            }
        }
        self.with_builder(|b, t| b.body_end(t))?;
        self.graph_end()
    }

    /// The next body line, or `None` at the end of the body. The graph's kind decides which lines
    /// it may hold.
    fn body(&mut self) -> Result<Option<Line>, ParseError> {
        if self.tokens.at(TokenKind::Dedent)? {
            return Ok(None);
        }
        let (keyword, offset) = self.tokens.word_or(GRAPH_FORM)?;
        self.keyword_offset = offset;
        let line = match keyword {
            "default" => Line::Default,
            "node" => Line::Node,
            "edge" => Line::Edge,
            "choose" => Line::Choose,
            "enum" => Line::Enum,
            "input" => Line::Input,
            _ => return Err(ParseError::new(GRAPH_FORM, offset)),
        };
        if let Err(message) = self.kind.allows(line) {
            return Err(ParseError::new(message, offset));
        }
        match line {
            Line::Default
                if self.kind == Kind::Machine
                    && (self.linking || !self.builder().graph.nodes.is_empty()) =>
            {
                return Err(ParseError::new(
                    "`default` must come before the first node",
                    offset,
                ));
            }
            Line::Node if self.linking => {
                return Err(ParseError::new(
                    "nodes must be declared before edges and choices",
                    offset,
                ));
            }
            Line::Edge | Line::Choose => self.linking = true,
            _ => {}
        }
        Ok(Some(line))
    }

    /// `enum NAME MEMBER...` or `enum NAME`: one line, or a head whose members follow.
    fn enum_line(&mut self) -> Result<(), ParseError> {
        if !self.with_builder(|b, t| b.enum_head(t))? {
            return Ok(());
        }
        while !self.tokens.at(TokenKind::Dedent)? {
            self.with_builder(|b, t| b.member(t))?;
        }
        self.with_builder(|b, t| b.enum_end(t))
    }

    /// The head of a choose block, then its arms up to the end of the block.
    fn choose(&mut self) -> Result<(), ParseError> {
        let (name, offset) = self.with_builder(|b, t| b.choice_head(t))?;
        let first_arm = self.builder().graph.arms.len();
        self.choice = Some(ChoiceHead {
            name,
            first_arm,
            offset,
        });
        self.else_seen = false;
        self.arms()
    }

    /// The arms of a choose block: `when` arms, then at most one final `else`, then the block's end.
    fn arms(&mut self) -> Result<(), ParseError> {
        while !self.tokens.at(TokenKind::Dedent)? {
            let (keyword, offset) = self.tokens.word_or(CHOOSE_ARM)?;
            self.keyword_offset = offset;
            let arms_so_far = self.builder().graph.arms.len();
            let first_arm = self.choice.as_ref().map_or(0, |head| head.first_arm);
            match keyword {
                "when" if !self.else_seen => {
                    self.pending_condition = Some(self.with_builder(|b, t| b.when_condition(t))?);
                }
                "else" if !self.else_seen && arms_so_far > first_arm => {
                    self.else_seen = true;
                    self.pending_condition = None;
                }
                "else" if !self.else_seen => {
                    return Err(ParseError::new(CHOOSE_NEEDS_WHEN, offset));
                }
                _ => return Err(ParseError::new(CHOOSE_ARM, offset)),
            }
            let condition = self.pending_condition;
            self.with_builder(|b, t| b.arm(t, condition))?;
        }
        let ChoiceHead {
            name,
            first_arm,
            offset,
        } = self
            .choice
            .take()
            .ok_or_else(|| ParseError::new("a choose block ended without starting", 0))?;
        self.with_builder(|b, t| b.choice_end(t, name, first_arm, offset))
    }

    /// Finishes the graph, checks its structure, and stores it.
    fn graph_end(&mut self) -> Result<(), ParseError> {
        let Some(builder) = self.builder.take() else {
            return Err(ParseError::new("a graph ended without starting", 0));
        };
        let mut graph = builder.graph;
        structure::finish(&mut graph)?;
        self.graphs.push(graph);
        Ok(())
    }
}
