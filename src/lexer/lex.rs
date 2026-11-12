//! The lexer as a stage graph. `lex.graphite` holds its states and their edges. Each state's task here
//! scans its part of the source, and reports which edges hold next. The result is the same token
//! stream as the iterator in `lexer.rs`, and the same first error.

use std::sync::OnceLock;

use crate::bootstrap;
use crate::bootstrap::lex::{Task, task};
use crate::graph::CompiledGraph;
use crate::lexer::{LexError, LexErrorKind, Token, TokenKind};
use crate::stage::Stepper;

fn lexer_graph() -> &'static CompiledGraph {
    static GRAPH: OnceLock<CompiledGraph> = OnceLock::new();
    GRAPH.get_or_init(bootstrap::lex::graph)
}

/// The lexer as an iterator. Each call steps the graph until it has a token
/// or an error to hand out, so the source is scanned lazily.
pub struct Lexer<'a> {
    stepper: Stepper<'static, Task>,
    state: State<'a>,
    /// Tokens already handed out, as an index into `state.tokens`.
    handed: usize,
    finished: bool,
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str) -> Self {
        Lexer {
            stepper: Stepper::new(lexer_graph(), task)
                .unwrap_or_else(|error| panic!("the bundled lex.graphite is wrong: {error}")),
            state: State::new(source),
            handed: 0,
            finished: false,
        }
    }
}

impl<'a> Iterator for Lexer<'a> {
    type Item = Result<Token<'a>, LexError>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(&token) = self.state.tokens.get(self.handed) {
                self.handed += 1;
                return Some(Ok(token));
            }
            // Everything emitted so far has been handed out, so the buffer is reused.
            self.state.tokens.clear();
            self.handed = 0;
            if self.finished {
                return self.state.error.take().map(Err);
            }
            let state = &mut self.state;
            let end = self
                .stepper
                .step(&mut |task| state.run(task))
                .unwrap_or_else(|message| panic!("the lexer graph stopped: {message}"));
            if end.is_some() {
                self.finished = true;
            }
        }
    }
}

/// The lexer's memory: where it is in the source, the indentation stack, and what it has emitted.
struct State<'a> {
    source: &'a str,
    position: usize,
    /// Where the current pass over the source started, before its leading spaces.
    pass_start: usize,
    indents: Vec<usize>,
    pending_dedents: usize,
    line_start: bool,
    line_has_tokens: bool,
    tokens: Vec<Token<'a>>,
    error: Option<LexError>,
}

impl<'a> State<'a> {
    fn new(source: &'a str) -> Self {
        State {
            source,
            position: 0,
            pass_start: 0,
            indents: Vec::new(),
            pending_dedents: 0,
            line_start: true,
            line_has_tokens: false,
            tokens: Vec::new(),
            error: None,
        }
    }

