//! The lexer: a streaming scanner that yields tokens and at most one error, then ends.

use crate::lexer::{LexError, LexErrorKind, Token, TokenKind, WORD_BREAK};

/// The lexer as an iterator. Each call scans only as far as the next token or error needs, so the
/// source is read lazily.
pub struct Lexer<'a> {
    state: State<'a>,
    /// Tokens already handed out, as an index into `state.tokens`.
    handed: usize,
    finished: bool,
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str) -> Self {
        Self::at(source, 0)
    }

    /// A lexer that starts at byte `start`, which must be the start of a line. Token offsets stay
    /// absolute, so diagnostics point into the whole source.
    pub fn at(source: &'a str, start: usize) -> Self {
        Lexer {
            state: State::new(source, start),
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
            self.finished = self.state.step();
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
    /// Set once the source is exhausted: the lexer closes the last line and every open block.
    finishing: bool,
    tokens: Vec<Token<'a>>,
    error: Option<LexError>,
}

/// What the first character of a step's text is.
enum Class {
    Eof,
    Newline,
    Tab,
    LineStart,
    At,
    Colon,
    Arrow,
    Word,
}

impl<'a> State<'a> {
    fn new(source: &'a str, start: usize) -> Self {
        State {
            source,
            position: start,
            pass_start: start,
            indents: Vec::new(),
            pending_dedents: 0,
            line_start: true,
            line_has_tokens: false,
            finishing: false,
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

    fn fail(&mut self, kind: LexErrorKind, offset: usize) -> bool {
        self.error = Some(LexError { kind, offset });
        true
    }

    /// Runs one step and returns `true` once the lexer is done or has failed. A step may emit
    /// tokens or none; the caller hands tokens out between steps.
    fn step(&mut self) -> bool {
        if self.finishing {
            return self.finish();
        }
        if self.pending_dedents > 0 {
            self.pending_dedents -= 1;
            self.emit(TokenKind::Dedent, self.position);
            return false;
        }
        match self.classify() {
            Class::Eof => {
                self.finishing = true;
                self.finish()
            }
            Class::Newline => {
                self.line_break();
                false
            }
            Class::Tab => self.fail(LexErrorKind::Tab, self.position),
            Class::LineStart => self.indentation(),
            Class::At => self.punctuation(TokenKind::At, 1),
            Class::Colon => self.punctuation(TokenKind::Colon, 1),
            Class::Arrow => self.punctuation(TokenKind::Arrow, 2),
            Class::Word => self.word(),
        }
    }

    /// Skips the spaces, then reports what the next character is.
    fn classify(&mut self) -> Class {
        let bytes = self.bytes();
        self.pass_start = self.position;
        while bytes.get(self.position) == Some(&b' ') {
            self.position += 1;
        }
        match bytes.get(self.position) {
            None => Class::Eof,
            Some(b'\n' | b'\r') => Class::Newline,
            Some(b'\t') => Class::Tab,
            Some(_) if self.line_start => Class::LineStart,
            Some(b'@') => Class::At,
            Some(b':') => Class::Colon,
            Some(b'-') if bytes.get(self.position + 1) == Some(&b'>') => Class::Arrow,
            Some(_) => Class::Word,
        }
    }

    /// Consumes a line break, and emits the line's newline when the line had tokens.
    fn line_break(&mut self) {
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
    }

    /// The first token of a line: its indentation becomes an indent, a dedent, or nothing.
    fn indentation(&mut self) -> bool {
        self.line_start = false;
        let width = self.position - self.pass_start;
        let current = self.indents.last().copied().unwrap_or(0);
        if width > current {
            self.indents.push(width);
            self.emit(TokenKind::Indent, self.position);
            return false;
        }
        if width == current {
            return false;
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
                    return true;
                }
            }
        };
        self.pending_dedents = self.indents.len() - depth - 1;
        self.indents.truncate(depth);
        self.emit(TokenKind::Dedent, self.position);
        false
    }

    /// `@`, `:` or `->`: a token of its own.
    fn punctuation(&mut self, kind: TokenKind<'a>, length: usize) -> bool {
        let offset = self.position;
        self.position += length;
        self.line_has_tokens = true;
        self.emit(kind, offset);
        false
    }

    /// A bare word: everything up to a break character or `->`.
    fn word(&mut self) -> bool {
        let offset = self.position;
        self.position += word_length(&self.bytes()[offset..]);
        self.line_has_tokens = true;
        let text = &self.source[offset..self.position];
        self.emit(TokenKind::Word(text), offset);
        false
    }

    /// The end of the source: close the last line, then close every open block.
    fn finish(&mut self) -> bool {
        if self.line_has_tokens {
            self.line_has_tokens = false;
            self.emit(TokenKind::Newline, self.position);
            return false;
        }
        match self.indents.pop() {
            Some(_) => {
                self.emit(TokenKind::Dedent, self.position);
                false
            }
            None => true,
        }
    }
}

/// The length of the word at the start of `bytes`: up to a break character or `->`.
fn word_length(bytes: &[u8]) -> usize {
    let mut length = 0;
    while let Some(&byte) = bytes.get(length) {
        if WORD_BREAK[byte as usize] || (byte == b'-' && bytes.get(length + 1) == Some(&b'>')) {
            break;
        }
        length += 1;
    }
    length
}
