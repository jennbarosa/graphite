use crate::lexer::{Diagnostic, LexError};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParseError {
    pub message: &'static str,
    pub offset: usize,
}

impl ParseError {
    pub(super) fn new(message: &'static str, offset: usize) -> Self {
        Self { message, offset }
    }

    /// Renders the error with its source line and a caret under the offending text.
    /// `source` must be the text that was parsed. Costs nothing until formatted.
    pub fn diagnostic<'s>(&self, source: &'s str) -> Diagnostic<'s> {
        Diagnostic::new(self.message, self.offset, source)
    }
}

impl From<LexError> for ParseError {
    fn from(error: LexError) -> Self {
        Self::new(error.kind.message(), error.offset)
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{} at byte {}", self.message, self.offset)
    }
}

impl std::error::Error for ParseError {}
