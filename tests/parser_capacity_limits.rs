//! The caller's capacity hints bound what the parser stores. A hint that cannot be honored is
//! refused with a message that names the hint, before anything is parsed into it.

use graphite::parser::{Capacity, Schema, parse};

const TWO_NODES: &str = "graph g\n  node a\n  node b end\n  edge a -> b\n";

/// Generous hints for `TWO_NODES`, so one field can be changed at a time.
fn roomy() -> Capacity {
    Capacity {
        nodes: 8,
        edges: 8,
        conditions: 8,
        choices: 4,
        arms: 8,
        enums: 0,
        members: 0,
        inputs: 0,
        annotations: 8,
        flags: 0,
        strings: 32,
        string_bytes: 256,
    }
}

fn refused(capacity: Capacity, source: &str) -> String {
    let error = parse(source, &Schema::default(), &[capacity])
        .err()
        .expect("a refused capacity");
    error.to_string()
}

#[test]
fn roomy_hints_parse_the_graph() {
    assert!(parse(TWO_NODES, &Schema::default(), &[roomy()]).is_ok());
}

#[test]
fn a_count_that_does_not_fit_in_32_bits_is_refused() {
    let capacity = Capacity {
        nodes: u32::MAX as usize + 1,
        ..roomy()
    };
    assert!(refused(capacity, TWO_NODES).contains("capacity exceeds u32 storage"));
}

#[test]
fn a_buffer_too_large_to_reserve_is_refused() {
    let capacity = Capacity {
        nodes: u32::MAX as usize,
        ..roomy()
    };
    assert!(refused(capacity, TWO_NODES).contains("could not reserve memory for the graph"));
}

#[test]
fn a_string_table_too_large_to_reserve_is_refused() {
    let capacity = Capacity {
        strings: usize::MAX,
        ..roomy()
    };
    assert!(refused(capacity, TWO_NODES).contains("could not reserve memory for graph strings"));
}

#[test]
fn a_graph_name_that_does_not_fit_the_string_hint_is_refused() {
    let capacity = Capacity {
        strings: 0,
        ..roomy()
    };
    assert!(refused(capacity, TWO_NODES).contains("string capacity exhausted"));
}

#[test]
fn a_node_count_above_the_hint_is_refused_at_the_node() {
    let capacity = Capacity {
        nodes: 1,
        ..roomy()
    };
    let message = refused(capacity, TWO_NODES);
    assert!(
        message.contains("graph capacity exhausted; raise its count hints"),
        "{message}"
    );
}
