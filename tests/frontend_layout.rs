//! Layout of the TypeScript chunks: spans are chunk-local, targets and IDs are global, and
//! chunk boundaries fall at exact multiples of the chunk size.

use graphite::frontend::typescript::{TypeScript, chunked};
use graphite::frontend::{Frontend, Module};
use graphite::graph::CompiledGraph;

#[path = "support/parse.rs"]
mod fixture;

/// Two-item chunks, so a five-node graph spans two node chunks.
type Small = TypeScript<2>;

fn module_text(frontend: &Small, graph: &CompiledGraph, module: Module) -> String {
    let mut bytes = Vec::new();
    frontend.emit_module(graph, module, &mut bytes).unwrap();
    String::from_utf8(bytes).unwrap()
}

fn one_graph(source: &str) -> CompiledGraph {
    fixture::parse(source, &Default::default())
        .unwrap()
        .remove(0)
}

const CHAIN: &str = "graph chain
  node a
  node b
  node c
  node d
  node e end
  edge a -> b
  edge b -> c when go
  edge b -> d
  edge c -> e @sfx:done
  edge d -> e
";

const NONE: u32 = u32::MAX;

// Column positions, in the order the emitter writes them. Node columns hold one value per node,
// span columns hold a start and an end per node or edge, and so on.
const NODE_SEQUENCE: usize = 2;
const NODE_BITS: usize = 3;
const NODE_FLAGS: usize = 4;
const NODE_ANNOTATIONS: usize = 5;
const NODE_OUTGOING: usize = 6;
const EDGE_ID: usize = 7;
const EDGE_TARGET: usize = 8;
const EDGE_CONDITION: usize = 9;
const EDGE_ANNOTATIONS: usize = 10;

/// A node chunk's columns, read back from the base64 the module states.
struct Columns {
    nodes: usize,
    edges: usize,
    annotations: usize,
    flags: usize,
    words: Vec<u32>,
}

impl Columns {
    fn parse(text: &str) -> Self {
        let body = text
            .split("decodeNodeChunk({")
            .nth(1)
            .expect("a node chunk decodes its columns");
        let count = |label: &str| -> usize {
            let rest = body.split(label).nth(1).expect("count is stated");
            let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
            digits.parse().expect("count is a number")
        };
        let encoded = body.split('"').nth(1).expect("base64 text is quoted");
        let bytes = base64_decode(encoded);
        let words = bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|four| u32::from_le_bytes(*four))
            .collect();
        Self {
            nodes: count("nodes:"),
            edges: count("edges:"),
            annotations: count("annotations:"),
            flags: count("flags:"),
            words,
        }
    }

    /// Lengths of the columns, in position order.
    fn lengths(&self) -> [usize; 14] {
        let (n, e, a) = (self.nodes, self.edges, self.annotations);
        [
            n,
            n,
            n,
            n,
            2 * n,
            2 * n,
            2 * n,
            e,
            e,
            e,
            2 * e,
            a,
            a,
            self.flags,
        ]
    }

    fn column(&self, position: usize) -> &[u32] {
        let lengths = self.lengths();
        let start: usize = lengths[..position].iter().sum();
        &self.words[start..start + lengths[position]]
    }

    fn word(&self, position: usize, index: usize) -> u32 {
        self.column(position)[index]
    }

    fn pair(&self, position: usize, index: usize) -> [u32; 2] {
        let column = self.column(position);
        [column[2 * index], column[2 * index + 1]]
    }
}

fn base64_decode(text: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let mut buffer = 0u32;
    let mut bits = 0;
    for byte in text.bytes().filter(|byte| *byte != b'=') {
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => panic!("not base64: {byte}"),
        };
        buffer = (buffer << 6) | u32::from(value);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
        }
    }
    out
}

