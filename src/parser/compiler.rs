//! Compile grammar forms directly into preallocated graph arrays.

use std::collections::{HashMap, HashSet};
use std::num::NonZeroU32;
use std::ops::Range;

use crate::graph::{
    Annotation, Arm, Choice, CompiledGraph, Edge, Enum, Input, Member, Node, Span, Test, Ty, narrow,
};
use crate::interner::{Interner, StringId};
use crate::kind::Kind;
use crate::lexer::TokenKind;

use super::ParseError;
use super::tokens::{INVALID_NAME, Tokens, valid_name};

/// The line forms allowed inside a graph body, shared by the message and the fallback arm.
pub(super) const GRAPH_FORM: &str = "expected `default`, `node`, `edge`, or `choose` line";
/// The arms allowed inside a choose block.
pub(super) const CHOOSE_ARM: &str = "expected `when` or a final `else` arm";
/// The refusal of a choose block with no `when` arm, whether it is empty or opens with `else`.
pub(super) const CHOOSE_NEEDS_WHEN: &str = "choose needs at least one `when` arm";
const STRINGS_FULL: &str = "a graph's strings exceed the 4 GiB offset limit";
const ANNOTATIONS_FULL: &str = "a graph's annotations exceed the 4 GiB offset limit";
const ENUM_DUPLICATE: &str = "enum name is already declared in this graph";
const MEMBER_DUPLICATE: &str = "member is already declared in this enum";
const INPUT_DUPLICATE: &str = "input name is already declared in this graph";
const ENUM_EMPTY: &str = "an enum needs at least one member";
const INPUT_TYPE: &str = "unknown type; expected `bool` or an enum declared above this input";
const INPUT_COLON: &str = "expected `:` and `bool` or an enum after the input name";
const CHOOSE_ENUM: &str = "a choose in a pure file needs `: ENUM` naming its enum";
const CHOOSE_UNKNOWN_ENUM: &str = "unknown enum; declare it with `enum` above this choose";
const WHEN_UNKNOWN_INPUT: &str = "unknown input; declare it with `input` above its `when`";
const WHEN_BOOL_MEMBER: &str = "`:MEMBER` tests an enum input; this input is a bool";
const WHEN_ENUM_MEMBER: &str = "an enum input needs `:MEMBER` after its name";
const WHEN_NOT_MEMBER: &str = "not a member of this enum";
const WHEN_REPEATED: &str = "this test is already an arm above, so this arm can never be chosen";
const TAG_NOT_MEMBER: &str = "the arm's tag must be a member of the choose's enum";
const ANNOTATION_KEY: &str =
    "invalid annotation key: use letters, digits, `_`, or `-`, separated by `.`";

pub(super) struct Builder<'s> {
    pub(super) graph: CompiledGraph,
    bindings: Vec<NameBindings>,
    flags: &'s HashSet<&'s str>,
    /// Annotations from `default` lines, added to every node that does not set the same key
    /// (and, in a pure file, to every arm).
    defaults: Vec<Annotation>,
    kind: Kind,
    /// Position of each member within its enum, keyed by (enum index, name index). One lookup
    /// answers the duplicate check, the `when` test and the arm's tag check, so none scans a list.
    member_positions: HashMap<(u32, usize), u32>,
    /// The enum being declared, whose members the member lines add to.
    current_enum: Option<u32>,
    /// The enum and annotations of the choose block being read.
    choice_ty: Option<u32>,
    choice_annotations: Span,
    /// The condition ids that the `when` arms of the choose being read already use. A pure
    /// choose refuses a repeat, since the later arm could never be chosen.
    choice_tests: HashSet<u32>,
}

/// Independent namespaces indexed by the interner's dense string IDs.
#[derive(Default)]
struct NameBindings {
    node: Option<NonZeroU32>,
    condition: Option<NonZeroU32>,
    choice: bool,
    /// Index plus one into `graph.enums`.
    enum_: Option<NonZeroU32>,
    /// Index plus one into `graph.inputs`.
    input: Option<NonZeroU32>,
}

