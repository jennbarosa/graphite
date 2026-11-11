use graphite::parser::{Capacity, Schema, parse};

#[test]
fn invalid_annotation_key_diagnostics_precede_string_capacity_errors() {
    let source = "graph door\n  node closed end @time..delay\n";
    let hint = Capacity {
        nodes: 1,
        annotations: 1,
        strings: 2,
        string_bytes: 10,
        ..Capacity::default()
    };
    let error = parse(source, &Schema::default(), &[hint]).err().unwrap();
    assert_eq!(
        error.message,
        "invalid annotation key: use letters, digits, `_`, or `-`, separated by `.`"
    );
    assert_eq!(error.offset, source.find("time..delay").unwrap());
}

#[test]
fn parser_reports_the_first_error_without_scanning_the_rest_of_the_source() {
    let source = "graph door\n  node closed end unknown\n\tnode open end\n";
    let capacity = Capacity {
        nodes: 1,
        strings: 2,
        string_bytes: 10,
        ..Capacity::default()
    };
    let error = parse(source, &Schema::default(), &[capacity])
        .err()
        .unwrap();
    assert_eq!(
        error.message,
        "unknown node flag; expected `loop`, `end`, or a flag from the schema"
    );
    assert_eq!(error.offset, source.find("unknown").unwrap());
}

const SOURCE: &str = "graph walk\n  node windup looping @anim:windup\n  node stopped end\n  edge windup -> stopped when keyup @sfx:latch\n  choose facing\n    when left -> side @flip:true\n    else -> down\n";

fn capacity() -> Capacity {
    let words = [
        "walk", "windup", "looping", "anim", "stopped", "keyup", "sfx", "latch", "facing", "left",
        "side", "flip", "true", "down",
    ];
    Capacity {
        nodes: 2,
        edges: 1,
        conditions: 2,
        choices: 1,
        arms: 2,
        annotations: 3,
        flags: 1,
        strings: words.len(),
        string_bytes: words.iter().map(|word| word.len()).sum(),
    }
}

#[test]
fn exact_capacity_compiles_all_storage_roles() {
    let schema = Schema {
        node_flags: &["looping"],
    };
    let graphs = parse(SOURCE, &schema, &[capacity()]).unwrap();
    let graph = &graphs[0];
    assert_eq!(graph.nodes.len(), 2);
    assert_eq!(graph.edges.len(), 1);
    assert_eq!(graph.conditions.len(), 2);
    assert_eq!(graph.choices.len(), 1);
    assert_eq!(graph.arms.len(), 2);
    assert_eq!(graph.annotations.len(), 3);
    assert_eq!(graph.flags.len(), 1);
    assert_eq!(graph.outgoing, [0]);
}

#[test]
fn underestimating_any_storage_role_returns_an_error() {
    let schema = Schema {
        node_flags: &["looping"],
    };
    for role in 0..9 {
        let mut hint = capacity();
        let bound = match role {
            0 => &mut hint.nodes,
            1 => &mut hint.edges,
            2 => &mut hint.conditions,
            3 => &mut hint.choices,
            4 => &mut hint.arms,
            5 => &mut hint.annotations,
            6 => &mut hint.flags,
            7 => &mut hint.strings,
            _ => &mut hint.string_bytes,
        };
        *bound -= 1;
        let error = parse(SOURCE, &schema, &[hint]).err().unwrap();
        assert!(error.message.contains("exhausted"), "role {role}: {error}");
    }
}

#[test]
fn each_graph_consumes_its_own_hint_and_unused_hints_are_allowed() {
    let first = "graph door\n  node closed end\n";
    let second = "graph walk\n  node stopped end\n";
    let hint = Capacity {
        nodes: 1,
        strings: 2,
        string_bytes: 16,
        ..Capacity::default()
    };
    let source = format!("{first}{second}");
    let error = parse(&source, &Schema::default(), &[hint]).err().unwrap();
    assert_eq!(
        error.message,
        "no capacity hint for this graph; pass one per graph"
    );
    assert_eq!(error.offset, source.find("walk").unwrap());
    let graphs = parse(&source, &Schema::default(), &[hint; 2]).unwrap();
    assert_eq!(graphs.len(), 2);
    assert_eq!(graphs[0].strings.resolve(graphs[0].name), Some("door"));
    assert_eq!(graphs[1].strings.resolve(graphs[1].name), Some("walk"));
    assert_eq!(
        parse(first, &Schema::default(), &[hint; 2]).unwrap().len(),
        1
    );
    assert_eq!(
        parse(first, &Schema::default(), &[]).err().unwrap().message,
        "no capacity hint for this graph; pass one per graph"
    );
}

#[test]
fn unrepresentable_capacities_fail_without_panicking() {
    for role in 0..9 {
        let mut hint = capacity();
        let bound = match role {
            0 => &mut hint.nodes,
            1 => &mut hint.edges,
            2 => &mut hint.conditions,
            3 => &mut hint.choices,
            4 => &mut hint.arms,
            5 => &mut hint.annotations,
            6 => &mut hint.flags,
            7 => &mut hint.strings,
            _ => &mut hint.string_bytes,
        };
        *bound = usize::MAX;
        let error = parse(
            SOURCE,
            &Schema {
                node_flags: &["looping"],
            },
            &[hint],
        )
        .err()
        .unwrap();
        if role < 7 {
            assert_eq!(
                error.message, "capacity exceeds u32 storage; counts must fit in 32 bits",
                "role {role}"
            );
        }
    }
}