    fn bytes(&self) -> &'a [u8] {
        self.source.as_bytes()
    }

    fn emit(&mut self, kind: TokenKind<'a>, offset: usize) {
        self.tokens.push(Token { kind, offset });
    }

    fn fail(&mut self, kind: LexErrorKind, offset: usize) -> &'static [&'static str] {
        self.error = Some(LexError { kind, offset });
        &["fail"]
    }

    /// Runs the task of one state and returns the conditions that hold after it.
    fn run(&mut self, task: Task) -> &'static [&'static str] {
        match task {
            Task::Pending => self.pending(),
            Task::Classify => self.classify(),
            Task::LineBreak => self.line_break(),
            Task::Indentation => self.indentation(),
            Task::At => self.punctuation(TokenKind::At, 1),
            Task::Colon => self.punctuation(TokenKind::Colon, 1),
            Task::Arrow => self.punctuation(TokenKind::Arrow, 2),
            Task::Word => self.word(),
            Task::TabError => self.fail(LexErrorKind::Tab, self.position),
            Task::Finish => self.finish(),
        }
    }

    /// Emits a pending dedent, one per visit, or reports that none are left.
    fn pending(&mut self) -> &'static [&'static str] {
        if self.pending_dedents == 0 {
            return &["idle"];
        }
        self.pending_dedents -= 1;
        self.emit(TokenKind::Dedent, self.position);
        &["dedent"]
    }

    /// Skips the spaces, then reports what the next character is.
    fn classify(&mut self) -> &'static [&'static str] {
        let bytes = self.bytes();
        self.pass_start = self.position;
        while bytes.get(self.position) == Some(&b' ') {
            self.position += 1;
        }
        match bytes.get(self.position) {
            None => &["eof"],
            Some(b'\n' | b'\r') => &["newline"],
            Some(b'\t') => &["tab"],
            Some(_) if self.line_start => &["line_start"],
            Some(b'@') => &["at_sign"],
            Some(b':') => &["colon"],
            Some(b'-') if bytes.get(self.position + 1) == Some(&b'>') => &["arrow"],
            Some(_) => &["word"],
        }
    }

    /// Consumes a line break, and emits the line's newline when the line had tokens.
    fn line_break(&mut self) -> &'static [&'static str] {
        let bytes = self.bytes();
        let offset = self.position;
        self.position += 1;
        if bytes[offset] == b'\r' && bytes.get(self.position) == Some(&b'\n') {
            self.position += 1;
        }
        self.line_start = true;
        if self.line_has_tokens {
            self.line_has_tokens = false;
            self.emit(TokenKind::Newline, offset);
        }
        &["again"]
    }

    /// The first token of a line: its indentation becomes an indent, a dedent, or nothing.
    fn indentation(&mut self) -> &'static [&'static str] {
        self.line_start = false;
        let width = self.position - self.pass_start;
        let current = self.indents.last().copied().unwrap_or(0);
        if width > current {
            self.indents.push(width);
            self.emit(TokenKind::Indent, self.position);
            return &["indent"];
        }
        if width == current {
            return &["same"];
        }
        let depth = if width == 0 {
            0
        } else {
            match self.indents.binary_search(&width) {
                Ok(found) => found + 1,
                Err(_) => {
                    self.error = Some(LexError {
                        kind: LexErrorKind::Indentation,
                        offset: self.pass_start,
                    });
                    return &["bad"];
                }
            }
        };
        self.pending_dedents = self.indents.len() - depth - 1;
        self.indents.truncate(depth);
        self.emit(TokenKind::Dedent, self.position);
        &["dedent"]
    }

    /// `@`, `:` or `->`: a token of its own.
    fn punctuation(&mut self, kind: TokenKind<'a>, length: usize) -> &'static [&'static str] {
        let offset = self.position;
        self.position += length;
        self.line_has_tokens = true;
        self.emit(kind, offset);
        &["again"]
    }

    /// A bare word: everything up to a break character or `->`.
    fn word(&mut self) -> &'static [&'static str] {
        let offset = self.position;
        self.position += word_length(&self.bytes()[offset..]);
        self.line_has_tokens = true;
        let text = &self.source[offset..self.position];
        self.emit(TokenKind::Word(text), offset);
        &["again"]
    }

    /// The end of the source: close the last line, then close every open block.
    fn finish(&mut self) -> &'static [&'static str] {
        if self.line_has_tokens {
            self.line_has_tokens = false;
            self.emit(TokenKind::Newline, self.position);
            return &["again"];
        }
        match self.indents.pop() {
            Some(_) => {
                self.emit(TokenKind::Dedent, self.position);
                &["again"]
            }
            None => &["done"],
        }
    }
}

/// The length of the word at the start of `bytes`: up to a break character or `->`. Uses the same
/// SIMD scan as `Lexer` when that feature is on, and a byte loop otherwise.
#[cfg(all(feature = "lexer-simd", target_arch = "x86_64"))]
fn word_length(bytes: &[u8]) -> usize {
    super::simd::word_prefix_len(bytes)
}

#[cfg(not(all(feature = "lexer-simd", target_arch = "x86_64")))]
fn word_length(bytes: &[u8]) -> usize {
    use crate::lexer::WORD_BREAK;
    let mut length = 0;
    while let Some(&byte) = bytes.get(length) {
        if WORD_BREAK[byte as usize] || (byte == b'-' && bytes.get(length + 1) == Some(&b'>')) {
            break;
        }
        length += 1;
    }
    length
}
