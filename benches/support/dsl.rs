use std::collections::{BTreeSet, HashMap};
use std::fmt::Write;

use graphite::lexer::{Lexer, TokenKind};
use graphite::parser::Capacity;

/// Bounds owned by this generator, computed from its inputs rather than DSL tokens.
#[allow(dead_code)]
pub fn graph_capacity(names: &[String]) -> Capacity {
    let nodes = names.len();
    let full_choices = nodes.div_ceil(5);
    let partial_choices = (nodes + 1) / 5;
    let constants = [
        "walk",
        "anim",
        "idle",
        "time.delay",
        "250",
        "sfx.latch",
        "latch",
        "sfx",
        "step",
        "side",
        "flip",
        "true",
        "up",
        "down",
        "none",
        "false",
    ];
    Capacity {
        nodes,
        edges: nodes + nodes / 2,
        conditions: nodes,
        choices: full_choices + partial_choices,
        arms: full_choices * 4 + partial_choices * 2,
        annotations: nodes
            + nodes / 3 * 2
            + usize::from(nodes % 3 == 2)
            + (nodes + 1) / 3
            + full_choices * 3
            + partial_choices,
        flags: 0,
        strings: nodes + constants.len(),
        string_bytes: names.iter().map(String::len).sum::<usize>()
            + constants.iter().map(|word| word.len()).sum::<usize>(),
    }
}

/// Smallest graph where every construct appears: choose at stage 0, a no-`else` choose at
/// stage 3, `end` on the last stage, and two gates on stage 0.
pub const MIN_GRAPH_NODES: usize = 4;

/// Every language feature the generated DSL must contain.
pub const FEATURES: &[&str] = &[
    "multiple graphs",
    "graph",
    "node",
    "loop flag",
    "end flag",
    "edge",
    "sequence edge",
    "gated edge (when)",
    "multiple gates on one node",
    "edge annotation",
    "keyed annotation",
    "dotted key",
    "bare annotation",
    "choose",
    "choose arm",
    "choose arm annotation",
    "else arm",
    "choose without else",
    "arrow",
    "at",
    "colon",
    "newline",
    "indent",
    "dedent",
];

/// One graph covering every construct. Nodes come first, then edges, then choices, per the
/// grammar. Every non-`end` node gets exactly one sequence edge, and the last node is `end`.
/// Panics on fewer than `MIN_GRAPH_NODES` names, because smaller graphs cannot hold every
/// construct.
pub fn graph_source(names: &[String]) -> String {
    let count = names.len();
    assert!(
        count >= MIN_GRAPH_NODES,
        "a graph needs at least {MIN_GRAPH_NODES} nodes"
    );
    let last = count - 1;
    let at = |offset: usize| &names[(offset).min(last)];

    let mut source = String::from("graph walk\n");
    for (index, name) in names.iter().enumerate() {
        let flags = match (index % 3 == 1, index == last) {
            (true, true) => " loop end",
            (true, false) => " loop",
            (false, true) => " end",
            (false, false) => "",
        };
        let annotation = match index % 3 {
            0 => "@anim:idle",
            1 => "@time.delay:250 @sfx.latch",
            _ => "@anim:walk @latch",
        };
        writeln!(source, "  node {name}{flags} {annotation}").unwrap();
    }

    for (index, name) in names.iter().enumerate().take(last) {
        if index % 2 == 0 {
            writeln!(source, "  edge {name} -> {} when {name}", at(index + 2)).unwrap();
        }
        if index == 0 {
            writeln!(source, "  edge {name} -> {} when {}", at(3), at(1)).unwrap();
        }
        let annotation = if index % 3 == 0 { " @sfx:step" } else { "" };
        writeln!(source, "  edge {name} -> {}{annotation}", at(index + 1)).unwrap();
    }

    for (index, name) in names.iter().enumerate() {
        match index % 5 {
            0 => {
                writeln!(source, "  choose {name}").unwrap();
                writeln!(source, "    when {name} -> side @flip:true").unwrap();
                writeln!(source, "    when {} -> side", at(index + 1)).unwrap();
                writeln!(source, "    when {} -> up @anim:up", at(index + 2)).unwrap();
                writeln!(source, "    else -> down @sfx:none").unwrap();
            }
            3 => {
                writeln!(source, "  choose {name}").unwrap();
                writeln!(source, "    when {name} -> side").unwrap();
                writeln!(source, "    when {} -> up @flip:false", at(index + 1)).unwrap();
            }
            _ => {}
        }
    }
    source
}

