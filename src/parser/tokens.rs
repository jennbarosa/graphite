use crate::lexer::{LexError, Token, TokenKind};

use super::ParseError;

/// The token source a parse reads: any lexer that yields tokens or errors in source order.
pub(crate) type TokenSource<'a> = Box<dyn Iterator<Item = Result<Token<'a>, LexError>> + 'a>;

pub(super) struct Tokens<'a> {
    lexer: TokenSource<'a>,
    current: Option<Result<Token<'a>, LexError>>,
    end: usize,
}

impl<'a> Tokens<'a> {
    pub fn new(source: &'a str, mut lexer: TokenSource<'a>) -> Self {
        let current = lexer.next();
        Self {
            lexer,
            current,
            end: source.len(),
        }
    }

    pub fn peek(&mut self) -> Result<Option<Token<'a>>, ParseError> {
        self.current.transpose().map_err(Into::into)
    }

    pub fn offset(&mut self) -> usize {
        match self.current {
            Some(Ok(token)) => token.offset,
            Some(Err(error)) => error.offset,
            None => self.end,
        }
    }

    pub fn error(&mut self, message: &'static str) -> ParseError {
        ParseError::new(message, self.offset())
    }

    pub fn at(&mut self, kind: TokenKind<'a>) -> Result<bool, ParseError> {
        Ok(self.peek()?.is_some_and(|token| token.kind == kind))
    }

    pub fn expect(&mut self, kind: TokenKind<'a>) -> Result<(), ParseError> {
        if !self.at(kind)? {
            return Err(self.error(expected(kind)));
        }
        self.advance();
        Ok(())
    }

    pub fn word(&mut self) -> Result<(&'a str, usize), ParseError> {
        self.word_or("expected a word")
    }

    /// Reads a word, reporting `message` at the current token when there is none.
    pub fn word_or(&mut self, message: &'static str) -> Result<(&'a str, usize), ParseError> {
        match self.peek()? {
            Some(Token {
                kind: TokenKind::Word(word),
                offset,
            }) => {
                self.advance();
                Ok((word, offset))
            }
            _ => Err(self.error(message)),
        }
    }

    pub fn name(&mut self, message: &'static str) -> Result<(&'a str, usize), ParseError> {
        let (word, offset) = self.word_or(message)?;
        if !valid_name(word) {
            return Err(ParseError::new(INVALID_NAME, offset));
        }
        Ok((word, offset))
    }

    // Keep the lexer body out of repeated cached-token checks.
    #[inline(never)]
    fn advance(&mut self) {
        self.current = self.lexer.next();
    }
}

pub(super) const INVALID_NAME: &str = "invalid name: use letters, digits, `_`, or `-`";

/// What the grammar wanted when `kind` was missing. Kept out of line so callers stay small.
#[cold]
#[inline(never)]
fn expected(kind: TokenKind<'_>) -> &'static str {
    match kind {
        TokenKind::Word("graph") => "expected `graph` to start a graph block",
        TokenKind::Word("when") => "expected `when`",
        TokenKind::Word(_) => "expected a keyword",
        TokenKind::Arrow => "expected `->`",
        TokenKind::At => "expected `@` before an annotation",
        TokenKind::Colon => "expected `:` before an annotation value",
        TokenKind::Newline => "expected the end of the line",
        TokenKind::Indent => "expected an indented block",
        TokenKind::Dedent => "expected the end of the block",
    }
}

pub(super) fn valid_name(word: &str) -> bool {
    #[cfg(test)]
    tests::VALIDATIONS.with(|count| count.set(count.get() + 1));

    !word.is_empty()
        && word
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
}

#[cfg(test)]
#[path = "../../tests/support/name_validation.rs"]
mod tests;
