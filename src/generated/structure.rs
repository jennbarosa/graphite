pub(crate) fn graph() -> CompiledGraph {
    const STRINGS: &[&str] = &["structure", "degrees", "task", "offsets", "reach", "routes", "done", "failed", "passed"];
    CompiledGraph {
        name: id(0),
        offset: 6,
        strings: interner(STRINGS, 54),
        nodes: vec![Node { name: id(1), looping: false, end: false, flags: 0..0, annotations: 0..1, outgoing: 0..2, sequence: None, offset: 23 }, Node { name: id(3), looping: false, end: false, flags: 0..0, annotations: 1..2, outgoing: 2..4, sequence: None, offset: 52 }, Node { name: id(4), looping: false, end: false, flags: 0..0, annotations: 2..3, outgoing: 4..6, sequence: None, offset: 81 }, Node { name: id(5), looping: false, end: false, flags: 0..0, annotations: 3..4, outgoing: 6..8, sequence: None, offset: 108 }, Node { name: id(6), looping: false, end: true, flags: 0..0, annotations: 4..4, outgoing: 8..8, sequence: None, offset: 136 }, Node { name: id(7), looping: false, end: true, flags: 0..0, annotations: 4..4, outgoing: 8..8, sequence: None, offset: 155 }],
        edges: vec![Edge { source: 0, target: 1, condition: Some(0), annotations: 4..4 }, Edge { source: 0, target: 5, condition: Some(1), annotations: 4..4 }, Edge { source: 1, target: 2, condition: Some(0), annotations: 4..4 }, Edge { source: 1, target: 5, condition: Some(1), annotations: 4..4 }, Edge { source: 2, target: 3, condition: Some(0), annotations: 4..4 }, Edge { source: 2, target: 5, condition: Some(1), annotations: 4..4 }, Edge { source: 3, target: 4, condition: Some(0), annotations: 4..4 }, Edge { source: 3, target: 5, condition: Some(1), annotations: 4..4 }],
        outgoing: vec![0, 1, 2, 3, 4, 5, 6, 7],
        conditions: vec![id(8), id(7)],
        choices: vec![],
        arms: vec![],
        annotations: vec![Annotation { key: id(2), value: Some(id(1)) }, Annotation { key: id(2), value: Some(id(3)) }, Annotation { key: id(2), value: Some(id(4)) }, Annotation { key: id(2), value: Some(id(5)) }],
        flags: vec![],
        graph_annotations: 0..0,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Task {
    Degrees,
    Offsets,
    Reach,
    Routes,
}

pub(crate) fn task(name: &str) -> Option<Task> {
    Some(match name {
        "degrees" => Task::Degrees,
        "offsets" => Task::Offsets,
        "reach" => Task::Reach,
        "routes" => Task::Routes,
        _ => return None,
    })
}
