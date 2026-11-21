//! Pure files: `enum`, `input`, typed `choose`, closed `when` tests, and the diagnostics for each.
//! Expected values are written from the spec, not copied from the parser's output.

use graphite::graph::{CompiledGraph, Test, Ty};
use graphite::kind::Kind;
use graphite::parser::{ParseError, Schema};
use graphite::pipeline::compile;

fn pure(source: &str) -> Vec<CompiledGraph> {
    let schema = Schema {
        pure: true,
        ..Schema::default()
    };
    match compile(source, &schema) {
        Ok(graphs) => graphs,
        Err(error) => panic!("expected the pure file to parse: {}", error.message),
    }
}

fn refused(source: &str) -> ParseError {
    refused_with(source, &Schema::default())
}

fn refused_pure(source: &str) -> ParseError {
    refused_with(
        source,
        &Schema {
            pure: true,
            ..Schema::default()
        },
    )
}

fn refused_with(source: &str, schema: &Schema<'_>) -> ParseError {
    match compile(source, schema) {
        Err(error) => error,
        Ok(_) => panic!("expected the source to be refused"),
    }
}

/// The text of each name in a span of `graph.strings`, in order.
fn names(
    graph: &CompiledGraph,
    ids: impl Iterator<Item = graphite::interner::StringId>,
) -> Vec<String> {
    ids.map(|id| graph.text(id).to_owned()).collect()
}

const TACTIC: &str = "\
@!pure
graph combat @owner:ai
  default @log:off
  enum Tactic flee hit hold @ui:icon
  enum Dir @axis:compass
    north @deg:0
    east  @deg:90

  input homeEast : bool
  input lowHealth : bool @source:stats
  input dir : Dir

  choose tactic : Tactic @tick:fast
    when lowHealth -> flee @anim:run @log:on
    when homeEast  -> hit  @anim:swing
    else           -> hold

  choose facing : Dir
    when dir:east -> east
    else          -> north
";

#[test]
fn a_pure_file_builds_a_pure_graph_with_its_declarations() {
    let graphs = pure(TACTIC);
    let graph = &graphs[0];
    assert_eq!(graph.kind, Kind::Pure);
    assert_eq!(graph.text(graph.name), "combat");
    assert!(graph.nodes.is_empty());
    assert_eq!(
        names(graph, graph.enums.iter().map(|e| e.name)),
        ["Tactic", "Dir"]
    );
    assert_eq!(
        names(graph, graph.inputs.iter().map(|i| i.name)),
        ["homeEast", "lowHealth", "dir"]
    );
    assert_eq!(
        graph.inputs.iter().map(|i| i.ty).collect::<Vec<_>>(),
        [Ty::Bool, Ty::Bool, Ty::Enum(1)]
    );
}

#[test]
fn a_one_line_enum_takes_its_trailing_annotations_not_its_last_member() {
    let graphs = pure(TACTIC);
    let graph = &graphs[0];
    let tactic = &graph.enums[0];
    assert_eq!(
        names(
            graph,
            graph.members[index(&tactic.members)].iter().map(|m| m.name)
        ),
        ["flee", "hit", "hold"]
    );
    assert_eq!(annotations_of(graph, &tactic.annotations), ["ui:icon"]);
    for member in &graph.members[index(&tactic.members)] {
        assert!(annotations_of(graph, &member.annotations).is_empty());
    }
}

#[test]
fn a_block_enum_puts_annotations_on_its_head_and_its_members() {
    let graphs = pure(TACTIC);
    let graph = &graphs[0];
    let dir = &graph.enums[1];
    assert_eq!(annotations_of(graph, &dir.annotations), ["axis:compass"]);
    let members = &graph.members[index(&dir.members)];
    assert_eq!(
        names(graph, members.iter().map(|m| m.name)),
        ["north", "east"]
    );
    assert_eq!(annotations_of(graph, &members[0].annotations), ["deg:0"]);
    assert_eq!(annotations_of(graph, &members[1].annotations), ["deg:90"]);
}

#[test]
fn a_bool_test_is_a_flag_and_an_enum_test_names_the_member_by_position() {
    let graphs = pure(TACTIC);
    let graph = &graphs[0];
    assert_eq!(graph.tests, [Test::Flag(1), Test::Flag(0), Test::Is(2, 1),]);
    assert_eq!(
        names(graph, graph.conditions.iter().copied()),
        ["lowHealth", "homeEast", "dir:east"]
    );
}

#[test]
fn choices_keep_their_enum_and_their_annotations() {
    let graphs = pure(TACTIC);
    let graph = &graphs[0];
    let tactic = &graph.choices[0];
    assert_eq!(tactic.ty, Some(0));
    assert_eq!(annotations_of(graph, &tactic.annotations), ["tick:fast"]);
    assert_eq!(graph.choices[1].ty, Some(1));
}

