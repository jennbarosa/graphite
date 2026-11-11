//! Streaming lexical analysis. Grammar and schema validation belong to the parser.

mod diagnostic;
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
const WORD_BREAK: [bool; 256] = word_break_table();

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

/// O(source bytes) scanning with borrowed words and O(nesting depth) storage.
/// Blank lines are ignored; the final nonempty line gets a newline even without LF.
/// Tabs are rejected rather than assigned an implicit display width.
pub struct Lexer<'a> {
    source: &'a str,
    position: usize,
    indents: Vec<usize>,
    pending_dedents: usize,
    line_start: bool,
    line_has_tokens: bool,
    failed: bool,
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
            position: 0,
            indents: Vec::new(),
            pending_dedents: 0,
            line_start: true,
            line_has_tokens: false,
            failed: false,
        }
    }

    fn token(&self, kind: TokenKind<'a>, offset: usize) -> Option<Result<Token<'a>, LexError>> {
        Some(Ok(Token { kind, offset }))
    }

    fn error(&mut self, kind: LexErrorKind, offset: usize) -> Option<Result<Token<'a>, LexError>> {
        self.failed = true;
        Some(Err(LexError { kind, offset }))
    }

    #[inline]
    fn indentation(&mut self, width: usize) -> Result<Option<TokenKind<'a>>, LexErrorKind> {
        let current = self.indents.last().copied().unwrap_or(0);
        if width > current {
            self.indents.push(width);
            return Ok(Some(TokenKind::Indent));
        }
        if width == current {
            return Ok(None);
        }

        let depth = if width == 0 {
            0
        } else {
            self.indents
                .binary_search(&width)
                .map_err(|_| LexErrorKind::Indentation)?
                + 1
        };
        self.pending_dedents = self.indents.len() - depth - 1;
        self.indents.truncate(depth);
        Ok(Some(TokenKind::Dedent))
    }

    /// Classifies the lexeme starting at `offset` and advances past it.
    #[inline(always)]
    fn scan(&mut self, offset: usize) -> TokenKind<'a> {
        let bytes = self.source.as_bytes();
        match bytes[offset] {
            b'@' => {
                self.position += 1;
                TokenKind::At
            }
            b':' => {
                self.position += 1;
                TokenKind::Colon
            }
            b'-' if bytes.get(offset + 1) == Some(&b'>') => {
                self.position += 2;
                TokenKind::Arrow
            }
            _ => {
                #[cfg(all(feature = "lexer-simd", target_arch = "x86_64"))]
                {
                    // The SIMD scanner already stops on the same breaks, including `->`.
                    self.position += simd::word_prefix_len(&bytes[self.position..]);
                }
                #[cfg(not(all(feature = "lexer-simd", target_arch = "x86_64")))]
                while let Some(&byte) = bytes.get(self.position) {
                    if WORD_BREAK[byte as usize]
                        || (byte == b'-' && bytes.get(self.position + 1) == Some(&b'>'))
                    {
                        break;
                    }
                    self.position += 1;
                }
                TokenKind::Word(&self.source[offset..self.position])
            }
        }
    }
}

impl<'a> Iterator for Lexer<'a> {
    type Item = Result<Token<'a>, LexError>;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        if self.failed {
            return None;
        }
        if self.pending_dedents > 0 {
            self.pending_dedents -= 1;
            return self.token(TokenKind::Dedent, self.position);
        }

        let bytes = self.source.as_bytes();
        loop {
            let start = self.position;
            while bytes.get(self.position) == Some(&b' ') {
                self.position += 1;
            }
            let offset = self.position;
            match bytes.get(offset) {
                None => {
                    if self.line_has_tokens {
                        self.line_has_tokens = false;
                        return self.token(TokenKind::Newline, offset);
                    }
                    // No open block left means the input is fully consumed.
                    self.indents.pop()?;
                    return self.token(TokenKind::Dedent, offset);
                }
                Some(b'\t') => return self.error(LexErrorKind::Tab, offset),
                Some(b'\n' | b'\r') => {
                    self.position += 1;
                    if bytes[offset] == b'\r' && bytes.get(self.position) == Some(&b'\n') {
                        self.position += 1;
                    }
                    self.line_start = true;
                    if self.line_has_tokens {
                        self.line_has_tokens = false;
                        return self.token(TokenKind::Newline, offset);
                    }
                    continue;
                }
                _ => {}
            }

            if self.line_start {
                self.line_start = false;
                match self.indentation(offset - start) {
                    Ok(Some(kind)) => return self.token(kind, offset),
                    Err(kind) => return self.error(kind, start),
                    Ok(None) => {}
                }
            }

            self.line_has_tokens = true;
            let kind = self.scan(offset);
            return self.token(kind, offset);
        }
    }
}
