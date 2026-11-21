//! The lexer's indentation, punctuation and error paths, checked on exact token streams. Offsets
//! point into the source, so each expectation names the byte a token starts at.

use graphite::lexer::{Diagnostic, LexError, LexErrorKind, Lexer, Token, TokenKind};

fn lex(source: &str) -> Vec<Result<Token<'_>, LexError>> {
    Lexer::new(source).collect()
}

fn kinds(source: &str) -> Vec<TokenKind<'_>> {
    lex(source)
        .into_iter()
        .map(|item| item.expect("no error expected").kind)
        .collect()
}

#[test]
fn indentation_opens_holds_and_closes_blocks() {
    use TokenKind::*;
    assert_eq!(
        kinds("a\n  b\n  c\nd\n"),
        vec![
            Word("a"),
            Newline,
            Indent,
            Word("b"),
            Newline,
            Word("c"),
            Newline,
            Dedent,
            Word("d"),
            Newline,
        ]
    );
}

#[test]
fn a_dedent_past_several_levels_closes_each_block() {
    use TokenKind::*;
    // Two levels open at once, then both close on the same unindented line.
    assert_eq!(
        kinds("a\n  b\n    c\nd\n"),
        vec![
            Word("a"),
            Newline,
            Indent,
            Word("b"),
            Newline,
            Indent,
            Word("c"),
            Newline,
            Dedent,
            Dedent,
            Word("d"),
            Newline,
        ]
    );
}

#[test]
fn a_dedent_to_an_open_level_stops_there() {
    use TokenKind::*;
    // Back to the middle level: one dedent closes the inner block, and the middle one stays open
    // until the end of the source.
    assert_eq!(
        kinds("a\n  b\n    c\n  d\n"),
        vec![
            Word("a"),
            Newline,
            Indent,
            Word("b"),
            Newline,
            Indent,
            Word("c"),
            Newline,
            Dedent,
            Word("d"),
            Newline,
            Dedent,
        ]
    );
}

#[test]
fn an_open_block_closes_at_the_end_of_the_source() {
    use TokenKind::*;
    let tokens = kinds("a\n  b");
    assert_eq!(
        tokens,
        vec![Word("a"), Newline, Indent, Word("b"), Newline, Dedent]
    );
}

#[test]
fn punctuation_and_arrows_are_tokens_of_their_own() {
    use TokenKind::*;
    assert_eq!(
        kinds("a -> b @k:v"),
        vec![
            Word("a"),
            Arrow,
            Word("b"),
            At,
            Word("k"),
            Colon,
            Word("v"),
            Newline
        ]
    );
}

#[test]
fn a_dash_that_is_not_an_arrow_stays_in_the_word() {
    assert_eq!(kinds("a-b-")[0], TokenKind::Word("a-b-"));
}

#[test]
fn an_empty_source_has_no_tokens() {
    assert!(lex("").is_empty());
    assert!(lex("\n\n").iter().all(|item| item.is_ok()));
}

#[test]
fn a_lexer_started_mid_source_keeps_absolute_offsets() {
    let source = "a\n  b\n";
    let second_line = source.find("  b").unwrap();
    let tokens: Vec<_> = Lexer::at(source, second_line).map(Result::unwrap).collect();
    assert_eq!(tokens[0].kind, TokenKind::Indent);
    assert_eq!(tokens[0].offset, second_line + 2);
    assert_eq!(tokens[1].kind, TokenKind::Word("b"));
    assert_eq!(tokens[1].offset, second_line + 2);
}

#[test]
fn a_tab_is_refused_at_its_offset() {
    let source = "a\n\tb\n";
    let error = lex(source).into_iter().find_map(Result::err).unwrap();
    assert_eq!(error.kind, LexErrorKind::Tab);
    assert_eq!(error.offset, source.find('\t').unwrap());
    assert_eq!(error.to_string(), "use spaces instead of tabs at byte 2");
}

#[test]
fn a_dedent_to_no_open_level_is_refused_at_the_line_start() {
    let source = "a\n    b\n  c\n";
    let items = lex(source);
    let error = items.iter().find_map(|item| item.as_ref().err()).unwrap();
    assert_eq!(error.kind, LexErrorKind::Indentation);
    assert_eq!(error.offset, source.find("  c").unwrap());
    assert_eq!(
        error.kind.message(),
        "dedent does not match an earlier indentation level"
    );
    // The iterator stops after the error: nothing follows it.
    assert!(matches!(items.last(), Some(Err(_))));
}

#[test]
fn a_lex_error_renders_with_its_source_line() {
    let source = "a\n  b\n c\n";
    let error = lex(source).into_iter().find_map(Result::err).unwrap();
    let text = error.diagnostic(source).to_string();
    assert!(
        text.starts_with(
            "error: dedent does not match an earlier indentation level\n --> line 3, column 1"
        ),
        "{text}"
    );
}

#[test]
fn a_diagnostic_can_carry_a_message_of_its_own() {
    let text = Diagnostic::new("custom", 0, "one line").to_string();
    assert_eq!(
        text,
        "error: custom\n --> line 1, column 1\n  |\n1 | one line\n  | ^^^ custom"
    );
}

#[test]
fn a_diagnostic_past_the_end_points_at_the_last_line() {
    let text = Diagnostic::new("late", 999, "abc\r\n").to_string();
    assert!(text.contains("--> line 2, column 1"), "{text}");
}
