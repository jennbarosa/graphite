//! Merged test file. Each section below was `tests/<name>.rs` before; see git history.

// Merged from `tests/lexer.rs`.
mod lexer {

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

        let source =
            "graph walk\n  choose facing\n    else -> down\n  choose facing\n    else -> down";
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
}

// Merged from `tests/lexer_paths.rs`.
mod lexer_paths {
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
}