impl<'s> Builder<'s> {
    pub(super) fn new(
        name: &str,
        tokens: &mut Tokens<'_>,
        flags: &'s HashSet<&'s str>,
        kind: Kind,
    ) -> Result<Self, ParseError> {
        let mut strings = Interner::default();
        let name = strings
            .intern(name)
            .map_err(|_| tokens.error(STRINGS_FULL))?;
        let graph = CompiledGraph {
            name,
            offset: 0,
            kind,
            strings,
            nodes: Vec::new(),
            edges: Vec::new(),
            outgoing: Vec::new(),
            conditions: Vec::new(),
            tests: Vec::new(),
            choices: Vec::new(),
            arms: Vec::new(),
            enums: Vec::new(),
            members: Vec::new(),
            inputs: Vec::new(),
            annotations: Vec::new(),
            flags: Vec::new(),
            graph_annotations: 0..0,
        };
        Ok(Self {
            graph,
            bindings: Vec::new(),
            flags,
            defaults: Vec::new(),
            kind,
            member_positions: HashMap::new(),
            current_enum: None,
            choice_ty: None,
            choice_annotations: 0..0,
            choice_tests: HashSet::new(),
        })
    }

    /// The namespaces bound to `name`, growing the table to reach it.
    fn slot(&mut self, name: StringId) -> &mut NameBindings {
        let index = name.index();
        if self.bindings.len() <= index {
            self.bindings.resize_with(index + 1, NameBindings::default);
        }
        &mut self.bindings[index]
    }

    /// The namespaces bound to `name`, if it has ever been bound.
    fn bound(&self, name: StringId) -> Option<&NameBindings> {
        self.bindings.get(name.index())
    }

    pub(super) fn intern(
        &mut self,
        tokens: &mut Tokens<'_>,
        word: &str,
    ) -> Result<StringId, ParseError> {
        self.graph
            .strings
            .intern(word)
            .map_err(|_| tokens.error(STRINGS_FULL))
    }

    pub(super) fn node(&mut self, tokens: &mut Tokens<'_>) -> Result<(), ParseError> {
        let (name, offset) = tokens.name("expected a node name after `node`")?;
        let id = self.graph.nodes.len() as u32;
        let name = self.intern(tokens, name)?;
        let binding = &mut self.slot(name).node;
        if binding.is_some() {
            return Err(ParseError::new(
                "node name is already declared in this graph",
                offset,
            ));
        }
        *binding = NonZeroU32::new(id + 1);
        let (looping, end, flags) = self.node_flags(tokens)?;
        let own = self.annotations(tokens)?;
        let annotations = narrow(self.with_defaults(tokens, own)?);
        self.graph.nodes.push(Node {
            name,
            looping,
            end,
            flags,
            annotations,
            outgoing: 0..0,
            sequence: None,
            offset,
        });
        Ok(())
    }

    /// Appends each default whose key the node's own annotations in `own` do not set. Returns the
    /// node's whole span, which stays contiguous because nothing was pushed since `own`.
    fn with_defaults(
        &mut self,
        tokens: &mut Tokens<'_>,
        own: Range<usize>,
    ) -> Result<Range<usize>, ParseError> {
        for index in 0..self.defaults.len() {
            let default = self.defaults[index];
            let set = self.graph.annotations[own.clone()]
                .iter()
                .any(|annotation| annotation.key == default.key);
            if !set {
                self.graph.annotations.push(default);
            }
        }
        if u32::try_from(self.graph.annotations.len()).is_err() {
            return Err(tokens.error(ANNOTATIONS_FULL));
        }
        Ok(own.start..self.graph.annotations.len())
    }

    /// Finishes the `graph` line, which may carry the graph's own annotations, and opens the body.
    pub(super) fn opened(mut self, tokens: &mut Tokens<'_>) -> Result<Self, ParseError> {
        // `annotations` reads through the end of the line, so it also takes the newline.
        self.graph.graph_annotations = narrow(self.annotations(tokens)?);
        tokens.expect(TokenKind::Indent)?;
        Ok(self)
    }

