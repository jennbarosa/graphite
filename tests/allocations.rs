use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use graphite::arena::Arena;
use graphite::interner::{Interner, InternerFull};
use graphite::lexer::{Lexer, TokenKind};
use graphite::parser::{Capacity, Schema, parse};

/// Counts heap allocations made by the test that asked for it.
struct CountingAllocator;

thread_local! {
    // const init and no destructor: safe to touch from inside the allocator.
    static COUNT_THIS_THREAD: Cell<bool> = const { Cell::new(false) };
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
    static REQUESTED_BYTES: Cell<usize> = const { Cell::new(0) };
}

fn count_if_watched(size: usize) {
    if COUNT_THIS_THREAD.with(Cell::get) {
        ALLOCATIONS.with(|count| count.set(count.get() + 1));
        REQUESTED_BYTES.with(|bytes| bytes.set(bytes.get() + size));
    }
}

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count_if_watched(layout.size());
        unsafe { System.alloc(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        count_if_watched(size);
        unsafe { System.realloc(ptr, layout, size) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static GLOBAL: CountingAllocator = CountingAllocator;

/// Runs `work` and returns its result plus the allocations made by this test's thread only,
/// so other test threads cannot skew the count.
fn allocations_during<T>(work: impl FnOnce() -> T) -> (T, usize) {
    COUNT_THIS_THREAD.with(|c| c.set(true));
    let before = ALLOCATIONS.with(Cell::get);
    let result = work();
    let after = ALLOCATIONS.with(Cell::get);
    COUNT_THIS_THREAD.with(|c| c.set(false));
    (result, after - before)
}

#[test]
fn counter_detects_a_real_allocation() {
    let barrier = std::sync::Barrier::new(2);
    std::thread::scope(|scope| {
        let measure = || {
            let ((), allocated) = allocations_during(|| {
                barrier.wait();
                std::hint::black_box(Box::new(7u64));
                barrier.wait();
            });
            assert_eq!(allocated, 1);
        };
        let other = scope.spawn(measure);
        measure();
        other.join().unwrap();
    });
}

#[test]
fn refilling_after_clear_does_not_allocate() {
    let mut arena = Arena::with_capacity(100).unwrap();
    for value in 0..100u32 {
        arena.push(value).unwrap();
    }
    arena.clear();

    let ((), allocated) = allocations_during(|| {
        for value in 0..100u32 {
            arena.push(value).unwrap();
        }
    });

    assert_eq!(allocated, 0);
}

#[test]
fn interner_setup_fits_compact_storage_budget() {
    let before = REQUESTED_BYTES.with(Cell::get);
    let (pool, _) = allocations_during(|| Interner::with_capacity(8, 128).unwrap());
    let requested = REQUESTED_BYTES.with(Cell::get) - before;

    // Keep the previous interner's compact storage budget as the ceiling.
    assert!(
        requested <= 128 + 8 * 12 + 16 * 4,
        "requested {requested} bytes"
    );
    std::hint::black_box(pool);
}

#[test]
fn interner_setup_uses_three_allocations_within_the_previous_large_budget() {
    let strings = 5_000;
    let bytes = 128_890;
    let before = REQUESTED_BYTES.with(Cell::get);
    let (pool, allocated) = allocations_during(|| Interner::with_capacity(strings, bytes).unwrap());
    let requested = REQUESTED_BYTES.with(Cell::get) - before;
    assert_eq!(allocated, 3);
    assert!(
        requested <= bytes + strings * 20,
        "requested {requested} bytes"
    );
    std::hint::black_box(pool);
}

#[test]
fn interning_lookup_and_capacity_errors_do_not_allocate_after_setup() {
    let mut pool = Interner::with_capacity(2, 16).unwrap();

    let ((closed, opening), allocated) = allocations_during(|| {
        let closed = pool.intern("closed").unwrap();
        let opening = pool.intern("opening").unwrap();
        assert_eq!(pool.intern("closed"), Ok(closed));
        assert_eq!(pool.resolve(opening), Some("opening"));
        assert_eq!(pool.intern("open"), Err(InternerFull));
        (closed, opening)
    });

    assert_ne!(closed, opening);
    assert_eq!(allocated, 0);
}

#[test]
fn lexer_allocations_do_not_grow_with_graph_count() {
    warm_rule_graphs();
    let source = "graph door\n  node closed end\n".repeat(1_000);
    let mut lexer = Lexer::new(&source);
    while lexer.next().unwrap().unwrap().kind != TokenKind::Indent {}

    let (count, allocated) = allocations_during(|| lexer.map(Result::unwrap).count());

    assert_eq!(count, 8_996);
    assert_eq!(allocated, 0);
}

#[test]
fn parser_allocations_do_not_grow_with_node_count() {
    warm_rule_graphs();
    let source = |nodes: usize| {
        let mut text = String::from("graph door\n");
        for index in 0..nodes {
            text.push_str(&format!("  node closed{index} @anim:idle\n"));
        }
        for index in 0..nodes {
            text.push_str(&format!(
                "  edge closed{index} -> closed{} when request\n",
                (index + 1) % nodes
            ));
        }
        text
    };
    let small = source(8);
    let large = source(5_000);
    let capacity = |nodes, bytes| Capacity {
        nodes,
        edges: nodes,
        conditions: 1,
        annotations: nodes,
        strings: nodes + 4,
        string_bytes: bytes,
        ..Capacity::default()
    };
    let (small_graph, small_allocations) = allocations_during(|| {
        parse(&small, &Schema::default(), &[capacity(8, small.len())]).unwrap()
    });
    let (large_graph, large_allocations) = allocations_during(|| {
        parse(&large, &Schema::default(), &[capacity(5_000, large.len())]).unwrap()
    });

    assert_eq!(small_graph[0].nodes.len(), 8);
    assert_eq!(large_graph[0].nodes.len(), 5_000);
    assert_eq!(large_allocations, small_allocations);
    // The graph lexer, grammar and structure checks each keep a stepper, which allocates a few small
    // buffers once per parse. That is a fixed cost; the equality above is what rules out growth.
    assert!(large_allocations < 32, "{large_allocations} allocations");
}

#[test]
fn parser_storage_excludes_keywords_and_known_node_references() {
    let edges = 1_000;
    let source = format!(
        "graph door\n  node closed loop\n{}",
        "  edge closed -> closed when request\n".repeat(edges)
    );
    let before = REQUESTED_BYTES.with(Cell::get);
    let capacity = Capacity {
        nodes: 1,
        edges,
        conditions: 1,
        strings: 3,
        string_bytes: "doorclosedrequest".len(),
        ..Capacity::default()
    };
    let (graphs, _) =
        allocations_during(|| parse(&source, &Schema::default(), &[capacity]).unwrap());
    let requested = REQUESTED_BYTES.with(Cell::get) - before;
    assert_eq!(graphs[0].edges.len(), edges);
    assert_eq!(graphs[0].conditions.len(), 1);
    // Includes compiled arrays, binding slots, interner bounds, and parser setup.
    assert!(requested <= 128 * edges, "requested {requested} bytes");
}

/// The lexer, grammar and structure graphs are built the first time a parse runs. Do that before a
/// test counts allocations, so the one-time setup is not charged to a parse.
fn warm_rule_graphs() {
    let _ = graphite::pipeline::compile(
        "graph warm\n  node a end\n",
        &graphite::parser::Schema::default(),
    );
}
