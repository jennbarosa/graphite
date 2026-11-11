use std::hint::black_box;
use std::time::Duration;

use criterion::{BatchSize, Criterion, Throughput, criterion_group, criterion_main};
use graphite::arena::Arena;
use graphite::interner::Interner;

#[path = "support/workload.rs"]
mod workload;

fn compiler_storage(criterion: &mut Criterion) {
    let tier = std::env::var("GRAPHITE_BENCH_SIZE").unwrap_or_else(|_| "small".into());
    let count = match tier.as_str() {
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
    let names = workload::names(count, seed);
    let bytes = names.iter().map(String::len).sum();
    eprintln!("Graphite storage: tier={tier} seed={seed} names={count} bytes={bytes}");
    let mut group = criterion.benchmark_group(format!("storage/{tier}/seed_{seed}"));
    group.throughput(Throughput::Elements(count as u64));

    group.bench_function("interner_insert", |bencher| {
        bencher.iter_batched(
            || Interner::with_capacity(count, bytes).unwrap(),
            |mut pool| {
                for name in &names {
                    black_box(pool.intern(black_box(name)).unwrap());
                }
                pool
            },
            BatchSize::PerIteration,
        );
    });

    group.bench_function("interner_build", |bencher| {
        bencher.iter(|| {
            let mut pool = Interner::with_capacity(count, bytes).unwrap();
            for name in &names {
                black_box(pool.intern(black_box(name)).unwrap());
            }
            black_box(pool);
        });
    });

    let mut pool = Interner::with_capacity(count, bytes).unwrap();
    let ids: Vec<_> = names
        .iter()
        .map(|name| pool.intern(name).unwrap())
        .collect();
    group.bench_function("interner_duplicate", |bencher| {
        bencher.iter(|| {
            for name in &names {
                black_box(pool.intern(black_box(name)).unwrap());
            }
        });
    });
    group.bench_function("interner_resolve", |bencher| {
        bencher.iter(|| {
            for id in &ids {
                black_box(pool.resolve(black_box(*id)).unwrap());
            }
        });
    });

    // Quadratic baseline is intentionally excluded from the million-name workload.
    if tier != "huge" {
        group.bench_function("linear_interner_build", |bencher| {
            bencher.iter(|| {
                let mut stored: Vec<String> = Vec::with_capacity(count);
                for name in &names {
                    if !stored.iter().any(|value| value == black_box(name)) {
                        stored.push(name.clone());
                    }
                }
                black_box(stored);
            });
        });
    }

    let mut arena = Arena::with_capacity(count).unwrap();
    group.bench_function("arena_refill", |bencher| {
        bencher.iter(|| {
            arena.clear();
            for index in 0..count as u32 {
                black_box(arena.push(black_box(index)).unwrap());
            }
            black_box(&arena);
        });
    });
    group.bench_function("vec_build", |bencher| {
        bencher.iter(|| {
            let mut items = Vec::new();
            for index in 0..count as u32 {
                items.push(black_box(index));
            }
            black_box(items);
        });
    });
    group.bench_function("arena_build", |bencher| {
        bencher.iter(|| {
            let mut arena = Arena::with_capacity(count).unwrap();
            for index in 0..count as u32 {
                black_box(arena.push(black_box(index)).unwrap());
            }
            black_box(arena);
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
    targets = compiler_storage
}
criterion_main!(benches);
