use std::io::{self, Write};

use graphite::frontend::{Frontend, Module};
use graphite::graph::CompiledGraph;
use graphite::interner::Interner;

struct StringTable;

impl Frontend for StringTable {
    fn modules(&self, _: &CompiledGraph) -> Vec<Module> {
        vec![Module::Index]
    }

    fn file_name(&self, _: &CompiledGraph, _: Module) -> String {
        "strings.txt".into()
    }

    fn emit_module(
        &self,
        graph: &CompiledGraph,
        _: Module,
        output: &mut dyn Write,
    ) -> io::Result<()> {
        for value in graph.strings.strings() {
            writeln!(output, "{value}")?;
        }
        Ok(())
    }

    fn emit_runtime(&self, _: &mut dyn Write) -> io::Result<()> {
        Ok(())
    }

    fn runtime_file_name(&self) -> &'static str {
        "strings.ts"
    }
}

#[test]
fn borrowing_iteration_preserves_insertion_order_and_zero_based_ids() {
    let mut strings = Interner::with_capacity(4, 64).unwrap();
    let names = ["", "é", "closed", "中"];
    let ids: Vec<_> = names
        .iter()
        .map(|name| strings.intern(name).unwrap())
        .collect();
    assert_eq!(strings.intern("é"), Ok(ids[1]));
    for (index, (value, id)) in strings.strings().zip(&ids).enumerate() {
        assert_eq!(id.index(), index);
        assert_eq!(value, names[index]);
        assert_eq!(value.as_ptr(), strings.resolve(*id).unwrap().as_ptr());
    }
    assert_eq!(strings.strings().len(), names.len());
    assert_eq!(Interner::with_capacity(0, 0).unwrap().strings().len(), 0);
}

#[path = "support/parse.rs"]
mod fixture;

#[test]
fn independent_adapter_uses_borrowed_graphs_and_caller_owned_output() {
    let graphs = fixture::parse("graph door\n  node closed end\n", &Default::default()).unwrap();
    let mut output = Vec::with_capacity(64);
    let adapter: &dyn Frontend = &StringTable;
    adapter
        .emit_module(&graphs[0], Module::Index, &mut output)
        .unwrap();
    assert_eq!(output, b"door\nclosed\n");
    assert_eq!(graphs[0].strings.resolve(graphs[0].name), Some("door"));
}

#[test]
fn independent_adapter_propagates_the_original_write_error() {
    struct Broken;

    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "closed sink"))
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    let graphs = fixture::parse("graph door\n  node closed end\n", &Default::default()).unwrap();
    let error = StringTable
        .emit_module(&graphs[0], Module::Index, &mut Broken)
        .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
    assert_eq!(error.to_string(), "closed sink");
}
