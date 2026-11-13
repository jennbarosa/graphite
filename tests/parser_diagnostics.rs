use graphite::parser::{ParseError, Schema};
use graphite::pipeline::compile;

fn rendered(source: &str) -> String {
    let error: ParseError = compile(source, &Schema::default()).err().unwrap();
    error.diagnostic(source).to_string()
}

#[test]
fn parse_errors_underline_the_offending_text() {
    let cases = [
        (
            "graph door\n  node open\n  node open\n",
            "error: node name is already declared in this graph\n --> line 3, column 8\n  |\n3 |   node open\n  |        ^^^^ node name is already declared in this graph",
        ),
        (
            "graph door\n  node a\n  wrong thing here\n",
            "error: expected `default`, `node`, `edge`, or `choose` line\n --> line 3, column 3\n  |\n3 |   wrong thing here\n  |   ^^^^^ expected `default`, `node`, `edge`, or `choose` line",
        ),
        (
            "graph door\n  node a\n  edge b -> c\n",
            "error: unknown node; declare it with `node` before linking to it\n --> line 3, column 8\n  |\n3 |   edge b -> c\n  |        ^ unknown node; declare it with `node` before linking to it",
        ),
        (
            // Columns count characters, not bytes: the caret stays under `->`.
            "graph door\n  node a\n  node é_bad -> a\n",
            "error: expected the end of the line\n --> line 3, column 14\n  |\n3 |   node é_bad -> a\n  |              ^^ expected the end of the line",
        ),
        (
            "graph door\n  node a\n  choose c\n    when x\n    -> t\n",
            "error: expected `->`\n --> line 4, column 11\n  |\n4 |     when x\n  |           ^ expected `->`",
        ),
        (
            "graph door\n\tnode a\n",
            "error: use spaces instead of tabs\n --> line 2, column 1\n  |\n2 |  node a\n  | ^ use spaces instead of tabs",
        ),
    ];
    for (source, expected) in cases {
        assert_eq!(rendered(source), expected, "source: {source:?}");
    }
}

#[test]
fn diagnostics_never_panic_at_end_of_input() {
    // The offset may sit at the end of the source with no trailing newline.
    let source = "graph door\n  node a -> ";
    let text = rendered(source);
    assert!(
        text.starts_with("error: expected the end of the line\n --> line 2, column 10"),
        "{text}"
    );
    let past_end = ParseError {
        message: "end",
        offset: source.len() + 10,
    };
    assert!(past_end.diagnostic(source).to_string().contains("line 2"));
}

#[test]
fn a_node_the_entry_cannot_reach_is_a_build_error_at_its_declaration() {
    // `dead` has an outgoing edge, so only reachability catches it.
    let source =
        "graph door\n  node a\n  node dead\n  edge a -> a when go\n  edge dead -> dead when go\n";
    let message = rendered(source);
    assert!(
        message.contains("node is unreachable from the entry node"),
        "{message}"
    );
    assert!(message.contains("line 3, column 8"), "{message}");
}
