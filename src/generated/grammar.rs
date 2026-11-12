pub(crate) fn graph() -> CompiledGraph {
    const STRINGS: &[&str] = &["grammar", "next", "task", "header", "body", "node_line", "edge_line", "choose", "arms", "when_arm", "else_arm", "arm", "arms_end", "block_end", "graph_end", "finished", "failed", "more", "passed", "line_node", "line_edge", "line_choose", "line_end", "arm_when", "arm_else", "arm_end"];
    CompiledGraph {
        name: id(0),
        offset: 6,
        strings: interner(STRINGS, 182),
        nodes: vec![Node { name: id(1), looping: false, end: false, flags: 0..0, annotations: 0..1, outgoing: 0..3, sequence: None, offset: 21 }, Node { name: id(3), looping: false, end: false, flags: 0..0, annotations: 1..2, outgoing: 3..5, sequence: None, offset: 50 }, Node { name: id(4), looping: false, end: false, flags: 0..0, annotations: 2..3, outgoing: 5..10, sequence: None, offset: 81 }, Node { name: id(5), looping: false, end: false, flags: 0..0, annotations: 3..4, outgoing: 10..12, sequence: None, offset: 110 }, Node { name: id(6), looping: false, end: false, flags: 0..0, annotations: 4..5, outgoing: 12..14, sequence: None, offset: 144 }, Node { name: id(7), looping: false, end: false, flags: 0..0, annotations: 5..6, outgoing: 14..16, sequence: None, offset: 178 }, Node { name: id(8), looping: false, end: false, flags: 0..0, annotations: 6..7, outgoing: 16..20, sequence: None, offset: 209 }, Node { name: id(9), looping: false, end: false, flags: 0..0, annotations: 7..8, outgoing: 20..22, sequence: None, offset: 238 }, Node { name: id(10), looping: false, end: false, flags: 0..0, annotations: 8..9, outgoing: 22..23, sequence: None, offset: 271 }, Node { name: id(11), looping: false, end: false, flags: 0..0, annotations: 9..10, outgoing: 23..25, sequence: None, offset: 304 }, Node { name: id(12), looping: false, end: false, flags: 0..0, annotations: 10..11, outgoing: 25..27, sequence: None, offset: 332 }, Node { name: id(13), looping: false, end: false, flags: 0..0, annotations: 11..12, outgoing: 27..29, sequence: None, offset: 365 }, Node { name: id(14), looping: false, end: false, flags: 0..0, annotations: 12..13, outgoing: 29..31, sequence: None, offset: 399 }, Node { name: id(15), looping: false, end: true, flags: 0..0, annotations: 13..13, outgoing: 31..31, sequence: None, offset: 433 }, Node { name: id(16), looping: false, end: true, flags: 0..0, annotations: 13..13, outgoing: 31..31, sequence: None, offset: 455 }],
        edges: vec![Edge { source: 0, target: 1, condition: Some(0), annotations: 13..13 }, Edge { source: 0, target: 13, condition: Some(1), annotations: 13..13 }, Edge { source: 0, target: 14, condition: Some(2), annotations: 13..13 }, Edge { source: 1, target: 2, condition: Some(3), annotations: 13..13 }, Edge { source: 1, target: 14, condition: Some(2), annotations: 13..13 }, Edge { source: 2, target: 3, condition: Some(4), annotations: 13..13 }, Edge { source: 2, target: 4, condition: Some(5), annotations: 13..13 }, Edge { source: 2, target: 5, condition: Some(6), annotations: 13..13 }, Edge { source: 2, target: 11, condition: Some(7), annotations: 13..13 }, Edge { source: 2, target: 14, condition: Some(2), annotations: 13..13 }, Edge { source: 3, target: 2, condition: Some(3), annotations: 13..13 }, Edge { source: 3, target: 14, condition: Some(2), annotations: 13..13 }, Edge { source: 4, target: 2, condition: Some(3), annotations: 13..13 }, Edge { source: 4, target: 14, condition: Some(2), annotations: 13..13 }, Edge { source: 5, target: 6, condition: Some(3), annotations: 13..13 }, Edge { source: 5, target: 14, condition: Some(2), annotations: 13..13 }, Edge { source: 6, target: 7, condition: Some(8), annotations: 13..13 }, Edge { source: 6, target: 8, condition: Some(9), annotations: 13..13 }, Edge { source: 6, target: 10, condition: Some(10), annotations: 13..13 }, Edge { source: 6, target: 14, condition: Some(2), annotations: 13..13 }, Edge { source: 7, target: 9, condition: Some(3), annotations: 13..13 }, Edge { source: 7, target: 14, condition: Some(2), annotations: 13..13 }, Edge { source: 8, target: 9, condition: Some(3), annotations: 13..13 }, Edge { source: 9, target: 6, condition: Some(3), annotations: 13..13 }, Edge { source: 9, target: 14, condition: Some(2), annotations: 13..13 }, Edge { source: 10, target: 2, condition: Some(3), annotations: 13..13 }, Edge { source: 10, target: 14, condition: Some(2), annotations: 13..13 }, Edge { source: 11, target: 12, condition: Some(3), annotations: 13..13 }, Edge { source: 11, target: 14, condition: Some(2), annotations: 13..13 }, Edge { source: 12, target: 0, condition: Some(3), annotations: 13..13 }, Edge { source: 12, target: 14, condition: Some(2), annotations: 13..13 }],
        outgoing: vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30],
        conditions: vec![id(17), id(15), id(16), id(18), id(19), id(20), id(21), id(22), id(23), id(24), id(25)],
        choices: vec![],
        arms: vec![],
        annotations: vec![Annotation { key: id(2), value: Some(id(1)) }, Annotation { key: id(2), value: Some(id(3)) }, Annotation { key: id(2), value: Some(id(4)) }, Annotation { key: id(2), value: Some(id(5)) }, Annotation { key: id(2), value: Some(id(6)) }, Annotation { key: id(2), value: Some(id(7)) }, Annotation { key: id(2), value: Some(id(8)) }, Annotation { key: id(2), value: Some(id(9)) }, Annotation { key: id(2), value: Some(id(10)) }, Annotation { key: id(2), value: Some(id(11)) }, Annotation { key: id(2), value: Some(id(12)) }, Annotation { key: id(2), value: Some(id(13)) }, Annotation { key: id(2), value: Some(id(14)) }],
        flags: vec![],
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Task {
    Next,
    Header,
    Body,
    NodeLine,
    EdgeLine,
    Choose,
    Arms,
    WhenArm,
    ElseArm,
    Arm,
    ArmsEnd,
    BlockEnd,
    GraphEnd,
}

pub(crate) fn task(name: &str) -> Option<Task> {
    Some(match name {
        "next" => Task::Next,
        "header" => Task::Header,
        "body" => Task::Body,
        "node_line" => Task::NodeLine,
        "edge_line" => Task::EdgeLine,
        "choose" => Task::Choose,
        "arms" => Task::Arms,
        "when_arm" => Task::WhenArm,
        "else_arm" => Task::ElseArm,
        "arm" => Task::Arm,
        "arms_end" => Task::ArmsEnd,
        "block_end" => Task::BlockEnd,
        "graph_end" => Task::GraphEnd,
        _ => return None,
    })
}
