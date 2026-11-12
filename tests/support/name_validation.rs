use std::cell::Cell;

use crate::parser::{Capacity, Schema, parse};

thread_local! {
    pub(super) static VALIDATIONS: Cell<usize> = const { Cell::new(0) };
}

#[test]
fn declared_node_certifies_condition_spelling() {
    let capacity = Capacity {
        nodes: 1,
        edges: 1,
        conditions: 1,
        strings: 2,
        string_bytes: 11,
        ..Capacity::default()
    };
    // The graph lexer parses its own description the first time it runs, and that parse validates
    // names too. Warm it up so the count below covers only this parse.
    let _ = crate::lexer::lex::GraphLexer::new("").next();
    VALIDATIONS.with(|count| count.set(0));
    let graphs = parse(
        "graph door\n  node request\n  edge request -> request when request\n",
        &Schema::default(),
        &[capacity],
    )
    .unwrap();
    assert_eq!(graphs[0].conditions, [graphs[0].nodes[0].name]);
    assert_eq!(VALIDATIONS.with(Cell::get), 2);
}
