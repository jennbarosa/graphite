//! Each parser error the author can cause, checked through `graphite check --json`: the message
//! must name the problem and the fix. Every case is a small graph with one mistake.

#[allow(dead_code)]
#[path = "support/cli.rs"]
mod cli;

use cli::{dsl, graphite, stdout};

/// (case name, source, pure, the message the check must print)
const CASES: &[(&str, &str, bool, &str)] = &[
    (
        "node_without_a_name",
        "graph g\n  node\n",
        false,
        "expected a node name after `node`",
    ),
    (
        "edge_arrow_without_a_target",
        "graph g\n  node a\n  edge a ->\n",
        false,
        "expected a node name after `->` or `edge`",
    ),
    (
        "when_without_a_condition_on_an_edge",
        "graph g\n  node a\n  edge a -> a when\n",
        false,
        "expected a condition name after `when`",
    ),
    (
        "when_without_a_condition_on_an_arm",
        "graph g\n  node a\n  choose c\n    when -> a\n",
        false,
        "expected a condition name after `when`",
    ),
    (
        "choose_without_a_name",
        "graph g\n  node a\n  choose\n",
        false,
        "expected a choose name after `choose`",
    ),
    (
        "choose_name_declared_twice",
        "graph g\n  node a\n  choose c\n    when x -> a\n    else -> a\n  choose c\n    when x -> a\n    else -> a\n",
        false,
        "choose name is already declared in this graph",
    ),
    (
        "choose_block_missing",
        "graph g\n  node a\n  choose c\n",
        false,
        "expected an indented block",
    ),
    (
        "choose_line_with_a_trailing_colon_name",
        "graph g\n  node a\n  choose c : E\n",
        false,
        "expected the end of the line",
    ),
    (
        "annotation_without_a_key",
        "graph g\n  node a @\n",
        false,
        "expected an annotation key after `@`",
    ),
    (
        "annotation_without_a_value",
        "graph g\n  node a @k:\n",
        false,
        "expected a value after `:`",
    ),
    (
        "edge_annotation_without_a_key",
        "graph g\n  node a\n  edge a -> a @\n",
        false,
        "expected an annotation key after `@`",
    ),
    (
        "arm_without_else_or_when",
        "graph g\n  node a\n  choose c\n    else -> \n",
        false,
        "expected `when` or a final `else` arm",
    ),
    (
        "node_declared_twice",
        "graph g\n  node a end\n  node a end\n",
        false,
        "node name is already declared in this graph",
    ),
    (
        "node_with_two_unconditional_edges",
        "graph g\n  node a\n  node b end\n  node c end\n  edge a -> b\n  edge a -> c\n",
        false,
        "a node may have one unconditional edge",
    ),
    (
        "graph_without_a_name",
        "graph\n  node a end\n",
        false,
        "expected a graph name after `graph`",
    ),
    (
        "graph_name_used_twice",
        "graph g\n  node a end\ngraph g\n  node b end\n",
        false,
        "graph name is already used by an earlier graph in this source",
    ),
    (
        "source_without_a_graph",
        "\n",
        false,
        "expected a `graph` block",
    ),
    (
        "graph_body_outside_a_graph",
        "  node a end\n",
        false,
        "expected `graph` to start a graph block",
    ),
    (
        "enum_without_a_name",
        "graph g\n  enum\n",
        true,
        "expected an enum name after `enum`",
    ),
    (
        "enum_without_members",
        "graph g\n  enum E\n  input x : bool\n",
        true,
        "an enum needs at least one member",
    ),
    (
        "input_without_a_name",
        "graph g\n  input\n",
        true,
        "expected an input name after `input`",
    ),
    (
        "input_without_a_type",
        "graph g\n  input x :\n",
        true,
        "expected `:` and `bool` or an enum after the input name",
    ),
    (
        "input_with_an_unknown_type",
        "graph g\n  input x : Q\n",
        true,
        "unknown type; expected `bool` or an enum declared above this input",
    ),
    (
        "input_declared_twice",
        "graph g\n  input x : bool\n  input x : bool\n",
        true,
        "input name is already declared in this graph",
    ),
    (
        "choose_names_an_unknown_input",
        "graph g\n  enum E\n    a\n  choose c : E\n    when y -> a\n    else -> a\n",
        true,
        "unknown input; declare it with `input` above its `when`",
    ),
    (
        "choose_without_else_arm_in_pure_file",
        "graph g\n  enum E\n    a\n  input i : bool\n  choose c : E\n    when i -> a\n",
        true,
        "choose needs an `else` arm in a pure file",
    ),
    (
        "choose_names_an_unknown_enum",
        "graph g\n  choose c : E\n",
        true,
        "unknown enum; declare it with `enum` above this choose",
    ),
    (
        "enum_member_with_a_dangling_annotation",
        "graph g\n  enum E\n    a @\n",
        true,
        "expected an annotation key after `@`",
    ),
    (
        "enum_member_with_a_colon",
        "graph g\n  enum E\n    a :\n",
        true,
        "expected the end of the line",
    ),
    (
        "pure_choose_when_without_input_name",
        "graph g\n  enum E\n    a\n  choose c : E\n    when -> a\n    else -> a\n",
        true,
        "expected an input name after `when`",
    ),
    (
        "node_in_a_pure_file",
        "graph g\n  node a end\n",
        true,
        "`node` is not allowed in a pure file",
    ),
    (
        "pure_graph_without_a_body",
        "graph g\n",
        true,
        "expected an indented block",
    ),
];

#[test]
fn each_parser_error_names_its_problem() {
    let mut failures = Vec::new();
    for (index, (name, source, pure, message)) in CASES.iter().enumerate() {
        let file = dsl(&format!("parser_error_{index}_{name}.graphite"), source);
        let mut args = vec!["check".as_ref(), file.as_os_str(), "--json".as_ref()];
        if *pure {
            args.push("--pure".as_ref());
        }
        let output = graphite(args);
        let text = stdout(&output);
        if output.status.success() || !text.contains(message) {
            failures.push(format!("{name}: expected `{message}`, got {text}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