    /// Reads `default @key:value ...`, after the `default` keyword. Each node declared later
    /// gets these annotations unless it sets the same key itself.
    pub(super) fn default(&mut self, tokens: &mut Tokens<'_>) -> Result<(), ParseError> {
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

    pub(super) fn node_reference(&mut self, tokens: &mut Tokens<'_>) -> Result<u32, ParseError> {
        let (name, offset) = tokens.word_or("expected a node name after `->` or `edge`")?;
        if let Some(id) = self
            .graph
            .strings
            .find(name)
            .and_then(|name| self.bound(name)?.node)
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

    pub(super) fn condition(&mut self, tokens: &mut Tokens<'_>) -> Result<u32, ParseError> {
        let (word, offset) = tokens.word_or("expected a condition name after `when`")?;
        let name = self.intern(tokens, word)?;
        if let Some(id) = self.bound(name).and_then(|binding| binding.condition) {
            return Ok(id.get() - 1);
        }
        if self.bound(name).and_then(|binding| binding.node).is_none() && !valid_name(word) {
            return Err(ParseError::new(INVALID_NAME, offset));
        }
        let id = self.graph.conditions.len() as u32;
        self.graph.conditions.push(name);
        self.slot(name).condition = NonZeroU32::new(id + 1);
        Ok(id)
    }

    /// Reads a node's flags up to the end of its line. Returns `(looping, end, domain-flag span)`.
    pub(super) fn node_flags(
        &mut self,
        tokens: &mut Tokens<'_>,
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
                    self.graph.flags.push(flag);
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
    pub(super) fn edge(&mut self, tokens: &mut Tokens<'_>) -> Result<(), ParseError> {
        let offset = tokens.offset();
        let start = self.graph.edges.len();
        let mut source = self.node_reference(tokens)?;
        loop {
            tokens.expect(TokenKind::Arrow)?;
            let target = self.node_reference(tokens)?;
            self.graph.edges.push(Edge {
                source,
                target,
                condition: None,
                annotations: 0..0,
            });
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
                self.graph.edges.push(Edge {
                    source,
                    target,
                    condition: None,
                    annotations: annotations.clone(),
                });
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

    /// Reads `choose NAME` and its indented block's opening, after the `choose` keyword. A pure
    /// choose names its enum as `: ENUM`, and may carry annotations on the line.
    pub(super) fn choice_head(
        &mut self,
        tokens: &mut Tokens<'_>,
    ) -> Result<(StringId, usize), ParseError> {
        let (word, offset) = tokens.name("expected a choose name after `choose`")?;
        let name = self.intern(tokens, word)?;
        let binding = &mut self.slot(name).choice;
        if *binding {
            return Err(ParseError::new(
                "choose name is already declared in this graph",
                offset,
            ));
        }
        *binding = true;
        self.choice_tests.clear();
        if self.kind == Kind::Pure {
            if !tokens.at(TokenKind::Colon)? {
                return Err(ParseError::new(CHOOSE_ENUM, offset));
            }
            tokens.expect(TokenKind::Colon)?;
            let (ty, ty_offset) = tokens.word_or("expected an enum name after `:`")?;
            self.choice_ty = Some(
                self.enum_index(ty)
                    .ok_or(ParseError::new(CHOOSE_UNKNOWN_ENUM, ty_offset))?,
            );
            self.choice_annotations = narrow(self.annotations(tokens)?);
        } else {
            tokens.expect(TokenKind::Newline)?;
        }
        tokens.expect(TokenKind::Indent)?;
        Ok((name, offset))
    }

    /// The index in `graph.enums` of the enum named `word`, if one is declared.
    fn enum_index(&self, word: &str) -> Option<u32> {
        let id = self.graph.strings.find(word)?;
        self.bound(id)?.enum_.map(|index| index.get() - 1)
    }

    /// The index in `graph.inputs` of the input named `word`, if one is declared.
    fn input_index(&self, word: &str) -> Option<u32> {
        let id = self.graph.strings.find(word)?;
        self.bound(id)?.input.map(|index| index.get() - 1)
    }

    /// Reads `enum NAME MEMBER...` (one line) or `enum NAME` (a head whose members follow in an
    /// indented block). Returns `true` for the block form, whose members are read next.
    pub(super) fn enum_head<'a>(&mut self, tokens: &mut Tokens<'a>) -> Result<bool, ParseError> {
        let (word, offset) = tokens.name("expected an enum name after `enum`")?;
        let name = self.intern(tokens, word)?;
        if self
            .bound(name)
            .is_some_and(|binding| binding.enum_.is_some())
        {
            return Err(ParseError::new(ENUM_DUPLICATE, offset));
        }
        let id = self.graph.enums.len() as u32;
        self.slot(name).enum_ = NonZeroU32::new(id + 1);
        let start = self.graph.members.len();
        self.graph.enums.push(Enum {
            name,
            annotations: 0..0,
            members: narrow(start..start),
            offset,
        });
        self.current_enum = Some(id);
        let mut inline = false;
        while tokens.at_word()? {
            let (member, member_offset) = tokens.name("expected a member name")?;
            self.add_member(tokens, member, member_offset, 0..0)?;
            inline = true;
        }
        let own = self.annotations(tokens)?;
        self.graph.enums[id as usize].annotations = narrow(own);
        if inline {
            self.current_enum = None;
            return Ok(false);
        }
        if tokens.at(TokenKind::Indent)? {
            tokens.expect(TokenKind::Indent)?;
            return Ok(true);
        }
        Err(tokens.error(ENUM_EMPTY))
    }

    /// Reads one member line of a block enum: its name and annotations.
    pub(super) fn member<'a>(&mut self, tokens: &mut Tokens<'a>) -> Result<(), ParseError> {
        let (word, offset) = tokens.name("expected a member name")?;
        let own = self.annotations(tokens)?;
        self.add_member(tokens, word, offset, narrow(own))
    }

    /// Adds `word` to the enum being declared, refusing a second member with the same name.
    fn add_member<'a>(
        &mut self,
        tokens: &mut Tokens<'a>,
        word: &str,
        offset: usize,
        annotations: Span,
    ) -> Result<(), ParseError> {
        let id = self.current_enum.expect("a member is read inside an enum") as usize;
        let name = self.intern(tokens, word)?;
        let key = (id as u32, name.index());
        if self.member_positions.contains_key(&key) {
            return Err(ParseError::new(MEMBER_DUPLICATE, offset));
        }
        let position = self.graph.members.len() as u32 - self.graph.enums[id].members.start;
        self.graph.members.push(Member {
            name,
            annotations,
            offset,
        });
        self.member_positions.insert(key, position);
        self.graph.enums[id].members.end = self.graph.members.len() as u32;
        Ok(())
    }

    /// Ends a block enum: its closing dedent, and at least one member.
    pub(super) fn enum_end<'a>(&mut self, tokens: &mut Tokens<'a>) -> Result<(), ParseError> {
        let id = self
            .current_enum
            .take()
            .expect("an enum block ends inside an enum") as usize;
        if self.graph.enums[id].members.is_empty() {
            return Err(tokens.error(ENUM_EMPTY));
        }
        tokens.expect(TokenKind::Dedent)
    }

    /// Reads `input NAME : bool|ENUM` and its annotations, after the `input` keyword.
    pub(super) fn input<'a>(&mut self, tokens: &mut Tokens<'a>) -> Result<(), ParseError> {
        let (word, offset) = tokens.name("expected an input name after `input`")?;
        let name = self.intern(tokens, word)?;
        if self
            .bound(name)
            .is_some_and(|binding| binding.input.is_some())
        {
            return Err(ParseError::new(INPUT_DUPLICATE, offset));
        }
        if !tokens.at(TokenKind::Colon)? {
            return Err(tokens.error(INPUT_COLON));
        }
        tokens.expect(TokenKind::Colon)?;
        let (ty_word, ty_offset) = tokens.word_or(INPUT_COLON)?;
        let ty = if ty_word == "bool" {
            Ty::Bool
        } else {
            Ty::Enum(
                self.enum_index(ty_word)
                    .ok_or(ParseError::new(INPUT_TYPE, ty_offset))?,
            )
        };
        let annotations = narrow(self.annotations(tokens)?);
        let id = self.graph.inputs.len() as u32;
        self.slot(name).input = NonZeroU32::new(id + 1);
        self.graph.inputs.push(Input {
            name,
            ty,
            annotations,
            offset,
        });
        Ok(())
    }

