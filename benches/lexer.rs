use std::hint::black_box;
use std::time::Duration;

use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use graphite::lexer::lex::GraphLexer;
use graphite::lexer::{Lexer, Token, TokenKind};

#[path = "support/dsl.rs"]
mod dsl;
#[path = "support/parallel.rs"]
mod parallel;
#[path = "support/workload.rs"]
mod workload;

fn lexer(criterion: &mut Criterion) {
    let throughput = std::env::var("GRAPHITE_BENCH_THROUGHPUT").unwrap_or_else(|_| "both".into());
    assert!(
        matches!(throughput.as_str(), "bytes" | "tokens" | "both"),
        "GRAPHITE_BENCH_THROUGHPUT must be bytes, tokens, or both"
    );
    let tier = std::env::var("GRAPHITE_BENCH_SIZE").unwrap_or_else(|_| "small".into());
    let stages = match tier.as_str() {
        "small" => 8,
        "medium" => 5_000,
        "large" => 100_000,
        "huge" => 1_000_000,
        _ => panic!("GRAPHITE_BENCH_SIZE must be small, medium, large, or huge"),
    };
    let seed = std::env::var("GRAPHITE_BENCH_SEED")
        .map(|value| {
            value
                .parse::<u64>()
                .expect("GRAPHITE_BENCH_SEED must be a u64")
        })
        .unwrap_or(42);
    let graphs = std::env::var("GRAPHITE_BENCH_GRAPHS")
        .map(|value| {
            value
                .parse::<usize>()
                .expect("GRAPHITE_BENCH_GRAPHS must be an integer")
        })
        .unwrap_or(2);
    assert!(
        graphs >= 2 && stages / graphs >= dsl::MIN_GRAPH_NODES,
        "graphs must be at least 2 (multiple graphs are a DSL feature) and each graph needs {} nodes",
        dsl::MIN_GRAPH_NODES
    );
    let workers = std::env::var("GRAPHITE_BENCH_WORKERS")
        .map(|value| {
            value
                .parse::<usize>()
                .expect("GRAPHITE_BENCH_WORKERS must be an integer")
        })
        .unwrap_or(0);
    let words = std::env::var("GRAPHITE_BENCH_WORDS").unwrap_or_else(|_| "generated".into());
    let names = match words.as_str() {
        "generated" => workload::names(stages, seed),
        "short" => vec!["closed".to_owned(); stages],
        "long" => workload::names(stages, seed)
            .into_iter()
            .map(|name| name.repeat(8))
            .collect(),
        _ => panic!("GRAPHITE_BENCH_WORDS must be generated, short, or long"),
    };
    let source: String = (0..graphs)
        .map(|index| {
            dsl::graph_source(&names[stages * index / graphs..stages * (index + 1) / graphs])
        })
        .collect();
    drop(names);

    // Setup checks, outside timing: the generated input must lex without errors and must
    // contain every DSL feature, so the measured lexing covers the whole language.
    assert!(
        Lexer::new(&source).all(|token| token.is_ok()),
        "generated DSL must lex"
    );
    let missing = dsl::missing_features(&source);
    assert!(
        missing.is_empty(),
        "generated DSL lacks features: {missing:?}"
    );
    // The graph lexer must agree with the iterator lexer before its speed is worth reporting.
    assert!(
        GraphLexer::new(&source).eq(Lexer::new(&source)),
        "graph lexer tokens must match the iterator lexer"
    );
    let tokens = Lexer::new(&source).count();
    if workers > 0 {
        assert_eq!(
            parallel::tokenize(&source, workers),
            Lexer::new(&source).collect::<Result<Vec<_>, _>>(),
            "parallel output must match sequential tokens and offsets"
        );
    }
    eprintln!(
        "Graphite lexer: tier={tier} seed={seed} stages={stages} graphs={graphs} words={words} workers={workers} simd={} bytes={} tokens={tokens}",
        cfg!(all(feature = "lexer-simd", target_arch = "x86_64")),
        source.len()
    );

    let mut name = format!("lexer/{tier}/seed_{seed}");
    if graphs > 1 {
        name.push_str(&format!("/graphs_{graphs}"));
    }
    if words != "generated" {
        name.push_str(&format!("/words_{words}"));
    }
    let mut group = criterion.benchmark_group(name);
    for (unit, count) in [
        ("bytes", Throughput::Bytes(source.len() as u64)),
        ("tokens", Throughput::Elements(tokens as u64)),
    ] {
        if throughput != "both" && throughput != unit {
            continue;
        }
        group.throughput(count);
        group.bench_function(format!("tokenize/{unit}"), |bencher| {
            bencher.iter(|| black_box(checksum(Lexer::new(black_box(&source)))));
        });
        group.bench_function(format!("tokenize_graph/{unit}"), |bencher| {
            bencher.iter(|| black_box(checksum(GraphLexer::new(black_box(&source)))));
        });
        if workers > 0 {
            group.bench_function(format!("collect/{unit}"), |bencher| {
                bencher.iter(|| {
                    black_box(
                        Lexer::new(black_box(&source))
                            .collect::<Result<Vec<_>, _>>()
                            .unwrap(),
                    );
                });
            });
            group.bench_function(format!("parallel_{workers}/{unit}"), |bencher| {
                bencher.iter(|| {
                    black_box(parallel::tokenize(black_box(&source), workers).unwrap());
                });
            });
        }
    }
    group.finish();
}

/// Folds each token into a checksum, so the lexer work is not dominated by per-token black_box
/// stores. The caller keeps the result live.
#[inline(always)]
fn checksum<'a, E: std::fmt::Debug>(tokens: impl Iterator<Item = Result<Token<'a>, E>>) -> usize {
    let mut total = 0usize;
    for token in tokens {
        let token = token.unwrap();
        let kind_len = match token.kind {
            TokenKind::Word(word) => word.len(),
            _ => 1,
        };
        total = total.wrapping_add(token.offset ^ kind_len);
    }
    total
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .sample_size(10)
        .warm_up_time(Duration::from_millis(500))
        .measurement_time(Duration::from_secs(1));
    targets = lexer
}
criterion_main!(benches);
