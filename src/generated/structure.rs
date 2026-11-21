pub(crate) fn graph() -> CompiledGraph {
    const STRINGS: &[&str] = &["structure", "kind", "task", "degrees", "offsets", "reach", "routes", "totality", "done", "failed", "machine", "pure", "passed"];
    CompiledGraph {
        name: id(0),
        offset: 6,
        kind: Kind::Machine,
        strings: interner(STRINGS, 77),
        nodes: vec![Node { name: id(1), looping: false, end: false, flags: 0..0, annotations: 0..1, outgoing: 0..3, sequence: None, offset: 23 }, Node { name: id(3), looping: false, end: false, flags: 0..0, annotations: 1..2, outgoing: 3..5, sequence: None, offset: 51 }, Node { name: id(4), looping: false, end: false, flags: 0..0, annotations: 2..3, outgoing: 5..7, sequence: None, offset: 82 }, Node { name: id(5), looping: false, end: false, flags: 0..0, annotations: 3..4, outgoing: 7..9, sequence: None, offset: 113 }, Node { name: id(6), looping: false, end: false, flags: 0..0, annotations: 4..5, outgoing: 9..11, sequence: None, offset: 142 }, Node { name: id(7), looping: false, end: false, flags: 0..0, annotations: 5..6, outgoing: 11..13, sequence: None, offset: 172 }, Node { name: id(8), looping: false, end: true, flags: 0..0, annotations: 6..6, outgoing: 13..13, sequence: None, offset: 204 }, Node { name: id(9), looping: false, end: true, flags: 0..0, annotations: 6..6, outgoing: 13..13, sequence: None, offset: 225 }],
        edges: vec![Edge { source: 0, target: 1, condition: Some(0), annotations: 6..6 }, Edge { source: 0, target: 5, condition: Some(1), annotations: 6..6 }, Edge { source: 0, target: 7, condition: Some(2), annotations: 6..6 }, Edge { source: 1, target: 2, condition: Some(3), annotations: 6..6 }, Edge { source: 1, target: 7, condition: Some(2), annotations: 6..6 }, Edge { source: 2, target: 3, condition: Some(3), annotations: 6..6 }, Edge { source: 2, target: 7, condition: Some(2), annotations: 6..6 }, Edge { source: 3, target: 4, condition: Some(3), annotations: 6..6 }, Edge { source: 3, target: 7, condition: Some(2), annotations: 6..6 }, Edge { source: 4, target: 6, condition: Some(3), annotations: 6..6 }, Edge { source: 4, target: 7, condition: Some(2), annotations: 6..6 }, Edge { source: 5, target: 6, condition: Some(3), annotations: 6..6 }, Edge { source: 5, target: 7, condition: Some(2), annotations: 6..6 }],
        outgoing: vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12],
        conditions: vec![id(10), id(11), id(9), id(12)],
        tests: vec![],
        choices: vec![],
        arms: vec![],
        enums: vec![],
        members: vec![],
        inputs: vec![],
        annotations: vec![Annotation { key: id(2), value: Some(id(1)) }, Annotation { key: id(2), value: Some(id(3)) }, Annotation { key: id(2), value: Some(id(4)) }, Annotation { key: id(2), value: Some(id(5)) }, Annotation { key: id(2), value: Some(id(6)) }, Annotation { key: id(2), value: Some(id(7)) }],
        flags: vec![],
        graph_annotations: 0..0,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Task {
    Kind,
    Degrees,
    Offsets,
    Reach,
    Routes,
    Totality,
}

pub(crate) fn task(name: &str) -> Option<Task> {
    Some(match name {
        "kind" => Task::Kind,
        "degrees" => Task::Degrees,
        "offsets" => Task::Offsets,
        "reach" => Task::Reach,
        "routes" => Task::Routes,
        "totality" => Task::Totality,
        _ => return None,
    })
}