    /// Reads the condition of a `when` arm. A machine names a free condition; a pure file names a
    /// declared input: `NAME` for a bool, `NAME:MEMBER` for an enum.
    pub(super) fn when_condition(&mut self, tokens: &mut Tokens<'_>) -> Result<u32, ParseError> {
        if self.kind == Kind::Machine {
            return self.condition(tokens);
        }
        let (word, offset) = tokens.word_or("expected an input name after `when`")?;
        let input = self
            .input_index(word)
            .ok_or(ParseError::new(WHEN_UNKNOWN_INPUT, offset))?;
        let member = if tokens.at(TokenKind::Colon)? {
            tokens.expect(TokenKind::Colon)?;
            Some(tokens.word_or("expected a member name after `:`")?)
        } else {
            None
        };
        let (test, text) = match (self.graph.inputs[input as usize].ty, member) {
            (Ty::Bool, None) => (Test::Flag(input), word.to_owned()),
            (Ty::Bool, Some(_)) => return Err(ParseError::new(WHEN_BOOL_MEMBER, offset)),
            (Ty::Enum(_), None) => return Err(ParseError::new(WHEN_ENUM_MEMBER, offset)),
            (Ty::Enum(ty), Some((member, member_offset))) => {
                let position = self
                    .member_position(ty, member)
                    .ok_or(ParseError::new(WHEN_NOT_MEMBER, member_offset))?;
                (Test::Is(input, position), format!("{word}:{member}"))
            }
        };
        let name = self.intern(tokens, &text)?;
        let id = match self.bound(name).and_then(|binding| binding.condition) {
            Some(id) => id.get() - 1,
            None => {
                let id = self.graph.conditions.len() as u32;
                self.graph.conditions.push(name);
                self.graph.tests.push(test);
                self.slot(name).condition = NonZeroU32::new(id + 1);
                id
            }
        };
        if !self.choice_tests.insert(id) {
            return Err(ParseError::new(WHEN_REPEATED, offset));
        }
        Ok(id)
    }

