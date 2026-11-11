use graphite::lexer::{LexErrorKind, Lexer, TokenKind};

fn kinds(source: &str) -> Vec<TokenKind<'_>> {
    Lexer::new(source)
        .map(|token| token.unwrap().kind)
        .collect()
}

#[test]
fn tokenizes_graph_nodes_edges_and_annotations_without_copying_text() {
    use TokenKind::*;

    let source = "graph door\n  node closed @anim:idle\n  edge closed -> opening when request";
    assert_eq!(
        kinds(source),
        [
            Word("graph"),
            Word("door"),
            Newline,
            Indent,
            Word("node"),
            Word("closed"),
            At,
            Word("anim"),
            Colon,
            Word("idle"),
            Newline,
            Word("edge"),
            Word("closed"),
            Arrow,
            Word("opening"),
            Word("when"),
            Word("request"),
            Newline,
            Dedent,
        ]
    );
    let token = Lexer::new(source).next().unwrap().unwrap();
    let Word(word) = token.kind else {
        panic!("expected word")
    };
    assert_eq!(word.as_ptr(), source.as_ptr());
    assert_eq!(token.offset, 0);
}

#[test]
fn nested_choose_blocks_dedent_before_the_next_graph() {
    use TokenKind::*;

    assert_eq!(
        kinds(
            "graph walk\n  choose facing\n    when left -> side @flip:true\n    else -> down\ngraph door\n  node closed end\n"
        ),
        [
            Word("graph"),
            Word("walk"),
            Newline,
            Indent,
            Word("choose"),
            Word("facing"),
            Newline,
            Indent,
            Word("when"),
            Word("left"),
            Arrow,
            Word("side"),
            At,
            Word("flip"),
            Colon,
            Word("true"),
            Newline,
            Word("else"),
            Arrow,
            Word("down"),
            Newline,
            Dedent,
            Dedent,
            Word("graph"),
            Word("door"),
            Newline,
            Indent,
            Word("node"),
            Word("closed"),
            Word("end"),
            Newline,
            Dedent,
        ]
    );
}

#[test]
fn blank_lines_crlf_and_trailing_spaces_do_not_change_blocks() {
    assert_eq!(
        kinds("\r\n  \r\ngraph door  \r\n  node closed end \r\n \r\n"),
        kinds("graph door\n  node closed end")
    );
    assert!(kinds("").is_empty());
    assert!(kinds("   \n\n").is_empty());
}

#[test]
fn utf8_words_have_byte_offsets_and_keep_opaque_punctuation() {
    let source = "@time.delay:é-1.5";
    let tokens: Vec<_> = Lexer::new(source).map(Result::unwrap).collect();
    let token = tokens
        .iter()
        .find(|token| token.kind == TokenKind::Word("é-1.5"))
        .unwrap();
    assert_eq!(token.offset, 12);
    assert_eq!(&source[token.offset..token.offset + "é-1.5".len()], "é-1.5");
    assert!(
        tokens
            .iter()
            .any(|token| token.kind == TokenKind::Word("time.delay"))
    );
}

#[test]
fn dedents_to_an_enclosing_block_and_closes_nested_blocks_at_eof() {
    use TokenKind::*;

    let source = "graph walk\n  choose facing\n    else -> down\n  choose facing\n    else -> down";
    let structure: Vec<_> = kinds(source)
        .into_iter()
        .filter(|kind| matches!(kind, Indent | Dedent))
        .collect();
    assert_eq!(structure, [Indent, Indent, Dedent, Indent, Dedent, Dedent]);
}

#[test]
fn invalid_indentation_reports_its_offset_and_stops() {
    for (source, kind, offset) in [
        ("graph door\n\tnode closed", LexErrorKind::Tab, 11),
        ("graph\tdoor", LexErrorKind::Tab, 5),
        (
            "graph door\n  node closed\n node open",
            LexErrorKind::Indentation,
            25,
        ),
    ] {
        let mut lexer = Lexer::new(source);
        let error = lexer.find_map(Result::err).unwrap();
        assert_eq!(error.kind, kind);
        assert_eq!(error.offset, offset);
        assert!(lexer.next().is_none());
        assert!(!error.to_string().is_empty());
        let _: &dyn std::error::Error = &error;
    }
}

#[test]
fn diagnostics_point_at_the_offending_source_text() {
    let cases = [
        (
            "graph door\n\tnode closed",
            "error: use spaces instead of tabs\n --> line 2, column 1\n  |\n2 |  node closed\n  | ^ use spaces instead of tabs",
        ),
        (
            "graph door\n  node closed\n node open",
            "error: dedent does not match an earlier indentation level\n --> line 3, column 1\n  |\n3 |  node open\n  |  ^^^^ dedent does not match an earlier indentation level",
        ),
    ];
    for (source, expected) in cases {
        let error = Lexer::new(source).find_map(Result::err).unwrap();
        assert_eq!(error.diagnostic(source).to_string(), expected);
    }
}
