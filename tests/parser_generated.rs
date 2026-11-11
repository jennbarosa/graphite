use graphite::graph::index;
use graphite::parser::{Capacity, Schema, parse};

#[allow(dead_code)]
#[path = "../benches/support/dsl.rs"]
mod dsl;
#[allow(dead_code)]
#[path = "../benches/support/workload.rs"]
mod workload;

#[test]
fn generated_graphs_resolve_all_references_and_adjacency() {
    for seed in [0, 42, u64::MAX] {
        let names = workload::names(5_000, seed);
        let source = dsl::graph_source(&names);
        let graphs = parse(&source, &Schema::default(), &[dsl::graph_capacity(&names)]).unwrap();
        let graph = &graphs[0];
        assert_eq!(graph.nodes.len(), names.len());
        assert_eq!(graph.choices.len(), 2_000);
        assert_eq!(graph.outgoing.len(), graph.edges.len());
        for (id, (node, name)) in graph.nodes.iter().zip(&names).enumerate() {
            assert_eq!(graph.strings.resolve(node.name), Some(name.as_str()));
            for &edge_id in &graph.outgoing[index(&node.outgoing)] {
                let edge = &graph.edges[edge_id as usize];
                assert_eq!(edge.source as usize, id);
                assert!((edge.target as usize) < names.len());
                if let Some(condition) = edge.condition {
                    assert!((condition as usize) < graph.conditions.len());
                }
            }
        }
    }
}

#[test]
fn truncated_source_never_panics() {
    let names = workload::names(8, 42);
    let capacity = [dsl::graph_capacity(&names)];
    let source = dsl::graph_source(&names);
    for (offset, _) in source.char_indices() {
        let _ = parse(&source[..offset], &Schema::default(), &capacity);
    }
}

#[test]
fn seeded_undersized_budgets_never_panic() {
    let names = workload::names(8, 42);
    let source = dsl::graph_source(&names);
    let full = dsl::graph_capacity(&names);
    let mut seed = 42u64;
    for _ in 0..256 {
        let mut bound = |limit: usize| {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            (seed >> 32) as usize % (limit + 1)
        };
        let hint = Capacity {
            nodes: bound(full.nodes),
            edges: bound(full.edges),
            conditions: bound(full.conditions),
            choices: bound(full.choices),
            arms: bound(full.arms),
            annotations: bound(full.annotations),
            flags: 0,
            strings: bound(full.strings),
            string_bytes: bound(full.string_bytes),
        };
        if let Ok(graphs) = parse(&source, &Schema::default(), &[hint]) {
            assert_eq!(graphs[0].nodes.len(), names.len());
            assert_eq!(graphs[0].outgoing.len(), graphs[0].edges.len());
        }
    }
}

#[test]
fn capacity_bounds_cover_distinct_names_in_every_interned_role() {
    let count = 40;
    let names = workload::names(count * 8, 42);
    let flags: Vec<_> = names[count * 6..count * 7]
        .iter()
        .map(String::as_str)
        .collect();
    let mut source = String::from("graph door\n");
    for i in 0..count {
        source.push_str(&format!(
            "  node {} {} @{}:{}\n",
            names[i],
            names[count * 6 + i],
            names[count * 4 + i],
            names[count * 5 + i]
        ));
    }
    for i in 0..count {
        source.push_str(&format!(
            "  edge {} -> {} when {} @time.delay\n",
            names[i],
            names[(i + 1) % count],
            names[count + i]
        ));
        source.push_str(&format!(
            "  choose {}\n    when {} -> {}\n",
            names[count * 2 + i],
            names[count * 7 + i],
            names[count * 3 + i]
        ));
    }
    let capacity = Capacity {
        nodes: count,
        edges: count,
        conditions: count * 2,
        choices: count,
        arms: count,
        annotations: count * 2,
        flags: count,
        strings: names.len() + 2,
        string_bytes: names.iter().map(String::len).sum::<usize>() + "doortime.delay".len(),
    };
    let graphs = parse(&source, &Schema { node_flags: &flags }, &[capacity]).unwrap();
    let graph = &graphs[0];
    assert_eq!(graph.nodes.len(), count);
    assert_eq!(graph.conditions.len(), count * 2);
    assert_eq!(graph.flags.len(), count);
    for i in 0..count {
        let resolve = |id| graph.strings.resolve(id).unwrap();
        assert_eq!(resolve(graph.flags[i]), names[count * 6 + i]);
        let annotation = &graph.annotations[graph.nodes[i].annotations.start as usize];
        assert_eq!(resolve(annotation.key), names[count * 4 + i]);
        assert_eq!(resolve(annotation.value.unwrap()), names[count * 5 + i]);
        assert_eq!(resolve(graph.choices[i].name), names[count * 2 + i]);
        assert_eq!(resolve(graph.arms[i].tag), names[count * 3 + i]);
        let edge_condition = graph.edges[i].condition.unwrap() as usize;
        let arm_condition = graph.arms[i].condition.unwrap() as usize;
        assert_eq!(resolve(graph.conditions[edge_condition]), names[count + i]);
        assert_eq!(
            resolve(graph.conditions[arm_condition]),
            names[count * 7 + i]
        );
    }
}
