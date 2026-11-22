//! The preprocessor: `@!` directives at the top of a file, read before the lexer runs. The first
//! other non-blank line starts the body, and no `@!` line may follow it.

use crate::parser::ParseError;

/// Every directive line starts with this, at column 0.
const PREFIX: &str = "@!";
/// The byte-order mark some editors write first. It is skipped, so a directive on line 1 still counts.
const BOM: &str = "\u{feff}";

const UNKNOWN: &str = "unknown directive: the only directive is `@!frontend:KEY`";
const NEEDS_VALUE: &str = "`@!frontend` needs a value: `@!frontend:KEY`";
const ONE_WORD: &str = "a directive is one word: `@!NAME` or `@!NAME:VALUE`";
const FRONTEND_REPEATED: &str = "`@!frontend` is repeated";
const AFTER_GRAPH: &str = "directives go at the top of the file, before the first `graph`";

/// The directives at the top of a file, and where the body starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Directives<'a> {
    /// The value and offset of `@!frontend:VALUE`, if the file has it. The value is text. The CLI
    /// checks it against the registered targets; `compile` only asks the registry whether it names a
    /// pure front end, and an unregistered key is left for the caller to refuse.
    pub frontend: Option<(&'a str, usize)>,
    /// The offset where the graphs start. Lexing and parsing begin here.
    pub body: usize,
}

/// Reads the directives at the top of `source`. Errors name the line they are on.
pub fn preprocess(source: &str) -> Result<Directives<'_>, ParseError> {
    let mut cursor = if source.starts_with(BOM) {
        BOM.len()
    } else {
        0
    };
    let mut frontend = None;
    let body = loop {
        if cursor >= source.len() {
            break source.len();
        }
        let (text, next) = current_line(source, cursor);
        if text.trim().is_empty() {
            cursor = next;
        } else if text.starts_with(PREFIX) {
            frontend = directive(text, cursor, frontend)?;
            cursor = next;
        } else {
            break cursor;
        }
    };
    reject_directives_after(source, body)?;
    Ok(Directives { frontend, body })
}

/// The line starting at `cursor`, without its line break, and the offset just past it.
fn current_line(source: &str, cursor: usize) -> (&str, usize) {
    let rest = &source[cursor..];
    let length = rest.find('\n').map_or(rest.len(), |at| at + 1);
    (
        rest[..length].trim_end_matches(['\n', '\r']),
        cursor + length,
    )
}

/// Reads one `@!` line, found at `offset`, and returns the front end chosen so far.
fn directive<'a>(
    text: &'a str,
    offset: usize,
    frontend: Option<(&'a str, usize)>,
) -> Result<Option<(&'a str, usize)>, ParseError> {
    // A trailing space is invisible in an editor, so it is not a second word. A tab is still
    // refused.
    let word = text[PREFIX.len()..].trim_end_matches(' ');
    if word.contains(char::is_whitespace) {
        return Err(ParseError::new(ONE_WORD, offset));
    }
    let (name, value) = match word.split_once(':') {
        Some((name, value)) => (name, Some(value)),
        None => (word, None),
    };
    match name {
        "frontend" => {
            let value = value
                .filter(|value| !value.is_empty())
                .ok_or(ParseError::new(NEEDS_VALUE, offset))?;
            if frontend.is_some() {
                return Err(ParseError::new(FRONTEND_REPEATED, offset));
            }
            Ok(Some((value, offset)))
        }
        _ => Err(ParseError::new(UNKNOWN, offset)),
    }
}

/// No `@!` line may appear anywhere in the body.
fn reject_directives_after(source: &str, body: usize) -> Result<(), ParseError> {
    let mut offset = body;
    for line in source[body..].split_inclusive('\n') {
        if line.starts_with(PREFIX) {
            return Err(ParseError::new(AFTER_GRAPH, offset));
        }
        offset += line.len();
    }
    Ok(())
}
