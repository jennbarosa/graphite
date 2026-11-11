use std::hint::black_box;
use std::time::Duration;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use graphite::frontend::Frontend;
use graphite::frontend::typescript::{RUNTIME_FILE, TYPESCRIPT, TypeScript};
use graphite::graph::CompiledGraph;
use graphite::parser::{Schema, parse};

#[allow(dead_code)]
#[path = "support/dsl.rs"]
mod dsl;
#[path = "support/workload.rs"]
mod workload;

const FRONTEND: TypeScript = TYPESCRIPT;

/// Emits every module of every graph, back to back, into one caller-owned buffer.
fn emit_all(graphs: &[CompiledGraph], output: &mut Vec<u8>) {
    for graph in graphs {
        for module in FRONTEND.modules(graph) {
            FRONTEND
                .emit_module(graph, module, output)
                .expect("emission must succeed");
        }
    }
}

fn frontend_emit(criterion: &mut Criterion) {
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

    // Parse and size the output outside timing; emission reads the checked graphs.
    let graphs = parse(&source, &schema, &capacity).expect("generated DSL must compile");
    drop(source);
    let mut probe = Vec::new();
    emit_all(&graphs, &mut probe);
    let bytes = probe.len();
    drop(probe);
    eprintln!("Graphite frontend: tier={tier} seed={seed} stages={stages} output_bytes={bytes}");
    if let Some(directory) = std::env::var_os("GRAPHITE_FRONTEND_EXPORT_DIR") {
        let directory = std::path::Path::new(&directory);
        std::fs::create_dir_all(directory).unwrap();
        let mut runtime =
            std::io::BufWriter::new(std::fs::File::create(directory.join(RUNTIME_FILE)).unwrap());
        FRONTEND.emit_runtime(&mut runtime).unwrap();
        std::io::Write::flush(&mut runtime).unwrap();
        for graph in &graphs {
            for module in FRONTEND.modules(graph) {
                let name = FRONTEND.file_name(graph, module);
                let mut file =
                    std::io::BufWriter::new(std::fs::File::create(directory.join(name)).unwrap());
                FRONTEND.emit_module(graph, module, &mut file).unwrap();
                std::io::Write::flush(&mut file).unwrap();
            }
        }
    }

    let mut group = criterion.benchmark_group(format!("frontend/{tier}/seed_{seed}"));
    group.throughput(Throughput::Bytes(bytes as u64));
    group.bench_function("typescript_emit", |bencher| {
        bencher.iter(|| {
            let mut output = Vec::with_capacity(bytes);
            emit_all(black_box(&graphs), &mut output);
            black_box(output);
        });
    });
    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .sample_size(10)
        .warm_up_time(Duration::from_millis(500))
        .measurement_time(Duration::from_secs(2));
    targets = frontend_emit
}
criterion_main!(benches);
