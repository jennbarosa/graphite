use graphite::graph::index;
use graphite::parser::Schema;

#[path = "support/parse.rs"]
mod fixture;
use fixture::parse;

#[test]
fn opaque_values_do_not_certify_annotation_keys() {
    let source = "graph door\n  node closed end @anim:time..delay @time..delay\n";
    let error = parse(source, &Schema::default()).err().unwrap();
    assert_eq!(
        error.message,
        "invalid annotation key: use letters, digits, `_`, or `-`, separated by `.`"
    );
    assert_eq!(error.offset, source.rfind("time..delay").unwrap());
    let graphs = parse(
        "graph door\n  node closed end @time.delay:250 @time.delay @anim:time.delay\n",
        &Schema::default(),
    )
    .unwrap();
    assert_eq!(graphs[0].annotations[0].key, graphs[0].annotations[1].key);
    assert_eq!(
        graphs[0].annotations[2].value,
        Some(graphs[0].annotations[0].key)
    );
}

const DOOR: &str = "graph door
  node closed  @anim:idle
  node opening @anim:open
  node open    @anim:idle_open
  node closing @anim:close

  edge closed  -> opening when request
  edge opening -> open
  edge open    -> closing when timeout
  edge closing -> closed  @sfx:latch
";

const WALK: &str = "graph walk
  node windup   @anim:windup
  node going    loop @anim:walk @group:locomotion
  node winddown end @anim:winddown
  node stopped  end @anim:idle

  edge windup    -> stopped   when keyup
  edge windup    -> going
  edge going     -> winddown  when keyup

  choose facing
    when left  -> side @flip:true
    when right -> side
    when up    -> up
    else       -> down
";

#[test]
fn door_emits_resolved_edges_and_opaque_annotations() {
    let graphs = parse(DOOR, &Schema::default()).unwrap();
    let graph = &graphs[0];
    assert_eq!(graph.strings.resolve(graph.name), Some("door"));
    assert_eq!(graph.nodes.len(), 4);
    assert_eq!(graph.edges.len(), 4);
    assert_eq!(graph.conditions.len(), 2);
    let gate = &graph.edges[graph.outgoing[graph.nodes[0].outgoing.start as usize] as usize];
    assert_eq!((gate.source, gate.target, gate.condition), (0, 1, Some(0)));
    let latch = &graph.annotations[graph.edges[3].annotations.start as usize];
    assert_eq!(graph.strings.resolve(latch.key), Some("sfx"));
    assert_eq!(graph.strings.resolve(latch.value.unwrap()), Some("latch"));
}

#[test]
fn walk_preserves_flags_conditions_and_arm_annotations() {
    let graphs = parse(WALK, &Schema::default()).unwrap();
    let g = &graphs[0];
    assert!(g.nodes[1].looping);
    assert!(g.nodes[2].end && g.nodes[3].end);
    assert_eq!(g.conditions.len(), 4);
    assert_eq!(g.edges[0].condition, g.edges[2].condition);
    assert_eq!(g.choices.len(), 1);
    let arms = &g.arms[index(&g.choices[0].arms)];
    assert_eq!(arms.len(), 4);
    assert_eq!(arms[0].tag, arms[1].tag);
    assert_eq!(arms[0].annotations.len(), 1);
    assert!(arms[1].annotations.is_empty());
    assert_eq!(arms[3].condition, None);
    assert_eq!(g.strings.resolve(arms[3].tag), Some("down"));
}

#[test]
fn graph_scopes_are_independent_and_output_owns_its_strings() {
    let source = format!("{DOOR}\ngraph walk\n  node closed end");
    let graphs = parse(&source, &Schema::default()).unwrap();
    drop(source);
    assert_eq!(graphs.len(), 2);
    assert_eq!(graphs[1].nodes.len(), 1);
    assert!(graphs[1].conditions.is_empty());
    assert_eq!(
        graphs[1].strings.resolve(graphs[1].nodes[0].name),
        Some("closed")
    );
}