/// Features of `source` that are absent, empty when every construct is covered.
pub fn missing_features(source: &str) -> Vec<&'static str> {
    let found = found_features(source);
    FEATURES
        .iter()
        .copied()
        .filter(|feature| !found.contains(feature))
        .collect()
}

/// Classifies each line by its leading keyword, so structure (sequence vs gated edges,
/// else-less chooses, repeated gates) is read from the grammar, not from bare keywords.
fn found_features(source: &str) -> BTreeSet<&'static str> {
    let tokens: Vec<TokenKind> = Lexer::new(source)
        .map(|token| token.unwrap().kind)
        .collect();
    let mut found = BTreeSet::new();
    for kind in &tokens {
        found.insert(match kind {
            TokenKind::Arrow => "arrow",
            TokenKind::At => "at",
            TokenKind::Colon => "colon",
            TokenKind::Newline => "newline",
            TokenKind::Indent => "indent",
            TokenKind::Dedent => "dedent",
            TokenKind::Word(_) => continue,
        });
    }

    let mut graphs = 0;
    let mut gates_per_node: HashMap<&str, usize> = HashMap::new();
    // Some(has_else) while inside a choose block.
    let mut choose_has_else: Option<bool> = None;

    for line in tokens.split(|kind| *kind == TokenKind::Newline) {
        let body: Vec<TokenKind> = line
            .iter()
            .copied()
            .filter(|kind| !matches!(kind, TokenKind::Indent | TokenKind::Dedent))
            .collect();
        let Some(TokenKind::Word(head)) = body.first().copied() else {
            continue;
        };
        for (index, kind) in body.iter().enumerate() {
            if *kind == TokenKind::At {
                annotation(&body[index + 1..], &mut found);
            }
        }
        let is_arm = matches!(head, "when" | "else");
        if !is_arm {
            close_choose(&mut found, choose_has_else.take());
        }
        match head {
            "graph" => {
                found.insert("graph");
                graphs += 1;
            }
            "node" => {
                found.insert("node");
                let flags = body.iter().take_while(|kind| **kind != TokenKind::At);
                for flag in flags {
                    match flag {
                        TokenKind::Word("loop") => found.insert("loop flag"),
                        TokenKind::Word("end") => found.insert("end flag"),
                        _ => false,
                    };
                }
            }
            "edge" => {
                found.insert("edge");
                if body.contains(&TokenKind::Word("when")) {
                    found.insert("gated edge (when)");
                    if let Some(TokenKind::Word(source)) = body.get(1) {
                        *gates_per_node.entry(source).or_default() += 1;
                    }
                } else {
                    found.insert("sequence edge");
                }
                if body.contains(&TokenKind::At) {
                    found.insert("edge annotation");
                }
            }
            "choose" => {
                found.insert("choose");
                choose_has_else = Some(false);
            }
            "when" => {
                found.insert("choose arm");
                if body.contains(&TokenKind::At) {
                    found.insert("choose arm annotation");
                }
            }
            "else" => {
                found.insert("else arm");
                choose_has_else = Some(true);
            }
            _ => {}
        }
    }
    close_choose(&mut found, choose_has_else);

    if graphs >= 2 {
        found.insert("multiple graphs");
    }
    if gates_per_node.values().any(|&gates| gates >= 2) {
        found.insert("multiple gates on one node");
    }
    found
}

/// Flags a choose block that ended without an `else` arm.
fn close_choose(found: &mut BTreeSet<&'static str>, has_else: Option<bool>) {
    if has_else == Some(false) {
        found.insert("choose without else");
    }
}

/// Records what one annotation (the tokens after its `@`) uses: a dotted key, and whether
/// it carries a `:VALUE`.
fn annotation(after_at: &[TokenKind], found: &mut BTreeSet<&'static str>) {
    let Some(TokenKind::Word(key)) = after_at.first() else {
        return;
    };
    if key.contains('.') {
        found.insert("dotted key");
    }
    if after_at.get(1) == Some(&TokenKind::Colon) {
        found.insert("keyed annotation");
    } else {
        found.insert("bare annotation");
    }
}
