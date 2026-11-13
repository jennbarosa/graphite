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
pub(super) const GRAPH_FORM: &str = "expected `default`, `node`, `edge`, or `choose` line";
/// The arms allowed inside a choose block.
pub(super) const CHOOSE_ARM: &str = "expected `when` or a final `else` arm";
const STRINGS_FULL: &str = "string capacity exhausted; raise the graph's `strings` hint";
const ANNOTATION_KEY: &str =
    "invalid annotation key: use letters, digits, `_`, or `-`, separated by `.`";
use super::{Capacity, ParseError};

pub(super) struct Builder<'s> {
    pub(super) graph: CompiledGraph,
    bindings: Vec<NameBindings>,
    flags: &'s HashSet<&'s str>,
    /// Annotations from `default` lines, added to every node that does not set the same key.
    defaults: Vec<Annotation>,
}

/// Independent namespaces indexed by the interner's dense string IDs.
#[derive(Default)]
struct NameBindings {
    node: Option<NonZeroU32>,
    condition: Option<NonZeroU32>,
    choice: bool,
    annotation_key: bool,
}

impl<'s> Builder<'s> {
    pub(super) fn new<'a>(
        c: Capacity,
        name: &str,
        tokens: &mut Tokens<'a>,
        flags: &'s HashSet<&'s str>,
    ) -> Result<Self, ParseError> {
        let graph = c.allocate(name, tokens.offset())?;
        let mut bindings = buffer(c.strings)?;
        bindings.resize_with(c.strings, NameBindings::default);
        Ok(Self {
            graph,
            bindings,
            flags,
            defaults: Vec::new(),
        })
    }

    pub(super) fn intern<'a>(
        &mut self,
        tokens: &mut Tokens<'a>,
        word: &str,
    ) -> Result<StringId, ParseError> {
        self.graph
            .strings
            .intern(word)
            .map_err(|_| tokens.error(STRINGS_FULL))
    }

    pub(super) fn node<'a>(&mut self, tokens: &mut Tokens<'a>) -> Result<(), ParseError> {
        let (name, offset) = tokens.name("expected a node name after `node`")?;
        let id = self.graph.nodes.len() as u32;
        let name = self.intern(tokens, name)?;
        let binding = &mut self.bindings[name.index()].node;
        if binding.is_some() {
            return Err(ParseError::new(
                "node name is already declared in this graph",
                offset,
            ));
        }
        *binding = NonZeroU32::new(id + 1);
        let (looping, end, flags) = self.node_flags(tokens)?;
        let own = self.annotations(tokens)?;
        let annotations = narrow(self.with_defaults(own, offset)?);
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

    /// Appends each default whose key the node's own annotations in `own` do not set. Returns the
    /// node's whole span, which stays contiguous because nothing was pushed since `own`.
    fn with_defaults(
        &mut self,
        own: Range<usize>,
        offset: usize,
    ) -> Result<Range<usize>, ParseError> {
        for index in 0..self.defaults.len() {
            let default = self.defaults[index];
            let set = self.graph.annotations[own.clone()]
                .iter()
                .any(|annotation| annotation.key == default.key);
            if !set {
                push(&mut self.graph.annotations, default, offset)?;
            }
        }
        Ok(own.start..self.graph.annotations.len())
    }

    /// Finishes the `graph` line, which may carry the graph's own annotations, and opens the body.
    pub(super) fn opened<'a>(mut self, tokens: &mut Tokens<'a>) -> Result<Self, ParseError> {
        // `annotations` reads through the end of the line, so it also takes the newline.
        self.graph.graph_annotations = narrow(self.annotations(tokens)?);
        tokens.expect(TokenKind::Indent)?;
        Ok(self)
    }

    /// Reads `default @key:value ...`, after the `default` keyword. Each node declared later
    /// gets these annotations unless it sets the same key itself.
    pub(super) fn default<'a>(&mut self, tokens: &mut Tokens<'a>) -> Result<(), ParseError> {
        let offset = tokens.offset();
        let range = self.annotations(tokens)?;
        if range.is_empty() {
            return Err(ParseError::new(
                "expected an annotation after `default`",
                offset,
            ));
        }
        self.defaults
            .extend_from_slice(&self.graph.annotations[range.clone()]);
        // The node lines copy these in, so the line's own entries would only be unused.
        self.graph.annotations.truncate(range.start);
        Ok(())
    }

    pub(super) fn node_reference<'a>(
        &mut self,
        tokens: &mut Tokens<'a>,
    ) -> Result<u32, ParseError> {
        let (name, offset) = tokens.word_or("expected a node name after `->` or `edge`")?;
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

    pub(super) fn condition<'a>(&mut self, tokens: &mut Tokens<'a>) -> Result<u32, ParseError> {
        let (word, offset) = tokens.word_or("expected a condition name after `when`")?;
        let name = self.intern(tokens, word)?;
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
    pub(super) fn node_flags<'a>(
        &mut self,
        tokens: &mut Tokens<'a>,
    ) -> Result<(bool, bool, Span), ParseError> {
        let start = self.graph.flags.len() as u32;
        let mut looping = false;
        let mut end = false;
        while let Some(token) = tokens.peek()? {
            let TokenKind::Word(flag) = token.kind else {
                break;
            };
            tokens.word()?;
            match flag {
                "loop" => looping = true,
                "end" => end = true,
                _ if self.flags.contains(flag) => {
                    let flag = self.intern(tokens, flag)?;
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

    /// Reads `edge A -> B [-> C ...] [when COND] [else T] [@key:value ...]`.
    /// Each hop becomes one edge carrying the line's condition and annotations.
    /// `else T` also adds an unconditional edge from each hop's source to `T`.
    pub(super) fn edge<'a>(&mut self, tokens: &mut Tokens<'a>) -> Result<(), ParseError> {
        let offset = tokens.offset();
        let start = self.graph.edges.len();
        let mut source = self.node_reference(tokens)?;
        loop {
            tokens.expect(TokenKind::Arrow)?;
            let target = self.node_reference(tokens)?;
            push(
                &mut self.graph.edges,
                Edge {
                    source,
                    target,
                    condition: None,
                    annotations: 0..0,
                },
                offset,
            )?;
            source = target;
            if !tokens.at(TokenKind::Arrow)? {
                break;
            }
        }
        let end = self.graph.edges.len();
        let condition = if tokens.at(TokenKind::Word("when"))? {
            tokens.expect(TokenKind::Word("when"))?;
            Some(self.condition(tokens)?)
        } else {
            None
        };
        let fallback = if tokens.at(TokenKind::Word("else"))? {
            tokens.expect(TokenKind::Word("else"))?;
            Some(self.node_reference(tokens)?)
        } else {
            None
        };
        let annotations = narrow(self.annotations(tokens)?);
        for index in start..end {
            let edge = &mut self.graph.edges[index];
            edge.condition = condition;
            edge.annotations = annotations.clone();
            let source = edge.source;
            self.link(source, index as u32, condition.is_none(), offset)?;
        }
        if let Some(target) = fallback {
            for index in start..end {
                let source = self.graph.edges[index].source;
                let id = self.graph.edges.len() as u32;
                self.link(source, id, true, offset)?;
                push(
                    &mut self.graph.edges,
                    Edge {
                        source,
                        target,
                        condition: None,
                        annotations: annotations.clone(),
                    },
                    offset,
                )?;
            }
        }
        Ok(())
    }

    /// Checks that `source` may take one more edge, and records it in the node's count.
    /// `sequence` marks an unconditional edge, which is the node's single sequence edge,
    /// stored as `edge`, the edge's index.
    fn link(
        &mut self,
        source: u32,
        edge: u32,
        sequence: bool,
        offset: usize,
    ) -> Result<(), ParseError> {
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
            node.sequence = Some(edge);
        }
        node.outgoing.end += 1;
        Ok(())
    }

    /// Reads `choose NAME` and its indented block's opening, after the `choose` keyword.
    pub(super) fn choice_head<'a>(
        &mut self,
        tokens: &mut Tokens<'a>,
    ) -> Result<(StringId, usize), ParseError> {
        let (name, offset) = tokens.name("expected a choose name after `choose`")?;
        let name = self.intern(tokens, name)?;
        let binding = &mut self.bindings[name.index()].choice;
        if *binding {
            return Err(ParseError::new(
                "choose name is already declared in this graph",
                offset,
            ));
        }
        *binding = true;
        tokens.expect(TokenKind::Newline)?;
        tokens.expect(TokenKind::Indent)?;
        Ok((name, offset))
    }

    /// Ends a choose block: it needs an arm, and its closing dedent. Stores the choice.
    pub(super) fn choice_end<'a>(
        &mut self,
        tokens: &mut Tokens<'a>,
        name: StringId,
        start: usize,
        offset: usize,
    ) -> Result<(), ParseError> {
        if self.graph.arms.len() == start {
            return Err(tokens.error("choose needs at least one `when` arm"));
        }
        tokens.expect(TokenKind::Dedent)?;
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

    /// Ends the graph body: it needs a node, and its closing dedent.
    pub(super) fn body_end<'a>(&mut self, tokens: &mut Tokens<'a>) -> Result<(), ParseError> {
        if self.graph.nodes.is_empty() {
            return Err(tokens.error("graph needs at least one node; the first is its entry"));
        }
        tokens.expect(TokenKind::Dedent)
    }

    /// Reads one arm's `-> tag` and annotations, then stores it.
    pub(super) fn arm<'a>(
        &mut self,
        tokens: &mut Tokens<'a>,
        condition: Option<u32>,
        offset: usize,
    ) -> Result<(), ParseError> {
        tokens.expect(TokenKind::Arrow)?;
        let (tag, _) = tokens.word_or("expected a tag name after `->`")?;
        let tag = self.intern(tokens, tag)?;
        let annotations = narrow(self.annotations(tokens)?);
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

    pub(super) fn annotations<'a>(
        &mut self,
        tokens: &mut Tokens<'a>,
    ) -> Result<Range<usize>, ParseError> {
        let start = self.graph.annotations.len();
        while tokens.at(TokenKind::At)? {
            tokens.expect(TokenKind::At)?;
            let (key, offset) = tokens.word_or("expected an annotation key after `@`")?;
            let key = self.annotation_key(tokens, key, offset)?;
            let value = if tokens.at(TokenKind::Colon)? {
                tokens.expect(TokenKind::Colon)?;
                let (value, _) = tokens.word_or("expected a value after `:`")?;
                Some(self.intern(tokens, value)?)
            } else {
                None
            };
            push(
                &mut self.graph.annotations,
                Annotation { key, value },
                offset,
            )?;
        }
        tokens.expect(TokenKind::Newline)?;
        Ok(start..self.graph.annotations.len())
    }

    pub(super) fn annotation_key<'a>(
        &mut self,
        tokens: &mut Tokens<'a>,
        key: &str,
        offset: usize,
    ) -> Result<StringId, ParseError> {
        // Preserve invalid-key diagnostics even when interner storage is exhausted.
        let id = self.intern(tokens, key).map_err(|error| {
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