#[test]
fn adjacency_groups_interleaved_sources_without_reordering_gates() {
    let source = "graph door\n  node closed\n  node opening\n  node open end\n  edge closed -> open\n  edge opening -> open\n  edge closed -> opening when request\n  edge closed -> open when timeout\n";
    let graphs = parse(source, &Schema::default()).unwrap();
    let g = &graphs[0];
    assert_eq!(&g.outgoing[index(&g.nodes[0].outgoing)], &[0, 2, 3]);
    assert_eq!(&g.outgoing[index(&g.nodes[1].outgoing)], &[1]);
    assert!(g.nodes[2].outgoing.is_empty());
}

#[test]
fn schema_flags_and_namespaced_valueless_annotations_are_data() {
    let schema = Schema {
        node_flags: &["looping"],
    };
    let graphs = parse(
        "graph walk\n  node stopped end looping @time.delay @anim:idle",
        &schema,
    )
    .unwrap();
    let g = &graphs[0];
    assert_eq!(g.strings.resolve(g.flags[0]), Some("looping"));
    assert_eq!(g.strings.resolve(g.annotations[0].key), Some("time.delay"));
    assert_eq!(g.annotations[0].value, None);
    assert_eq!(
        g.strings.resolve(g.annotations[1].value.unwrap()),
        Some("idle")
    );
}

#[test]
fn core_only_graphs_reserve_no_domain_flag_storage() {
    let graphs = parse(WALK, &Schema::default()).unwrap();
    assert_eq!(graphs[0].flags.capacity(), 0);
}

#[test]
fn edges_and_choices_can_mix_after_all_nodes_are_declared() {
    let source = "graph door\n  node closed\n  choose facing\n    when left -> side\n  edge closed -> closed when request\n";
    let graphs = parse(source, &Schema::default()).unwrap();
    assert_eq!(graphs[0].choices.len(), 1);
    assert_eq!(graphs[0].edges.len(), 1);
}

#[test]
fn choose_without_else_and_cycles_without_end_are_valid() {
    let source = "graph door\n  node closed\n  edge closed -> closed when request\n  choose facing\n    when left -> side\n";
    let graphs = parse(source, &Schema::default()).unwrap();
    assert_eq!(graphs[0].arms.len(), 1);
    assert_eq!(graphs[0].edges[0].target, 0);
}

#[test]
fn rejects_invalid_structure_and_grammar() {
    let invalid = [
        "",
        "  ",
        "graph door\n",
        "graph door\n  choose facing\n    when left -> side\n",
        "graph door\n  node closed\n",
        "graph door\n  node closed end\n  edge closed -> closed\n",
        "graph door\n  node closed\n  edge closed -> closed\n  edge closed -> closed\n",
        "graph door\n  node closed end\n  node closed end\n",
        "graph door\n  edge closed -> closed\n  node closed end\n",
        "graph door\n  node closed\n  edge closed -> opening\n  node opening end\n",
        "graph door\n  node closed end unknown\n",
        "graph door\n  node closed end @anim:\n",
        "graph door\n  node closed end @time..delay\n",
        "graph door\n  node closed end @anim:idle end\n",
        "graph door\n  node closed end\n  else -> closed\n",
        "graph door\n  node closed end\n  choose facing\n    else -> down\n",
        "graph door\n  node closed end\n  choose facing\n    when left -> side\n    else -> down\n    when up -> up\n",
        "graph door\n  node closed end\n  choose facing\n    when left -> side\n    else -> down\n    else -> down\n",
        "graph door\n  node closed end\n  choose facing\n    when left -> side\n  choose facing\n    when up -> up\n",
        "graph door\n  node closed end\n    node opening end\n",
        "graph door\n\tnode closed end\n",
        "graph door\n   node closed end\n  node opening end\n",
        "graph door\n  node closed end\ngraph door\n  node open end\n",
        "graph door\n  node closed end\ngraph walk\n  node open\n  edge open -> closed\n",
        "graph door extra\n  node closed end\n",
        "graph door\n  node closed end\n  choose facing\n    when left side\n",
        "graph door\n  node closed end\n  choose facing\n    when left -> side extra\n",
    ];
    for source in invalid {
        assert!(
            parse(source, &Schema::default()).is_err(),
            "accepted {source:?}"
        );
    }
}

