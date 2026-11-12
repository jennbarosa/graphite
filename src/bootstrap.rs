//! The bundled rule files, rebuilt from the compiled form that `build.rs` writes to `OUT_DIR`.
//! Nothing here parses source text: the converter in `examples/bootstrap_dump.rs` already did.

#[expect(
    unused_imports,
    reason = "each generated module names only the variants its graph uses"
)]
use crate::graph::{Annotation, Arm, Choice, CompiledGraph, Edge, Node};
use crate::interner::{Interner, StringId};

/// The string with zero-based index `index` in its graph's string table.
pub(crate) fn id(index: u32) -> StringId {
    StringId::from_index(index)
}

/// A string table holding `strings` in order, so each string gets the ID the converter recorded.
pub(crate) fn interner(strings: &[&str], bytes: usize) -> Interner {
    const FULL: &str = "bundled string table does not fit its recorded size";
    let mut table = Interner::with_capacity(strings.len(), bytes).expect(FULL);
    for text in strings {
        table.intern(text).expect(FULL);
    }
    table
}

pub(crate) mod lex {
    use super::*;
    include!(concat!(env!("OUT_DIR"), "/lex.rs"));
}

pub(crate) mod grammar {
    use super::*;
    include!(concat!(env!("OUT_DIR"), "/grammar.rs"));
}

pub(crate) mod structure {
    use super::*;
    include!(concat!(env!("OUT_DIR"), "/structure.rs"));
}
