//! Compile grammar forms directly into preallocated graph arrays.

use std::collections::HashSet;
use std::num::NonZeroU32;
use std::ops::Range;

use crate::graph::{Annotation, Arm, Choice, CompiledGraph, Edge, Node, Span, narrow};
use crate::interner::StringId;
use crate::lexer::TokenKind;

use super::capacity::{buffer, push};
use super::tokens::{INVALID_NAME, Tokens, valid_name};

/// The line forms allowed inside a graph body, shared by the message and the fallback arm.
const GRAPH_FORM: &str = "expected `node`, `edge`, or `choose` line";
/// The arms allowed inside a choose block.
const CHOOSE_ARM: &str = "expected `when` or a final `else` arm";
const STRINGS_FULL: &str = "string capacity exhausted; raise the graph's `strings` hint";
const ANNOTATION_KEY: &str =
    "invalid annotation key: use letters, digits, `_`, or `-`, separated by `.`";
use super::{Capacity, ParseError};

pub(super) struct Builder<'a, 't, 's> {
    pub(super) graph: CompiledGraph,
    tokens: &'t mut Tokens<'a>,
    bindings: Vec<NameBindings>,
    flags: &'s HashSet<&'s str>,
}

/// Independent namespaces indexed by the interner's dense string IDs.
#[derive(Default)]
struct NameBindings {
    node: Option<NonZeroU32>,
    condition: Option<NonZeroU32>,
    choice: bool,
    annotation_key: bool,
}

impl<'a, 't, 's> Builder<'a, 't, 's> {
    pub(super) fn new(
        c: Capacity,
        name: &str,
        tokens: &'t mut Tokens<'a>,
        flags: &'s HashSet<&'s str>,
    ) -> Result<Self, ParseError> {
        let graph = c.allocate(name, tokens.offset())?;
        let mut bindings = buffer(c.strings)?;
        bindings.resize_with(c.strings, NameBindings::default);
        Ok(Self {
            graph,
            tokens,
            bindings,
            flags,
        })
    }

    fn intern(&mut self, word: &str) -> Result<StringId, ParseError> {
        self.graph
            .strings
            .intern(word)
            .map_err(|_| self.tokens.error(STRINGS_FULL))
    }

    pub(super) fn body(&mut self) -> Result<(), ParseError> {
        let mut linking = false;
        while !self.tokens.at(TokenKind::Dedent)? {
            let (keyword, offset) = self.tokens.word_or(GRAPH_FORM)?;
            match keyword {
                "node" if !linking => self.node()?,
                "edge" => {
                    linking = true;
                    self.edge()?;
                }
                "choose" => {
                    linking = true;
                    self.choice()?;
                }
                "node" => {
                    return Err(ParseError::new(
                        "nodes must be declared before edges and choices",
                        offset,
                    ));
                }
                _ => return Err(ParseError::new(GRAPH_FORM, offset)),
            }
        }
        if self.graph.nodes.is_empty() {
            return Err(self
                .tokens
                .error("graph needs at least one node; the first is its entry"));
        }
        self.tokens.expect(TokenKind::Dedent)
    }

    fn node(&mut self) -> Result<(), ParseError> {
        let (name, offset) = self.tokens.name("expected a node name after `node`")?;
        let id = self.graph.nodes.len() as u32;
        let name = self.intern(name)?;
        let binding = &mut self.bindings[name.index()].node;
        if binding.is_some() {
            return Err(ParseError::new(
                "node name is already declared in this graph",
                offset,
            ));
        }
        *binding = NonZeroU32::new(id + 1);
        let (looping, end, flags) = self.node_flags()?;
        let annotations = narrow(self.annotations()?);
        push(
            &mut self.graph.nodes,
            Node {
                name,
                looping,
                end,
                flags,
                annotations,
                outgoing: 0..0,
                sequence: None,
                offset,
            },
            offset,
        )?;
        Ok(())
    }

    fn node_reference(&mut self) -> Result<u32, ParseError> {
        let (name, offset) = self
            .tokens
            .word_or("expected a node name after `->` or `edge`")?;
        if let Some(id) = self
            .graph
            .strings
            .find(name)
            .and_then(|name| self.bindings[name.index()].node)
        {
            return Ok(id.get() - 1);
        }
        let message = if valid_name(name) {
            "unknown node; declare it with `node` before linking to it"
        } else {
            INVALID_NAME
        };
        Err(ParseError::new(message, offset))
    }

    fn condition(&mut self) -> Result<u32, ParseError> {
        let (word, offset) = self
            .tokens
            .word_or("expected a condition name after `when`")?;
        let name = self.intern(word)?;
        let binding = &mut self.bindings[name.index()];
        if let Some(id) = binding.condition {
            return Ok(id.get() - 1);
        }
        if binding.node.is_none() && !valid_name(word) {
            return Err(ParseError::new(INVALID_NAME, offset));
        }
        let id = self.graph.conditions.len() as u32;
        push(&mut self.graph.conditions, name, offset)?;
        binding.condition = NonZeroU32::new(id + 1);
        Ok(id)
    }