#[test]
fn reports_nodes_after_links_invalid_names_and_lexer_errors_at_their_byte() {
    let cases = [
        (
            "graph door\n  node closed\n  edge closed -> closed\n  node opening end\n",
            "nodes must be declared before edges and choices",
            "node opening".to_string(),
        ),
        (
            "graph door\n  node closed! end\n",
            "invalid name: use letters, digits, `_`, or `-`",
            "closed!".to_string(),
        ),
        (
            "graph door\n  node closed\n  edge\tclosed -> closed\n",
            "use spaces instead of tabs",
            "\t".to_string(),
        ),
    ];
    for (source, message, anchor) in cases {
        let error = parse(source, &Schema::default()).err().unwrap();
        assert_eq!(error.message, message, "{source:?}");
        assert_eq!(error.offset, source.find(&anchor).unwrap(), "{source:?}");
    }
}

#[test]
fn errors_point_to_the_offending_source_byte() {
    let source = "graph door\n  node closed end unknown\n";
    let error = parse(source, &Schema::default()).err().unwrap();
    assert_eq!(error.offset, source.find("unknown").unwrap());
    assert!(error.to_string().contains("unknown node flag"));
    let source = "graph door\n  node closed\n";
    let error = parse(source, &Schema::default()).err().unwrap();
    assert_eq!(error.offset, source.find("closed").unwrap());
    assert!(error.to_string().contains("outgoing"));
}

#[test]
fn one_string_can_name_a_node_condition_choice_tag_and_annotation() {
    let source = "graph door\n  node request\n  node timeout end\n  edge request -> timeout when request\n  choose request\n    when request -> request @request:request\n";
    let graphs = parse(source, &Schema::default()).unwrap();
    let graph = &graphs[0];
    let request = graph.nodes[0].name;
    assert_eq!(graph.conditions, [request]);
    assert_eq!(graph.choices[0].name, request);
    assert_eq!(graph.arms[0].tag, request);
    assert_eq!(graph.annotations[0].key, request);
    assert_eq!(graph.annotations[0].value, Some(request));
    assert_eq!(graph.edges[0].source, 0);
    assert_eq!(graph.edges[0].target, 1);
    assert_eq!(graph.edges[0].condition, Some(0));
    assert_eq!(graph.arms[0].condition, Some(0));
}

#[test]
fn undeclared_references_keep_their_diagnostic_when_storage_is_tightly_sized() {
    let source = "graph door\n  node closed\n  edge closed -> opening\n";
    let error = parse(source, &Schema::default()).err().unwrap();
    assert_eq!(
        error.message,
        "unknown node; declare it with `node` before linking to it"
    );
    assert_eq!(error.offset, source.find("opening").unwrap());
}

#[test]
fn opaque_interned_values_do_not_bypass_identifier_validation() {
    for source in [
        "graph door\n  node closed @anim:request!\n  edge closed -> closed when request!\n",
        "graph door\n  node closed @anim:closed!\n  edge closed -> closed!\n",
        "graph door\n  node closed\n  edge closed! -> closed\n",
    ] {
        let error = parse(source, &Schema::default()).err().unwrap();
        assert_eq!(
            error.message,
            "invalid name: use letters, digits, `_`, or `-`"
        );
        let invalid = source[error.offset..].split_whitespace().next().unwrap();
        assert!(invalid.ends_with('!'));
        assert_eq!(error.offset, source.rfind(invalid).unwrap());
    }
}

