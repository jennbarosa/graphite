use graphite::lexer::{Lexer, TokenKind};
use graphite::parser::{Schema, parse};

#[path = "../benches/support/dsl.rs"]
mod dsl;
#[path = "../benches/support/workload.rs"]
mod workload;

#[test]
fn producer_capacity_covers_every_generator_boundary() {
    for count in dsl::MIN_GRAPH_NODES..=64 {
        let names = workload::names(count, 42);
        let capacity = dsl::graph_capacity(&names);
        let source = dsl::graph_source(&names);
        let graphs = parse(&source, &Schema::default(), &[capacity]).unwrap();
        let graph = &graphs[0];
        assert_eq!(graph.nodes.len(), capacity.nodes);
        assert_eq!(graph.edges.len(), capacity.edges);
        assert_eq!(graph.choices.len(), capacity.choices);
        assert_eq!(graph.arms.len(), capacity.arms);
        assert_eq!(graph.annotations.len(), capacity.annotations);
    }
}

#[test]
fn generated_graph_lexes_without_errors_and_ends_with_dedent() {
    let source = dsl::graph_source(&workload::names(12, 42));
    let tokens: Vec<_> = Lexer::new(&source).map(Result::unwrap).collect();
    assert_eq!(tokens.first().unwrap().kind, TokenKind::Word("graph"));
    assert_eq!(tokens.last().unwrap().kind, TokenKind::Dedent);
}

#[test]
fn one_graph_covers_every_dsl_feature() {
    let source = dsl::graph_source(&workload::names(dsl::MIN_GRAPH_NODES, 42));
    assert_eq!(dsl::missing_features(&source), ["multiple graphs"]);
}

#[test]
fn two_graphs_cover_every_dsl_feature() {
    let names = workload::names(8, 42);
    let source: String = names.chunks(4).map(dsl::graph_source).collect();
    assert_eq!(dsl::missing_features(&source), Vec::<&str>::new());
}

#[test]
fn coverage_check_rejects_a_graph_without_choose_else() {
    let source =
        dsl::graph_source(&workload::names(8, 42)).replace("    else ->", "    when zz ->");
    assert_eq!(
        dsl::missing_features(&source),
        ["multiple graphs", "else arm"]
    );
}

#[test]
fn coverage_check_detects_a_missing_second_gate() {
    let names = workload::names(8, 42);
    let source = dsl::graph_source(&names);
    let gate_prefix = format!("  edge {} -> ", names[0]);
    let stripped: String = source
        .lines()
        .filter(|line| !(line.starts_with(&gate_prefix) && line.contains(" when ")))
        .map(|line| format!("{line}\n"))
        .collect();
    assert_eq!(
        dsl::missing_features(&stripped),
        ["multiple graphs", "multiple gates on one node"]
    );
}