    /// The position of the member named `word` within enum `ty`.
    fn member_position(&self, ty: u32, word: &str) -> Option<u32> {
        let name = self.graph.strings.find(word)?;
        self.member_positions.get(&(ty, name.index())).copied()
    }

    /// Ends a choose block: it needs an arm, and its closing dedent. Stores the choice.
    pub(super) fn choice_end(
        &mut self,
        tokens: &mut Tokens<'_>,
        name: StringId,
        start: usize,
        offset: usize,
    ) -> Result<(), ParseError> {
        if self.graph.arms.len() == start {
            return Err(tokens.error(CHOOSE_NEEDS_WHEN));
        }
        tokens.expect(TokenKind::Dedent)?;
        let choice = Choice {
            name,
            arms: narrow(start..self.graph.arms.len()),
            ty: self.choice_ty.take(),
            annotations: std::mem::take(&mut self.choice_annotations),
            offset,
        };
        self.graph.choices.push(choice);
        Ok(())
    }

    /// Ends the graph body: a machine needs a node, and both need the closing dedent.
    pub(super) fn body_end(&mut self, tokens: &mut Tokens<'_>) -> Result<(), ParseError> {
        if self.kind == Kind::Machine && self.graph.nodes.is_empty() {
            return Err(tokens.error("graph needs at least one node; the first is its entry"));
        }
        tokens.expect(TokenKind::Dedent)
    }

    /// Reads one arm's `-> tag` and annotations, then stores it. A pure arm's tag must be a
    /// member of its choose's enum, and the defaults apply to it.
    pub(super) fn arm(
        &mut self,
        tokens: &mut Tokens<'_>,
        condition: Option<u32>,
    ) -> Result<(), ParseError> {
        tokens.expect(TokenKind::Arrow)?;
        let (word, tag_offset) = tokens.word_or("expected a tag name after `->`")?;
        let tag = self.intern(tokens, word)?;
        let member = match self.choice_ty {
            Some(ty) => *self
                .member_positions
                .get(&(ty, tag.index()))
                .ok_or(ParseError::new(TAG_NOT_MEMBER, tag_offset))?,
            None => 0,
        };
        let own = self.annotations(tokens)?;
        let annotations = if self.kind == Kind::Pure {
            narrow(self.with_defaults(tokens, own)?)
        } else {
            narrow(own)
        };
        self.graph.arms.push(Arm {
            condition,
            tag,
            annotations,
            member,
        });
        Ok(())
    }

    pub(super) fn annotations(
        &mut self,
        tokens: &mut Tokens<'_>,
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
            self.graph.annotations.push(Annotation { key, value });
        }
        tokens.expect(TokenKind::Newline)?;
        Ok(start..self.graph.annotations.len())
    }

    pub(super) fn annotation_key(
        &mut self,
        tokens: &mut Tokens<'_>,
        key: &str,
        offset: usize,
    ) -> Result<StringId, ParseError> {
        // Checked before interning, so an invalid key gets this message even when interner storage is
        // exhausted.
        if !key.split('.').all(valid_name) {
            return Err(ParseError::new(ANNOTATION_KEY, offset));
        }
        self.intern(tokens, key)
    }
}