#[test]
fn cached_reference_lookup_preserves_utf8_names_and_opaque_values() {
    let source = "graph door\n  node é @anim:🦀\n  node open end\n  edge é -> open when é\n  edge é -> é when é\n";
    let graphs = parse(source, &Schema::default()).unwrap();
    let graph = &graphs[0];
    assert_eq!(graph.strings.resolve(graph.nodes[0].name), Some("é"));
    assert_eq!(graph.conditions, [graph.nodes[0].name]);
    assert_eq!(graph.edges[0].target, 1);
    assert_eq!(graph.edges[1].target, 0);
    assert_eq!(graph.edges[0].condition, graph.edges[1].condition);
    assert_eq!(
        graph.strings.resolve(graph.annotations[0].value.unwrap()),
        Some("🦀")
    );
}

const LINE: &str = "graph line
  node a
  node b
  node c end

  edge a -> b -> c when go else c @sfx:step
";

#[test]
fn a_chain_expands_to_hops_then_fallbacks_with_shared_condition_and_annotations() {
    let graphs = parse(LINE, &Schema::default()).unwrap();
    let graph = &graphs[0];
    let edges: Vec<(u32, u32, Option<u32>)> = graph
        .edges
        .iter()
        .map(|edge| (edge.source, edge.target, edge.condition))
        .collect();
    let go = Some(0);
    // Hops first, in chain order; then one fallback per hop source, to `c`.
    assert_eq!(edges, [(0, 1, go), (1, 2, go), (0, 2, None), (1, 2, None)]);
    for edge in &graph.edges {
        assert_eq!(
            graph
                .strings
                .resolve(graph.annotations[edge.annotations.start as usize].key),
            Some("sfx")
        );
    }
    assert_eq!(graph.nodes[0].sequence, Some(2));
    assert_eq!(graph.nodes[1].sequence, Some(3));
    assert_eq!(graph.nodes[0].outgoing, 0..2);
    assert_eq!(graph.nodes[1].outgoing, 2..4);
}

#[test]
fn a_chain_without_else_is_a_sequence_per_hop() {
    let graphs = parse(
        "graph line\n  node a\n  node b\n  node c end\n  edge a -> b -> c\n",
        &Schema::default(),
    )
    .unwrap();
    let graph = &graphs[0];
    assert_eq!(graph.nodes[0].sequence, Some(0));
    assert_eq!(graph.nodes[1].sequence, Some(1));
    assert!(graph.edges.iter().all(|edge| edge.condition.is_none()));
}

#[test]
fn a_chain_with_else_conflicts_with_a_sequence_hop_at_the_line() {
    let source = "graph line\n  node a\n  node b\n  node c end\n  edge a -> b -> c else c\n";
    let error = parse(source, &Schema::default()).err().unwrap();
    assert_eq!(
        error.message,
        "a node may have one unconditional edge; give the others `when` conditions"
    );
    assert_eq!(error.offset, source.find("a -> ").unwrap());
}

#[test]
fn a_chain_cannot_leave_an_end_node_even_through_else() {
    let source =
        "graph line\n  node a\n  node c end\n  node b end\n  edge a -> c -> b when go else b\n";
    let error = parse(source, &Schema::default()).err().unwrap();
    assert_eq!(error.message, "an `end` node cannot have outgoing edges");
    assert_eq!(error.offset, source.find("a -> ").unwrap());
}

#[test]
fn a_chain_must_name_declared_nodes_at_every_hop() {
    let source = "graph line\n  node a\n  node b end\n  edge a -> b -> nowhere when go\n";
    let error = parse(source, &Schema::default()).err().unwrap();
    assert_eq!(
        error.message,
        "unknown node; declare it with `node` before linking to it"
    );
    assert_eq!(error.offset, source.find("nowhere").unwrap());
}
