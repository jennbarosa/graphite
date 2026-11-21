pub(crate) fn graph() -> CompiledGraph {
    const STRINGS: &[&str] = &["directive", "line", "task", "rest", "done", "failed", "skipped", "body", "passed"];
    CompiledGraph {
        name: id(0),
        offset: 6,
        strings: interner(STRINGS, 48),
        nodes: vec![Node { name: id(1), looping: false, end: false, flags: 0..0, annotations: 0..1, outgoing: 0..4, sequence: None, offset: 23 }, Node { name: id(0), looping: false, end: false, flags: 0..0, annotations: 1..2, outgoing: 4..6, sequence: None, offset: 51 }, Node { name: id(3), looping: false, end: false, flags: 0..0, annotations: 2..3, outgoing: 6..8, sequence: None, offset: 84 }, Node { name: id(4), looping: false, end: true, flags: 0..0, annotations: 3..3, outgoing: 8..8, sequence: None, offset: 112 }, Node { name: id(5), looping: false, end: true, flags: 0..0, annotations: 3..3, outgoing: 8..8, sequence: None, offset: 133 }],
        edges: vec![Edge { source: 0, target: 0, condition: Some(0), annotations: 3..3 }, Edge { source: 0, target: 1, condition: Some(1), annotations: 3..3 }, Edge { source: 0, target: 2, condition: Some(2), annotations: 3..3 }, Edge { source: 0, target: 4, condition: Some(3), annotations: 3..3 }, Edge { source: 1, target: 0, condition: Some(4), annotations: 3..3 }, Edge { source: 1, target: 4, condition: Some(3), annotations: 3..3 }, Edge { source: 2, target: 3, condition: Some(4), annotations: 3..3 }, Edge { source: 2, target: 4, condition: Some(3), annotations: 3..3 }],
        outgoing: vec![0, 1, 2, 3, 4, 5, 6, 7],
        conditions: vec![id(6), id(0), id(7), id(5), id(8)],
        choices: vec![],
        arms: vec![],
        annotations: vec![Annotation { key: id(2), value: Some(id(1)) }, Annotation { key: id(2), value: Some(id(0)) }, Annotation { key: id(2), value: Some(id(3)) }],
        flags: vec![],
        graph_annotations: 0..0,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Task {
    Line,
    Directive,
    Rest,
}

pub(crate) fn task(name: &str) -> Option<Task> {
    Some(match name {
        "line" => Task::Line,
        "directive" => Task::Directive,
        "rest" => Task::Rest,
        _ => return None,
    })
}