#[test]
fn an_arm_keeps_its_own_annotations_and_its_tag() {
    let graphs = pure(TACTIC);
    let graph = &graphs[0];
    let arms = &graph.arms[index(&graph.choices[0].arms)];
    assert_eq!(graph.text(arms[0].tag), "flee");
    assert_eq!(
        annotations_of(graph, &arms[0].annotations),
        ["anim:run", "log:on"]
    );
    assert_eq!(graph.text(arms[2].tag), "hold");
    assert_eq!(arms[2].condition, None);
}

#[test]
fn default_applies_to_every_arm_that_does_not_set_the_key() {
    let graphs = pure(TACTIC);
    let graph = &graphs[0];
    let arms = &graph.arms[index(&graph.choices[1].arms)];
    // `default @log:off` came first, so both arms of the second choose carry it.
    assert_eq!(annotations_of(graph, &arms[0].annotations), ["log:off"]);
    // The first choose's first arm sets `log` itself, so its own value wins.
    let first = &graph.arms[index(&graph.choices[0].arms)];
    assert_eq!(
        annotations_of(graph, &first[0].annotations),
        ["anim:run", "log:on"]
    );
    // Own annotations come first; a default the arm does not set is appended after them.
    assert_eq!(
        annotations_of(graph, &first[1].annotations),
        ["anim:swing", "log:off"]
    );
}

#[test]
fn two_arms_with_the_same_tag_keep_their_own_annotations() {
    let graphs = pure(
        "@!pure\n\
         graph g\n\
         \x20 enum E a b\n\
         \x20 input x : bool\n\
         \x20 choose c : E\n\
         \x20   when x -> a @n:one\n\
         \x20   else -> a @n:two\n",
    );
    let graph = &graphs[0];
    let arms = &graph.arms[index(&graph.choices[0].arms)];
    assert_eq!(annotations_of(graph, &arms[0].annotations), ["n:one"]);
    assert_eq!(annotations_of(graph, &arms[1].annotations), ["n:two"]);
}

#[test]
fn the_same_member_name_may_appear_in_two_enums() {
    let graphs = pure(
        "@!pure\n\
         graph g\n\
         \x20 enum A x y\n\
         \x20 enum B x z\n",
    );
    assert_eq!(graphs[0].enums.len(), 2);
}

#[test]
fn a_pure_graph_with_no_node_is_accepted() {
    let graphs = pure(
        "@!pure\ngraph g\n  enum E a\n  input x : bool\n  choose c : E\n    when x -> a\n    else -> a\n",
    );
    assert_eq!(graphs[0].nodes.len(), 0);
}

#[test]
fn a_machine_graph_has_no_pure_data() {
    let graphs = compile("graph door\n  node a end\n", &Schema::default()).unwrap();
    let graph = &graphs[0];
    assert_eq!(graph.kind, Kind::Machine);
    assert!(graph.enums.is_empty() && graph.members.is_empty() && graph.inputs.is_empty());
    assert!(graph.tests.is_empty());
    assert!(graph.choices.is_empty());
}

// ---- Diagnostics: each message, at its line.

#[test]
fn a_machine_file_refuses_input_and_enum_with_the_pure_hint() {
    let error = refused("graph g\n  node a end\n  input x : bool\n");
    assert_eq!(
        error.message,
        Kind::Machine
            .allows(graphite::kind::Line::Input)
            .unwrap_err()
    );
    assert_eq!(error.offset, "graph g\n  node a end\n".len() + 2);
}

#[test]
fn a_pure_file_refuses_nodes_and_edges() {
    let error = refused_pure("@!pure\ngraph g\n  node a end\n");
    assert_eq!(error.message, "`node` is not allowed in a pure file");
    assert_eq!(error.offset, "@!pure\ngraph g\n  ".len());
}

#[test]
fn a_duplicate_enum_name_is_refused_at_its_second_line() {
    let source = "@!pure\ngraph g\n  enum E a\n  enum E b\n";
    let error = refused_pure(source);
    assert_eq!(error.message, "enum name is already declared in this graph");
    assert_eq!(error.offset, source.rfind("E b").unwrap());
}

#[test]
fn a_duplicate_member_within_one_enum_is_refused() {
    let source = "@!pure\ngraph g\n  enum E a a\n";
    let error = refused_pure(source);
    assert_eq!(error.message, "member is already declared in this enum");
    assert_eq!(error.offset, source.rfind('a').unwrap());
}

#[test]
fn a_duplicate_input_name_is_refused() {
    let source = "@!pure\ngraph g\n  input x : bool\n  input x : bool\n";
    let error = refused_pure(source);
    assert_eq!(
        error.message,
        "input name is already declared in this graph"
    );
    assert_eq!(error.offset, source.rfind('x').unwrap());
}

#[test]
fn an_enum_with_no_members_is_refused() {
    let error = refused_pure("@!pure\ngraph g\n  enum E\n    \n");
    assert_eq!(error.message, "an enum needs at least one member");
}

#[test]
fn an_input_of_an_unknown_type_is_refused() {
    let source = "@!pure\ngraph g\n  input x : Missing\n";
    let error = refused_pure(source);
    assert_eq!(
        error.message,
        "unknown type; expected `bool` or an enum declared above this input"
    );
    assert_eq!(error.offset, source.find("Missing").unwrap());
}

