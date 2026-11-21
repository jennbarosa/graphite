//! The preprocessor: `@!` directives at the top of a file, and what the library does around them.

use graphite::lexer::{Lexer, TokenKind};
use graphite::parser::{ParseError, Schema, parse};
use graphite::pipeline::{block_capacities, compile};
use graphite::preprocess::preprocess;

const GRAPH: &str = "graph door\n  node a end\n";

/// The error `compile` gives for `source`. Graphs have no `Debug`, so `unwrap_err` can't be used.
fn error_of(source: &str) -> ParseError {
    match compile(source, &Schema::default()) {
        Err(error) => error,
        Ok(_) => panic!("expected `compile` to refuse the source"),
    }
}

#[test]
fn directives_are_read_with_blank_lines_between_them() {
    let source = format!("@!pure\n\n@!frontend:purescript\n\n{GRAPH}");
    let directives = preprocess(&source).unwrap();
    assert_eq!(directives.pure, Some(0));
    assert_eq!(directives.frontend, Some(("purescript", 8)));
    assert_eq!(directives.body, source.find("graph").unwrap());
}

#[test]
fn a_file_without_directives_has_no_signals_and_its_body_starts_at_zero() {
    let directives = preprocess(GRAPH).unwrap();
    assert_eq!(directives.pure, None);
    assert_eq!(directives.frontend, None);
    assert_eq!(directives.body, 0);
}

#[test]
fn a_file_of_only_directives_has_an_empty_body_at_its_end() {
    let source = "@!pure\n";
    let directives = preprocess(source).unwrap();
    assert_eq!(directives.body, source.len());
}

#[test]
fn an_unknown_directive_is_refused_on_its_line() {
    let error = preprocess(&format!("@!puer\n{GRAPH}")).unwrap_err();
    assert_eq!(error.offset, 0);
    assert_eq!(
        error.message,
        "unknown directive: the directives are `@!pure` and `@!frontend:KEY`"
    );
}

#[test]
fn a_frontend_directive_without_a_value_is_refused() {
    let error = preprocess(&format!("@!frontend\n{GRAPH}")).unwrap_err();
    assert_eq!(error.offset, 0);
    assert_eq!(
        error.message,
        "`@!frontend` needs a value: `@!frontend:KEY`"
    );
}

#[test]
fn an_empty_frontend_value_is_a_missing_value() {
    let error = preprocess(&format!("@!frontend:\n{GRAPH}")).unwrap_err();
    assert_eq!(
        error.message,
        "`@!frontend` needs a value: `@!frontend:KEY`"
    );
}

#[test]
fn pure_refuses_a_value() {
    let error = preprocess(&format!("@!pure:yes\n{GRAPH}")).unwrap_err();
    assert_eq!(error.offset, 0);
    assert_eq!(error.message, "`@!pure` takes no value");
}

#[test]
fn a_directive_is_one_word_with_nothing_else_on_its_line() {
    for line in ["@!frontend: typescript", "@!pure extra", "@!pure\t"] {
        let error = preprocess(&format!("{line}\n{GRAPH}")).unwrap_err();
        assert_eq!(error.offset, 0, "{line}");
        assert_eq!(
            error.message, "a directive is one word: `@!NAME` or `@!NAME:VALUE`",
            "{line}"
        );
    }
}

#[test]
fn a_repeated_directive_is_refused_at_the_second_copy() {
    let source = format!("@!pure\n@!pure\n{GRAPH}");
    let error = preprocess(&source).unwrap_err();
    assert_eq!(error.offset, 7);
    assert_eq!(error.message, "`@!pure` is repeated");

    let source = format!("@!frontend:rust\n@!frontend:rust\n{GRAPH}");
    let error = preprocess(&source).unwrap_err();
    assert_eq!(error.offset, 16);
    assert_eq!(error.message, "`@!frontend` is repeated");
}

#[test]
fn a_directive_after_the_first_graph_is_refused_at_its_line() {
    let source = format!("{GRAPH}@!pure\n");
    let error = preprocess(&source).unwrap_err();
    assert_eq!(error.offset, source.find("@!pure").unwrap());
    assert_eq!(
        error.message,
        "directives go at the top of the file, before the first `graph`"
    );
}

#[test]
fn a_directive_after_other_text_is_refused_too() {
    let source = "graph door\n  node a end\n\n@!pure\n";
    let error = preprocess(source).unwrap_err();
    assert_eq!(error.offset, source.find("@!pure").unwrap());
}

#[test]
fn errors_in_the_body_point_at_the_same_line_with_or_without_the_header() {
    let body = "graph door\n  node a end\n  node a end\n";
    let header = "@!frontend:typescript\n\n";
    let plain = error_of(body);
    let with_header = error_of(&format!("{header}{body}"));
    assert_eq!(with_header.message, plain.message);
    assert_eq!(with_header.offset, plain.offset + header.len());
}

#[test]
fn compile_builds_a_file_with_a_directive_header() {
    let source = format!("@!frontend:typescript\n\n{GRAPH}");
    let graphs = compile(&source, &Schema::default()).unwrap();
    assert_eq!(graphs.len(), 1);
}

#[test]
fn block_capacities_of_a_header_match_its_body_alone() {
    let body = "graph door\n  node a end\n  node b end\n    a -> b\n";
    let with_header = format!("@!frontend:typescript\n\n{body}");
    assert_eq!(
        block_capacities(&with_header, &Schema::default()),
        block_capacities(body, &Schema::default())
    );
}

#[test]
fn a_direct_parse_of_a_directive_header_fails_at_the_at_sign() {
    // `parse` lexes from byte 0 and does not read directives, so the header is a parse error.
    let source = format!("@!pure\n{GRAPH}");
    let capacities = block_capacities(&source, &Schema::default());
    let error = match parse(&source, &Schema::default(), &capacities) {
        Err(error) => error,
        Ok(_) => panic!("expected `parse` to refuse the header"),
    };
    assert_eq!(error.offset, 0);
}

#[test]
fn a_lexer_started_at_the_body_gives_absolute_offsets() {
    let source = format!("@!pure\n\n{GRAPH}");
    let body = source.find("graph").unwrap();
    let from_body: Vec<(usize, TokenKind<'_>)> = Lexer::at(&source, body)
        .map(|token| token.unwrap())
        .map(|token| (token.offset, token.kind))
        .collect();
    let alone: Vec<(usize, TokenKind<'_>)> = Lexer::new(GRAPH)
        .map(|token| token.unwrap())
        .map(|token| (token.offset, token.kind))
        .collect();
    let shifted: Vec<(usize, TokenKind<'_>)> = alone
        .into_iter()
        .map(|(offset, kind)| (offset + body, kind))
        .collect();
    assert_eq!(from_body, shifted);
}