    /// Reads a node's flags up to the end of its line. Returns `(looping, end, domain-flag span)`.
    fn node_flags(&mut self) -> Result<(bool, bool, Span), ParseError> {
        let start = self.graph.flags.len() as u32;
        let mut looping = false;
        let mut end = false;
        while let Some(token) = self.tokens.peek()? {
            let TokenKind::Word(flag) = token.kind else {
                break;
            };
            self.tokens.word()?;
            match flag {
                "loop" => looping = true,
                "end" => end = true,
                _ if self.flags.contains(flag) => {
                    let flag = self.intern(flag)?;
                    push(&mut self.graph.flags, flag, token.offset)?;
                }
                _ => {
                    return Err(ParseError::new(
                        "unknown node flag; expected `loop`, `end`, or a flag from the schema",
                        token.offset,
                    ));
                }
            }
        }
        Ok((looping, end, start..self.graph.flags.len() as u32))
    }

    fn edge(&mut self) -> Result<(), ParseError> {
        let offset = self.tokens.offset();
        let source = self.node_reference()?;
        self.tokens.expect(TokenKind::Arrow)?;
        let target = self.node_reference()?;
        let condition = if self.tokens.at(TokenKind::Word("when"))? {
            self.tokens.expect(TokenKind::Word("when"))?;
            Some(self.condition()?)
        } else {
            None
        };
        let annotations = narrow(self.annotations()?);
        self.link(source, condition.is_none(), offset)?;
        push(
            &mut self.graph.edges,
            Edge {
                source,
                target,
                condition,
                annotations,
            },
            offset,
        )?;
        Ok(())
    }

    /// Checks that `source` may take one more edge, and records it in the node's count.
    /// `sequence` marks an unconditional edge, which is the node's single sequence edge.
    fn link(&mut self, source: u32, sequence: bool, offset: usize) -> Result<(), ParseError> {
        let node = &mut self.graph.nodes[source as usize];
        if node.end {
            return Err(ParseError::new(
                "an `end` node cannot have outgoing edges",
                offset,
            ));
        }
        if sequence {
            if node.sequence.is_some() {
                return Err(ParseError::new(
                    "a node may have one unconditional edge; give the others `when` conditions",
                    offset,
                ));
            }
            node.sequence = Some(self.graph.edges.len() as u32);
        }
        node.outgoing.end += 1;
        Ok(())
    }

    fn choice(&mut self) -> Result<(), ParseError> {
        let (name, offset) = self.tokens.name("expected a choose name after `choose`")?;
        let name = self.intern(name)?;
        let binding = &mut self.bindings[name.index()].choice;
        if *binding {
            return Err(ParseError::new(
                "choose name is already declared in this graph",
                offset,
            ));
        }
        *binding = true;
        self.tokens.expect(TokenKind::Newline)?;
        self.tokens.expect(TokenKind::Indent)?;
        let start = self.graph.arms.len();
        let mut fallback = false;
        while !self.tokens.at(TokenKind::Dedent)? {
            let (keyword, offset) = self.tokens.word_or(CHOOSE_ARM)?;
            let condition = match keyword {
                "when" if !fallback => Some(self.condition()?),
                "else" if !fallback && self.graph.arms.len() > start => {
                    fallback = true;
                    None
                }
                _ => return Err(ParseError::new(CHOOSE_ARM, offset)),
            };
            self.arm(condition, offset)?;
        }
        if self.graph.arms.len() == start {
            return Err(self.tokens.error("choose needs at least one `when` arm"));
        }
        self.tokens.expect(TokenKind::Dedent)?;
        push(
            &mut self.graph.choices,
            Choice {
                name,
                arms: narrow(start..self.graph.arms.len()),
            },
            offset,
        )?;
        Ok(())
    }

    /// Reads one arm's `-> tag` and annotations, then stores it.
    fn arm(&mut self, condition: Option<u32>, offset: usize) -> Result<(), ParseError> {
        self.tokens.expect(TokenKind::Arrow)?;
        let (tag, _) = self.tokens.word_or("expected a tag name after `->`")?;
        let tag = self.intern(tag)?;
        let annotations = narrow(self.annotations()?);
        push(
            &mut self.graph.arms,
            Arm {
                condition,
                tag,
                annotations,
            },
            offset,
        )
    }

    fn annotations(&mut self) -> Result<Range<usize>, ParseError> {
        let start = self.graph.annotations.len();
        while self.tokens.at(TokenKind::At)? {
            self.tokens.expect(TokenKind::At)?;
            let (key, offset) = self
                .tokens
                .word_or("expected an annotation key after `@`")?;
            let key = self.annotation_key(key, offset)?;
            let value = if self.tokens.at(TokenKind::Colon)? {
                self.tokens.expect(TokenKind::Colon)?;
                let (value, _) = self.tokens.word_or("expected a value after `:`")?;
                Some(self.intern(value)?)
            } else {
                None
            };
            push(
                &mut self.graph.annotations,
                Annotation { key, value },
                offset,
            )?;
        }
        self.tokens.expect(TokenKind::Newline)?;
        Ok(start..self.graph.annotations.len())
    }

    fn annotation_key(&mut self, key: &str, offset: usize) -> Result<StringId, ParseError> {
        // Preserve invalid-key diagnostics even when interner storage is exhausted.
        let id = self.intern(key).map_err(|error| {
            if key.split('.').all(valid_name) {
                error
            } else {
                ParseError::new(ANNOTATION_KEY, offset)
            }
        })?;
        let binding = &mut self.bindings[id.index()];
        if !binding.annotation_key {
            if !key.split('.').all(valid_name) {
                return Err(ParseError::new(ANNOTATION_KEY, offset));
            }
            binding.annotation_key = true;
        }
        Ok(id)
    }
}
