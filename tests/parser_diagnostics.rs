//! Merged test file. Each section below was `tests/<name>.rs` before; see git history.

#[allow(dead_code)]
#[path = "support/cli.rs"]
mod cli;

// Merged from `tests/parser_diagnostics.rs`.
mod parser_diagnostics {

    use graphite::parser::{ParseError, Schema};
    use graphite::pipeline::compile;

    fn rendered(source: &str) -> String {
        let error: ParseError = compile(source, &Schema::default()).err().unwrap();
        error.diagnostic(source).to_string()
    }

    #[test]
    fn parse_errors_underline_the_offending_text() {
        let cases = [
            (
                "graph door\n  node open\n  node open\n",
                "error: node name is already declared in this graph\n --> line 3, column 8\n  |\n3 |   node open\n  |        ^^^^ node name is already declared in this graph",
            ),
            (
                "graph door\n  node a\n  wrong thing here\n",
                "error: expected `default`, `node`, `edge`, or `choose` line\n --> line 3, column 3\n  |\n3 |   wrong thing here\n  |   ^^^^^ expected `default`, `node`, `edge`, or `choose` line",
            ),
            (
                "graph door\n  node a\n  edge b -> c\n",
                "error: unknown node; declare it with `node` before linking to it\n --> line 3, column 8\n  |\n3 |   edge b -> c\n  |        ^ unknown node; declare it with `node` before linking to it",
            ),
            (
                // Columns count characters, not bytes: the caret stays under `->`.
                "graph door\n  node a\n  node é_bad -> a\n",
                "error: expected the end of the line\n --> line 3, column 14\n  |\n3 |   node é_bad -> a\n  |              ^^ expected the end of the line",
            ),
            (
                "graph door\n  node a\n  choose c\n    when x\n    -> t\n",
                "error: expected `->`\n --> line 4, column 11\n  |\n4 |     when x\n  |           ^ expected `->`",
            ),
            (
                "graph door\n\tnode a\n",
                "error: use spaces instead of tabs\n --> line 2, column 1\n  |\n2 |  node a\n  | ^ use spaces instead of tabs",
            ),
        ];
        for (source, expected) in cases {
            assert_eq!(rendered(source), expected, "source: {source:?}");
        }
    }

    #[test]
    fn diagnostics_never_panic_at_end_of_input() {
        // The offset may sit at the end of the source with no trailing newline.
        let source = "graph door\n  node a -> ";
        let text = rendered(source);
        assert!(
            text.starts_with("error: expected the end of the line\n --> line 2, column 10"),
            "{text}"
        );
        let past_end = ParseError {
            message: "end",
            offset: source.len() + 10,
        };
        assert!(past_end.diagnostic(source).to_string().contains("line 2"));
    }

    #[test]
    fn a_node_the_entry_cannot_reach_is_a_build_error_at_its_declaration() {
        // `dead` has an outgoing edge, so only reachability catches it.
        let source = "graph door\n  node a\n  node dead\n  edge a -> a when go\n  edge dead -> dead when go\n";
        let message = rendered(source);
        assert!(
            message.contains("node is unreachable from the entry node"),
            "{message}"
        );
        assert!(message.contains("line 3, column 8"), "{message}");
    }
}

// Merged from `tests/parser_capacity.rs`.
mod parser_capacity {

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
            "walk", "windup", "looping", "anim", "stopped", "keyup", "sfx", "latch", "facing",
            "left", "side", "flip", "true", "down",
        ];
        Capacity {
            nodes: 2,
            edges: 1,
            conditions: 2,
            choices: 1,
            arms: 2,
            enums: 0,
            members: 0,
            inputs: 0,
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
            ..Default::default()
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
            ..Default::default()
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
                    ..Default::default()
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
}

// Merged from `tests/parser_capacity_limits.rs`.
mod parser_capacity_limits {
    //! The caller's capacity hints bound what the parser stores. A hint that cannot be honored is
    //! refused with a message that names the hint, before anything is parsed into it.

    use graphite::parser::{Capacity, Schema, parse};

    const TWO_NODES: &str = "graph g\n  node a\n  node b end\n  edge a -> b\n";

    /// Generous hints for `TWO_NODES`, so one field can be changed at a time.
    fn roomy() -> Capacity {
        Capacity {
            nodes: 8,
            edges: 8,
            conditions: 8,
            choices: 4,
            arms: 8,
            enums: 0,
            members: 0,
            inputs: 0,
            annotations: 8,
            flags: 0,
            strings: 32,
            string_bytes: 256,
        }
    }

    fn refused(capacity: Capacity, source: &str) -> String {
        let error = parse(source, &Schema::default(), &[capacity])
            .err()
            .expect("a refused capacity");
        error.to_string()
    }

    #[test]
    fn roomy_hints_parse_the_graph() {
        assert!(parse(TWO_NODES, &Schema::default(), &[roomy()]).is_ok());
    }

    #[test]
    fn a_count_that_does_not_fit_in_32_bits_is_refused() {
        let capacity = Capacity {
            nodes: u32::MAX as usize + 1,
            ..roomy()
        };
        assert!(refused(capacity, TWO_NODES).contains("capacity exceeds u32 storage"));
    }

    #[test]
    fn a_buffer_too_large_to_reserve_is_refused() {
        let capacity = Capacity {
            nodes: u32::MAX as usize,
            ..roomy()
        };
        assert!(refused(capacity, TWO_NODES).contains("could not reserve memory for the graph"));
    }

    #[test]
    fn a_string_table_too_large_to_reserve_is_refused() {
        let capacity = Capacity {
            strings: usize::MAX,
            ..roomy()
        };
        assert!(
            refused(capacity, TWO_NODES).contains("could not reserve memory for graph strings")
        );
    }

    #[test]
    fn a_graph_name_that_does_not_fit_the_string_hint_is_refused() {
        let capacity = Capacity {
            strings: 0,
            ..roomy()
        };
        assert!(refused(capacity, TWO_NODES).contains("string capacity exhausted"));
    }

    #[test]
    fn a_node_count_above_the_hint_is_refused_at_the_node() {
        let capacity = Capacity {
            nodes: 1,
            ..roomy()
        };
        let message = refused(capacity, TWO_NODES);
        assert!(
            message.contains("graph capacity exhausted; raise its count hints"),
            "{message}"
        );
    }
}

// Merged from `tests/parser_error_messages.rs`.
mod parser_error_messages {
    //! Each parser error the author can cause, checked through `graphite check --json`: the message
    //! must name the problem and the fix. Every case is a small graph with one mistake.
    #[allow(unused_imports)]
    use super::cli;

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
            "choose needs at least one `when` arm",
        ),
        (
            "choose_opens_with_else",
            "graph g\n  node a end\n  choose c\n    else -> a\n",
            false,
            "choose needs at least one `when` arm",
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
            let source = if *pure {
                format!("@!frontend:purescript\n{source}")
            } else {
                source.to_string()
            };
            let file = dsl(&format!("parser_error_{index}_{name}.graphite"), &source);
            let output = graphite(["check".as_ref(), file.as_os_str(), "--json".as_ref()]);
            let text = stdout(&output);
            if output.status.success() || !text.contains(message) {
                failures.push(format!("{name}: expected `{message}`, got {text}"));
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }
}
