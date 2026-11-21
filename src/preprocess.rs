//! The preprocessor: `@!` directives at the top of a file, read before the lexer runs.
//!
//! The directive stage graph (`src/graphs/directive.graphite`) reads one line at a time. Each
//! `@!` line is checked here. The first other non-blank line starts the body, and no `@!` line
//! may follow it.

use std::sync::OnceLock;

use crate::bootstrap;
use crate::bootstrap::directive::{Task, task};
use crate::graph::CompiledGraph;
use crate::parser::ParseError;
use crate::stage;

/// Every directive line starts with this, at column 0.
const PREFIX: &str = "@!";
/// The byte-order mark some editors write first. It is skipped, so a directive on line 1 still counts.
const BOM: &str = "\u{feff}";

const UNKNOWN: &str = "unknown directive: the directives are `@!pure` and `@!frontend:KEY`";
const TAKES_NO_VALUE: &str = "`@!pure` takes no value";
const NEEDS_VALUE: &str = "`@!frontend` needs a value: `@!frontend:KEY`";
const ONE_WORD: &str = "a directive is one word: `@!NAME` or `@!NAME:VALUE`";
const PURE_REPEATED: &str = "`@!pure` is repeated";
const FRONTEND_REPEATED: &str = "`@!frontend` is repeated";
const AFTER_GRAPH: &str = "directives go at the top of the file, before the first `graph`";

/// The directives at the top of a file, and where the body starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Directives<'a> {
    /// The offset of `@!pure`, if the file has it.
    pub pure: Option<usize>,
    /// The value and offset of `@!frontend:VALUE`, if the file has it. The value is text: the CLI
    /// checks it against the registered targets, so the library knows no front ends.
    pub frontend: Option<(&'a str, usize)>,
    /// The offset where the graphs start. Lexing and parsing begin here.
    pub body: usize,
}

impl Directives<'_> {
    /// Whether a directive makes the file pure: `@!pure`, or `@!frontend:pure`.
    pub fn is_pure(&self) -> bool {
        self.pure.is_some() || self.frontend_is_pure()
    }

    /// Whether `@!frontend:pure` is set. It names no target, so it always makes the file pure.
    pub fn frontend_is_pure(&self) -> bool {
        self.frontend.is_some_and(|(value, _)| value == "pure")
    }
}

fn directive_graph() -> &'static CompiledGraph {
    static GRAPH: OnceLock<CompiledGraph> = OnceLock::new();
    GRAPH.get_or_init(bootstrap::directive::graph)
}

/// Reads the directives at the top of `source`. Errors name the line they are on.
pub fn preprocess(source: &str) -> Result<Directives<'_>, ParseError> {
    let mut reader = Reader {
        source,
        cursor: if source.starts_with(BOM) {
            BOM.len()
        } else {
            0
        },
        pure: None,
        frontend: None,
        body: 0,
        error: None,
    };
    let end = stage::drive(directive_graph(), task, |task| reader.run(task))
        .unwrap_or_else(|stall| panic!("the bundled directive.graphite is wrong: {stall}"));
    if end == "failed" {
        return Err(reader
            .error
            .take()
            .unwrap_or_else(|| ParseError::new("the directive reader stopped", 0)));
    }
    Ok(Directives {
        pure: reader.pure,
        frontend: reader.frontend,
        body: reader.body,
    })
}

/// What the stage tasks share: where the reader is, what it has found, and the first error.
struct Reader<'a> {
    source: &'a str,
    /// Offset of the line the next task reads.
    cursor: usize,
    pure: Option<usize>,
    frontend: Option<(&'a str, usize)>,
    body: usize,
    error: Option<ParseError>,
}

impl<'a> Reader<'a> {
    fn run(&mut self, task: Task) -> &'static [&'static str] {
        let result = match task {
            Task::Line => self.line_task(),
            Task::Directive => self.directive(),
            Task::Rest => self.rest(),
        };
        match result {
            Ok(conditions) => conditions,
            Err(error) => {
                self.error = Some(error);
                &["failed"]
            }
        }
    }

    /// The line at the cursor, without its line break, and the offset just past it.
    fn current_line(&self) -> (&'a str, usize) {
        let source = self.source;
        let rest = &source[self.cursor..];
        let length = rest.find('\n').map_or(rest.len(), |at| at + 1);
        (
            rest[..length].trim_end_matches(['\n', '\r']),
            self.cursor + length,
        )
    }

    /// Classifies the line at the cursor. A blank line is skipped; a `@!` line is a directive;
    /// anything else starts the body.
    fn line_task(&mut self) -> Result<&'static [&'static str], ParseError> {
        if self.cursor >= self.source.len() {
            self.body = self.source.len();
            return Ok(&["body"]);
        }
        let (text, next) = self.current_line();
        if text.trim().is_empty() {
            self.cursor = next;
            return Ok(&["skipped"]);
        }
        if text.starts_with(PREFIX) {
            return Ok(&["directive"]);
        }
        self.body = self.cursor;
        Ok(&["body"])
    }

    fn directive(&mut self) -> Result<&'static [&'static str], ParseError> {
        let offset = self.cursor;
        let (text, next) = self.current_line();
        // A trailing space is invisible in an editor, so it is not a second word. A tab is still
        // refused, as the test of `@!pure\t` requires.
        let word = text[PREFIX.len()..].trim_end_matches(' ');
        if word.contains(char::is_whitespace) {
            return Err(ParseError::new(ONE_WORD, offset));
        }
        let (name, value) = match word.split_once(':') {
            Some((name, value)) => (name, Some(value)),
            None => (word, None),
        };
        match name {
            "pure" => {
                if value.is_some() {
                    return Err(ParseError::new(TAKES_NO_VALUE, offset));
                }
                if self.pure.is_some() {
                    return Err(ParseError::new(PURE_REPEATED, offset));
                }
                self.pure = Some(offset);
            }
            "frontend" => {
                let value = value
                    .filter(|value| !value.is_empty())
                    .ok_or(ParseError::new(NEEDS_VALUE, offset))?;
                if self.frontend.is_some() {
                    return Err(ParseError::new(FRONTEND_REPEATED, offset));
                }
                self.frontend = Some((value, offset));
            }
            _ => return Err(ParseError::new(UNKNOWN, offset)),
        }
        self.cursor = next;
        Ok(&["passed"])
    }

    /// The body has started. No `@!` line may appear anywhere after it.
    fn rest(&mut self) -> Result<&'static [&'static str], ParseError> {
        let mut offset = self.body;
        for line in self.source[self.body..].split_inclusive('\n') {
            if line.starts_with(PREFIX) {
                return Err(ParseError::new(AFTER_GRAPH, offset));
            }
            offset += line.len();
        }
        Ok(&["passed"])
    }
}
