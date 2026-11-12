//! The lexer graph must produce exactly the tokens, and the first error, of the iterator lexer.

#[path = "support/rng.rs"]
mod rng;

use rng::Lcg;
use std::path::{Path, PathBuf};

use graphite::lexer::lex::lex;
use graphite::lexer::{LexError, Lexer, Token};

/// The iterator lexer's tokens up to its first error, and that error.
fn iterator(source: &str) -> (Vec<Token<'_>>, Option<LexError>) {
    let mut tokens = Vec::new();
    for item in Lexer::new(source) {
        match item {
            Ok(token) => tokens.push(token),
            Err(error) => return (tokens, Some(error)),
        }
    }
    (tokens, None)
}

fn assert_same(source: &str) {
    let (graph_tokens, graph_error) = lex(source);
    let (iterator_tokens, iterator_error) = iterator(source);
    assert_eq!(
        graph_tokens, iterator_tokens,
        "tokens differ for {source:?}"
    );
    assert_eq!(graph_error, iterator_error, "errors differ for {source:?}");
}

/// Every `.graphite` file under `directory`, recursively.
fn dsl_files(directory: &Path, found: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            dsl_files(&path, found);
        } else if path.extension().is_some_and(|extension| extension == "graphite") {
            found.push(path);
        }
    }
}

#[test]
fn every_bundled_and_fixture_dsl_file_lexes_the_same() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    dsl_files(&root.join("examples"), &mut files);
    dsl_files(&root.join("tests/fixtures"), &mut files);
    assert!(files.len() > 10, "expected the example and fixture graphs");
    for file in files {
        let source = std::fs::read_to_string(&file).unwrap();
        assert_same(&source);
    }
}

#[test]
fn hand_picked_cases_lex_the_same() {
    let cases = [
        "",
        "   \n\n  \n",
        "graph a\n",
        "graph a",
        "graph a\r\n  node b end\r\n",
        "graph a\n  node b @x:y -> c\n",
        "a -> b\n",
        "@ : ->\n",
        "a-b c->d\n",
        "-x -> y\n",
        "graph a\n  node b\n    node c end\n  node d end\n",
        "graph a\n    node b end\n  node c end\n",
        "graph a\n\tnode b end\n",
        "graph a\n  node b\n    node c\n  node d\n",
        "graph a\n  node b\n     node c end\n   node d end\n",
        "a\n\n\n   b\n",
        "x\n  y\n    z\n",
        "\r\r\n\n",
        "word\u{00e9} \u{4e2d}\n",
    ];
    for case in cases {
        assert_same(case);
    }
}

#[test]
fn random_inputs_lex_the_same() {
    const ALPHABET: &[&str] = &[
        "a", "b", " ", " ", "  ", "\n", "\r", "\r\n", "\t", "@", ":", "-", ">", "-> ", "é",
    ];
    let mut random = Lcg::new(7);
    for _ in 0..3000 {
        let length = random.below(40);
        let source: String = (0..length)
            .map(|_| ALPHABET[random.below(ALPHABET.len())])
            .collect();
        assert_same(&source);
    }
}
