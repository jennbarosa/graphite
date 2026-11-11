use std::hint::black_box;
use std::time::Duration;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use graphite::parser::{Schema, parse};

#[allow(dead_code)]
#[path = "support/dsl.rs"]
mod dsl;
#[path = "support/profile.rs"]
mod profile;
#[path = "support/workload.rs"]
mod workload;

fn parser(criterion: &mut Criterion) {
    let tier = std::env::var("GRAPHITE_BENCH_SIZE").unwrap_or_else(|_| "small".into());
    let stages = match tier.as_str() {
        "small" => 8,
        "medium" => 5_000,
        "huge" => 1_000_000,
        _ => panic!("GRAPHITE_BENCH_SIZE must be small, medium, or huge"),
    };
    let seed = std::env::var("GRAPHITE_BENCH_SEED")
        .map(|value| {
            value
                .parse::<u64>()
                .expect("GRAPHITE_BENCH_SEED must be a u64")
        })
        .unwrap_or(42);
    let names = workload::names(stages, seed);
    let capacity = [dsl::graph_capacity(&names)];
    let source = dsl::graph_source(&names);
    drop(names);
    let schema = Schema::default();

    // Verify real compiled output outside timing, and release it before measurement.
    let graphs = parse(&source, &schema, &capacity).expect("generated DSL must compile");
    assert_eq!(graphs.len(), 1);
    let graph = &graphs[0];
    assert_eq!(graph.nodes.len(), stages);
    assert!(graph.nodes.last().unwrap().end);
    assert_eq!(graph.outgoing.len(), graph.edges.len());
    eprintln!(
        "Graphite parser: tier={tier} seed={seed} stages={stages} bytes={} edges={} choices={} arms={} conditions={}",
        source.len(),
        graph.edges.len(),
        graph.choices.len(),
        graph.arms.len(),
        graph.conditions.len(),
    );
    drop(graphs);

    if profile::from_environment(&source, &schema, &capacity).expect("perf profiling failed") {
        return;
    }

    let mut group = criterion.benchmark_group(format!("parser/{tier}/seed_{seed}"));
    group.throughput(Throughput::Elements(stages as u64));
    group.bench_function("parse_build_drop", |bencher| {
        bencher.iter(|| {
            black_box(parse(black_box(&source), black_box(&schema), black_box(&capacity)).unwrap())
        });
    });
    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .sample_size(10)
        .warm_up_time(Duration::from_millis(500))
        .measurement_time(Duration::from_secs(1));
    targets = parser
}
criterion_main!(benches);