#[test]
fn node_chunks_keep_global_targets_and_chunk_local_spans() {
    let graph = one_graph(CHAIN);
    let frontend = chunked::<2>();
    assert_eq!(
        frontend.modules(&graph),
        vec![Module::Index, Module::Nodes(0), Module::Nodes(1)]
    );

    // Chunk 0 holds nodes a..d and their edges 0..4 in per-node order.
    let first = Columns::parse(&module_text(&frontend, &graph, Module::Nodes(0)));
    assert_eq!((first.nodes, first.edges), (4, 5));
    // b (node 1): gate b->c (local edge 1), sequence b->d (local edge 2), so its sequence is 2.
    assert_eq!(first.pair(NODE_OUTGOING, 1), [1, 3]);
    assert_eq!(first.word(NODE_SEQUENCE, 1), 2);
    // a: its only edge is the sequence edge, at local index 0.
    assert_eq!(first.pair(NODE_OUTGOING, 0), [0, 1]);
    assert_eq!(first.word(NODE_SEQUENCE, 0), 0);
    // The gate (local 1) keeps global edge ID 1, global target c (node 2), and condition 0.
    assert_eq!(first.word(EDGE_ID, 1), 1);
    assert_eq!(first.word(EDGE_TARGET, 1), 2);
    assert_eq!(first.word(EDGE_CONDITION, 1), 0);
    assert_eq!(first.pair(EDGE_ANNOTATIONS, 1), [0, 0]);
    // d -> e (local 4) crosses into chunk 1 by global target ID 4, with no condition.
    assert_eq!(first.word(EDGE_ID, 4), 4);
    assert_eq!(first.word(EDGE_TARGET, 4), 4);
    assert_eq!(first.word(EDGE_CONDITION, 4), NONE);
    // c -> e (local 3) carries its own annotation, stored in this chunk's annotation list.
    assert_eq!(first.word(EDGE_ID, 3), 3);
    assert_eq!(first.pair(EDGE_ANNOTATIONS, 3), [0, 1]);

    // Chunk 1 holds only node e: an end node (bit 2) with no outgoing edges and no edge table.
    let second = Columns::parse(&module_text(&frontend, &graph, Module::Nodes(1)));
    assert_eq!((second.nodes, second.edges), (1, 0));
    assert_eq!(second.word(NODE_BITS, 0), 2);
    assert_eq!(second.pair(NODE_OUTGOING, 0), [0, 0]);
    assert_eq!(second.word(NODE_SEQUENCE, 0), NONE);
    assert_eq!(second.pair(NODE_FLAGS, 0), [0, 0]);
    assert_eq!(second.pair(NODE_ANNOTATIONS, 0), [0, 0]);
}

#[test]
fn choice_chunks_restart_arm_indices_in_each_chunk() {
    let mut source = String::from("graph many\n  node x end\n");
    for index in 0..5 {
        source.push_str(&format!(
            "  choose c{index}\n    when on -> t{index}\n    else -> f{index}\n"
        ));
    }
    let graph = one_graph(&source);
    let frontend = chunked::<2>();

    // Chunk 0 has choices 0..3; chunk 1 has choice 4 alone.
    let first = module_text(&frontend, &graph, Module::Choices(0));
    assert_eq!(first.matches("{name:").count(), 4);

    let second = module_text(&frontend, &graph, Module::Choices(1));
    assert_eq!(second.matches("{name:").count(), 1);
    // The sole choice's arms start at local index 0 and span its two arms.
    assert!(second.contains("arms:[0,2]}"));
    assert!(second.contains("{condition:0,tag:"));
    assert!(second.contains("{condition:null,tag:"));
}

#[test]
fn index_chunk_list_matches_the_exact_chunk_count() {
    let four = one_graph(
        "graph g\n  node a\n  node b\n  node c\n  node d end\n  edge a -> b\n  edge b -> c\n  edge c -> d\n",
    );
    let five = one_graph(
        "graph g\n  node a\n  node b\n  node c\n  node d\n  node e end\n  edge a -> b\n  edge b -> c\n  edge c -> d\n  edge d -> e\n",
    );
    let frontend = chunked::<2>();

    let index = module_text(&frontend, &four, Module::Index);
    assert!(index.contains("nodeChunks: [nodes0,],"));
    assert!(!index.contains("nodes1"));

    let index = module_text(&frontend, &five, Module::Index);
    assert!(index.contains("nodeChunks: [nodes0,nodes1,],"));
}

#[test]
fn a_graph_without_choices_has_an_empty_choice_type_and_no_choice_chunks() {
    let graph = one_graph("graph g\n  node a end\n");
    let index = module_text(&chunked::<2>(), &graph, Module::Index);
    assert!(index.contains("readonly choices: {};"));
    assert!(index.contains("choiceChunks: [],"));
}

#[test]
fn many_choice_chunks_join_into_one_vocabulary_type() {
    let mut source = String::from("graph many\n  node x end\n");
    for index in 0..5 {
        source.push_str(&format!(
            "  choose c{index}\n    when on -> t{index}\n    else -> f{index}\n"
        ));
    }
    let graph = one_graph(&source);
    let index = module_text(&chunked::<2>(), &graph, Module::Index);
    assert!(index.contains("readonly choices: Choices0 & Choices1;"));
    assert!(index.contains("choiceChunks: [choices0,choices1,],"));
}
