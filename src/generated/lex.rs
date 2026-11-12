pub(crate) fn graph() -> CompiledGraph {
    const STRINGS: &[&str] = &["lex", "check_pending", "task", "pending", "start", "classify", "line_break", "indentation", "at", "colon", "arrow", "word", "tab_error", "finish", "done", "failed", "dedent", "idle", "eof", "newline", "tab", "line_start", "at_sign", "again", "indent", "same", "bad", "fail"];
    CompiledGraph {
        name: id(0),
        offset: 6,
        strings: interner(STRINGS, 164),
        nodes: vec![Node { name: id(1), looping: false, end: false, flags: 0..0, annotations: 0..1, outgoing: 0..2, sequence: None, offset: 17 }, Node { name: id(4), looping: false, end: false, flags: 0..0, annotations: 1..2, outgoing: 2..10, sequence: None, offset: 52 }, Node { name: id(6), looping: false, end: false, flags: 0..0, annotations: 2..3, outgoing: 10..11, sequence: None, offset: 88 }, Node { name: id(7), looping: false, end: false, flags: 0..0, annotations: 3..4, outgoing: 11..15, sequence: None, offset: 126 }, Node { name: id(8), looping: false, end: false, flags: 0..0, annotations: 4..5, outgoing: 15..16, sequence: None, offset: 165 }, Node { name: id(9), looping: false, end: false, flags: 0..0, annotations: 5..6, outgoing: 16..17, sequence: None, offset: 195 }, Node { name: id(10), looping: false, end: false, flags: 0..0, annotations: 6..7, outgoing: 17..18, sequence: None, offset: 228 }, Node { name: id(11), looping: false, end: false, flags: 0..0, annotations: 7..8, outgoing: 18..19, sequence: None, offset: 261 }, Node { name: id(12), looping: false, end: false, flags: 0..0, annotations: 8..9, outgoing: 19..20, sequence: None, offset: 293 }, Node { name: id(13), looping: false, end: false, flags: 0..0, annotations: 9..10, outgoing: 20..22, sequence: None, offset: 330 }, Node { name: id(14), looping: false, end: true, flags: 0..0, annotations: 10..10, outgoing: 22..22, sequence: None, offset: 364 }, Node { name: id(15), looping: false, end: true, flags: 0..0, annotations: 10..10, outgoing: 22..22, sequence: None, offset: 389 }],
        edges: vec![Edge { source: 0, target: 0, condition: Some(0), annotations: 10..10 }, Edge { source: 0, target: 1, condition: Some(1), annotations: 10..10 }, Edge { source: 1, target: 9, condition: Some(2), annotations: 10..10 }, Edge { source: 1, target: 2, condition: Some(3), annotations: 10..10 }, Edge { source: 1, target: 8, condition: Some(4), annotations: 10..10 }, Edge { source: 1, target: 3, condition: Some(5), annotations: 10..10 }, Edge { source: 1, target: 4, condition: Some(6), annotations: 10..10 }, Edge { source: 1, target: 5, condition: Some(7), annotations: 10..10 }, Edge { source: 1, target: 6, condition: Some(8), annotations: 10..10 }, Edge { source: 1, target: 7, condition: Some(9), annotations: 10..10 }, Edge { source: 2, target: 1, condition: Some(10), annotations: 10..10 }, Edge { source: 3, target: 1, condition: Some(11), annotations: 10..10 }, Edge { source: 3, target: 1, condition: Some(12), annotations: 10..10 }, Edge { source: 3, target: 0, condition: Some(0), annotations: 10..10 }, Edge { source: 3, target: 11, condition: Some(13), annotations: 10..10 }, Edge { source: 4, target: 1, condition: Some(10), annotations: 10..10 }, Edge { source: 5, target: 1, condition: Some(10), annotations: 10..10 }, Edge { source: 6, target: 1, condition: Some(10), annotations: 10..10 }, Edge { source: 7, target: 1, condition: Some(10), annotations: 10..10 }, Edge { source: 8, target: 11, condition: Some(14), annotations: 10..10 }, Edge { source: 9, target: 9, condition: Some(10), annotations: 10..10 }, Edge { source: 9, target: 10, condition: Some(15), annotations: 10..10 }],
        outgoing: vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21],
        conditions: vec![id(16), id(17), id(18), id(19), id(20), id(21), id(22), id(9), id(10), id(11), id(23), id(24), id(25), id(26), id(27), id(14)],
        choices: vec![],
        arms: vec![],
        annotations: vec![Annotation { key: id(2), value: Some(id(3)) }, Annotation { key: id(2), value: Some(id(5)) }, Annotation { key: id(2), value: Some(id(6)) }, Annotation { key: id(2), value: Some(id(7)) }, Annotation { key: id(2), value: Some(id(8)) }, Annotation { key: id(2), value: Some(id(9)) }, Annotation { key: id(2), value: Some(id(10)) }, Annotation { key: id(2), value: Some(id(11)) }, Annotation { key: id(2), value: Some(id(12)) }, Annotation { key: id(2), value: Some(id(13)) }],
        flags: vec![],
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Task {
    Pending,
    Classify,
    LineBreak,
    Indentation,
    At,
    Colon,
    Arrow,
    Word,
    TabError,
    Finish,
}

pub(crate) fn task(name: &str) -> Option<Task> {
    Some(match name {
        "pending" => Task::Pending,
        "classify" => Task::Classify,
        "line_break" => Task::LineBreak,
        "indentation" => Task::Indentation,
        "at" => Task::At,
        "colon" => Task::Colon,
        "arrow" => Task::Arrow,
        "word" => Task::Word,
        "tab_error" => Task::TabError,
        "finish" => Task::Finish,
        _ => return None,
    })
}
