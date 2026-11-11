use graphite::lexer::{Lexer, Token};

#[path = "../benches/support/parallel.rs"]
mod parallel;

#[test]
fn parallel_preserves_tokens_offsets_and_first_error() {
    for source in [
        "",
        "   \r\n",
        "graph walk",
        "graph walk\n  node closed end",
        "\n graph walk\n  node closed end\n\ngraph door\n  choose facing\n    else -> down\r\n  node closed end\r\n\r\ngraph walk",
        "graph walk\r  node closed end\rgraph door\r  node closed end",
        "graph walk\n  node é-1.5\n \n\n graph door\nnode closed end\n",
        "graph walk\n  node closed end\n node closed\ngraph door\n\tnode closed",
        "graph walk\n\tnode closed\ngraph door\n\tnode closed",
    ] {
        let expected = Lexer::new(source).collect::<Result<Vec<Token<'_>>, _>>();
        for workers in [1, 2, 4, 16] {
            assert_eq!(
                parallel::tokenize(source, workers),
                expected,
                "{workers}: {source:?}"
            );
        }
    }
}

#[test]
fn parallel_matches_scalar_on_seeded_fragment_sequences() {
    let fragments = [
        "graph walk",
        "  node closed end",
        "    else -> down",
        "\tnode closed",
        " node closed",
        "",
        "@time.delay:é-1.5",
    ];
    let mut state = 42u64;
    for _ in 0..200 {
        let mut source = String::new();
        for _ in 0..32 {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            source.push_str(fragments[(state >> 32) as usize % fragments.len()]);
            source.push_str(["\n", "\r", "\r\n"][(state >> 16) as usize % 3]);
        }
        assert_eq!(
            parallel::tokenize(&source, 4),
            Lexer::new(&source).collect::<Result<Vec<_>, _>>()
        );
    }
}
