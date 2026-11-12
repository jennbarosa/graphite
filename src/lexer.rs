//! Streaming lexical analysis, driven by the graph in `lex.graphite`. Grammar and schema
//! validation belong to the parser.

mod diagnostic;
pub mod lex;
#[cfg(all(feature = "lexer-simd", target_arch = "x86_64"))]
mod simd;

pub use diagnostic::Diagnostic;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenKind<'a> {
    Word(&'a str),
    Arrow,
    At,
    Colon,
    Newline,
    Indent,
    Dedent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Token<'a> {
    pub kind: TokenKind<'a>,
    /// Byte offset into the original UTF-8 source, including for synthetic tokens.
    pub offset: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LexErrorKind {
    Tab,
    Indentation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LexError {
    pub kind: LexErrorKind,
    pub offset: usize,
}

impl LexErrorKind {
    pub fn message(self) -> &'static str {
        match self {
            LexErrorKind::Tab => "use spaces instead of tabs",
            LexErrorKind::Indentation => "dedent does not match an earlier indentation level",
        }
    }
}

impl LexError {
    /// Renders the error with its source line and a caret under the offending text.
    /// `source` must be the text that was lexed. Costs nothing until formatted.
    pub fn diagnostic<'s>(&self, source: &'s str) -> Diagnostic<'s> {
        Diagnostic::new(self.kind.message(), self.offset, source)
    }
}

impl std::fmt::Display for LexError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{} at byte {}", self.kind.message(), self.offset)
    }
}

impl std::error::Error for LexError {}

/// Bytes that end a bare word; `-` is a break only before `>`, which the scanners check.
pub(crate) const WORD_BREAK: [bool; 256] = word_break_table();

const fn word_break_table() -> [bool; 256] {
    let mut table = [false; 256];
    let breaks = b" \t\n\r@:";
    let mut index = 0;
    while index < breaks.len() {
        table[breaks[index] as usize] = true;
        index += 1;
    }
    table
}

pub use lex::Lexer;