#[test]
fn a_choose_without_an_enum_is_refused_in_a_pure_file() {
    let source = "@!pure\ngraph g\n  enum E a\n  input x : bool\n  choose c\n    when x -> a\n    else -> a\n";
    let error = refused_pure(source);
    assert_eq!(
        error.message,
        "a choose in a pure file needs `: ENUM` naming its enum"
    );
    assert_eq!(error.offset, source.find("c\n").unwrap());
}

#[test]
fn a_choose_naming_an_unknown_enum_is_refused() {
    let source = "@!pure\ngraph g\n  choose c : Nope\n    else -> a\n";
    let error = refused_pure(source);
    assert_eq!(
        error.message,
        "unknown enum; declare it with `enum` above this choose"
    );
    assert_eq!(error.offset, source.find("Nope").unwrap());
}

#[test]
fn a_when_naming_no_declared_input_is_refused_at_the_name() {
    let source = "@!pure\ngraph g\n  enum E a\n  input x : bool\n  choose c : E\n    when lowHelth -> a\n    else -> a\n";
    let error = refused_pure(source);
    assert_eq!(
        error.message,
        "unknown input; declare it with `input` above its `when`"
    );
    assert_eq!(error.offset, source.find("lowHelth").unwrap());
}

#[test]
fn a_bool_input_with_a_member_is_refused() {
    let source = "@!pure\ngraph g\n  enum E a\n  input x : bool\n  choose c : E\n    when x:a -> a\n    else -> a\n";
    let error = refused_pure(source);
    assert_eq!(
        error.message,
        "`:MEMBER` tests an enum input; this input is a bool"
    );
    assert_eq!(error.offset, source.find("x:a").unwrap());
}

#[test]
fn an_enum_input_without_a_member_is_refused() {
    let source = "@!pure\ngraph g\n  enum E a\n  input x : E\n  choose c : E\n    when x -> a\n    else -> a\n";
    let error = refused_pure(source);
    assert_eq!(
        error.message,
        "an enum input needs `:MEMBER` after its name"
    );
    assert_eq!(error.offset, source.find("when x").unwrap() + 5);
}

#[test]
fn a_member_that_is_not_in_the_enum_is_refused_at_the_member() {
    let source = "@!pure\ngraph g\n  enum E a\n  input x : E\n  choose c : E\n    when x:up -> a\n    else -> a\n";
    let error = refused_pure(source);
    assert_eq!(error.message, "not a member of this enum");
    assert_eq!(error.offset, source.find("up").unwrap());
}

#[test]
fn a_tag_outside_the_choose_enum_is_refused_at_the_tag() {
    let source = "@!pure\ngraph g\n  enum E a\n  enum F b\n  input x : bool\n  choose c : E\n    when x -> b\n    else -> a\n";
    let error = refused_pure(source);
    assert_eq!(
        error.message,
        "the arm's tag must be a member of the choose's enum"
    );
    assert_eq!(error.offset, source.find("-> b").unwrap() + 3);
}

#[test]
fn a_choose_without_an_else_arm_is_refused_at_the_choose() {
    let source = "@!pure\ngraph g\n  enum E a\n  input x : bool\n  choose c : E\n    when x -> a\n";
    let error = refused_pure(source);
    assert_eq!(error.message, "choose needs an `else` arm in a pure file");
    assert_eq!(error.offset, source.find("c : E").unwrap());
}

#[test]
fn an_input_line_in_a_machine_file_names_the_pure_fix() {
    let error = refused("graph g\n  node a end\n  enum E x\n");
    assert!(error.message.starts_with("`enum` needs a pure file"));
}

fn index(span: &graphite::graph::Span) -> std::ops::Range<usize> {
    graphite::graph::index(span)
}

fn annotations_of(graph: &CompiledGraph, span: &graphite::graph::Span) -> Vec<String> {
    graph.annotations[index(span)]
        .iter()
        .map(|entry| match entry.value {
            Some(value) => format!("{}:{}", graph.text(entry.key), graph.text(value)),
            None => graph.text(entry.key).to_owned(),
        })
        .collect()
}

const SMALL_PURE: &str =
    "graph g\n  enum E a\n  input x : bool\n  choose c : E\n    when x -> a\n    else -> a\n";

#[test]
fn a_pure_directive_alone_makes_the_graph_pure() {
    let source = format!("@!pure\n{SMALL_PURE}");
    let graphs = compile(&source, &Schema::default()).unwrap();
    assert_eq!(graphs[0].kind, Kind::Pure);
}

#[test]
fn frontend_pure_is_a_pure_signal_too() {
    let source = format!("@!frontend:pure\n{SMALL_PURE}");
    let graphs = compile(&source, &Schema::default()).unwrap();
    assert_eq!(graphs[0].kind, Kind::Pure);
}

#[test]
fn a_pure_schema_refuses_a_machine_node() {
    let error = refused_pure("graph door\n  node a end\n");
    assert_eq!(error.message, "`node` is not allowed in a pure file");
}
