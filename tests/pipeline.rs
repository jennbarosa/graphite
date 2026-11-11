use std::io;
use std::path::{Path, PathBuf};

#[path = "support/bun.rs"]
mod bun;

use graphite::frontend::typescript::TYPESCRIPT;
use graphite::frontend::{Frontend, Module, registry};
use graphite::graph::CompiledGraph;
use graphite::parser::Schema;
use graphite::pipeline::{Collision, block_capacities, check_outputs, compile, write_modules};

fn scratch(name: &str) -> PathBuf {
    let directory = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&directory);
    directory
}

#[test]
fn each_graph_block_gets_bounds_from_its_own_lines_and_bytes() {
    let source = "graph a\n  node x end\ngraph b\n  node y\n  node z end\n  edge y -> z\n";
    let capacities = block_capacities(source, &Schema::default());
    assert_eq!(capacities.len(), 2);
    assert_eq!(capacities[0].nodes, 2);
    assert_eq!(capacities[1].nodes, 4);
    assert_eq!(capacities[0].string_bytes, "graph a\n  node x end\n".len());
    assert_eq!(capacities[0].flags, 0);
}

#[test]
fn block_bounds_are_enough_for_a_compile_that_uses_every_line() {
    let source = "graph walk\n  node windup @anim:windup\n  node going loop @anim:walk\n  node stopped end @anim:idle\n  edge windup -> stopped when keyup\n  edge windup -> going\n  edge going -> stopped when keyup\n  choose facing\n    when left -> side @flip:true\n    else -> down\n";
    let schema = Schema {
        node_flags: &["loop"],
    };
    let graphs = compile(source, &schema).unwrap();
    assert_eq!(graphs.len(), 1);
    assert_eq!(graphs[0].nodes.len(), 3);
}

#[test]
fn compile_reports_the_source_offset_of_a_semantic_error() {
    let Err(error) = compile(
        "graph a\n  node x end\n  edge x -> nowhere\n",
        &Schema::default(),
    ) else {
        panic!("an undeclared target must not compile");
    };
    assert!(error.offset > 0);
}

#[test]
fn stray_text_before_any_graph_is_an_error_not_an_empty_success() {
    assert!(compile("node floating end\n", &Schema::default()).is_err());
}

#[test]
fn writes_one_index_and_chunks_per_graph_plus_the_runtime() {
    let directory = scratch("pipeline-typescript");
    let source = "graph door\n  node closed end\ngraph walk\n  node windup end\n";
    let graphs = compile(source, &Schema::default()).unwrap();
    let frontend = registry::lookup("typescript").unwrap().frontend;
    let written = write_modules(frontend, &graphs, &directory).unwrap();
    let names: Vec<_> = written
        .iter()
        .map(|path| path.file_name().unwrap().to_str().unwrap().to_owned())
        .collect();
    assert_eq!(
        names,
        vec![
            "graphite.ts",
            "door.ts",
            "door.nodes.0.ts",
            "walk.ts",
            "walk.nodes.0.ts",
        ]
    );
    for path in &written {
        assert!(path.exists());
    }
    let door = std::fs::read_to_string(directory.join("door.ts")).unwrap();
    assert!(door.contains("export const graph = defineGraph<Vocab>({\nname:\"door\","));
}

#[test]
fn a_source_without_any_graph_line_has_no_block_bounds() {
    assert!(block_capacities("", &Schema::default()).is_empty());
    assert!(block_capacities("   \n", &Schema::default()).is_empty());
}

/// The whole path from DSL text. The lexer and parser build the spec's walk cycle, codegen
/// writes its TypeScript modules, and the bun host in `tests/typescript/walk_host.test.ts`
/// runs them and checks the behavior.
#[test]
fn walk_cycle_runs_through_the_full_pipeline() {
    let directory = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("walk-host");
    let _ = std::fs::remove_dir_all(&directory);
    let source = include_str!("fixtures/runtime/walk_spec.dsl");
    let graphs = compile(source, &Schema::default()).unwrap();
    let frontend = registry::lookup("typescript").unwrap().frontend;
    write_modules(frontend, &graphs, &directory).unwrap();

    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    bun::assert_bun_test_passes(
        manifest,
        "tests/typescript/walk_host.test.ts",
        "GRAPHITE_WALK_HOST",
        &directory,
    );
}

fn typescript() -> &'static dyn graphite::frontend::Frontend {
    registry::lookup("typescript").unwrap().frontend
}

#[test]
fn a_graph_named_like_the_runtime_is_a_caret_error_and_writes_nothing() {
    let directory = scratch("pipeline-runtime-collision");
    let source = "graph graphite\n  node start end\n";
    let graphs = compile(source, &Schema::default()).unwrap();
    let collision = check_outputs(typescript(), &graphs).unwrap_err();
    assert_eq!(
        collision.diagnostic(source).to_string(),
        "error: graph `graphite` would overwrite the runtime file `graphite.ts`\n --> line 1, column 7\n  |\n1 | graph graphite\n  |       ^^^^^^^^ graph `graphite` would overwrite the runtime file `graphite.ts`",
    );
    let error = write_modules(typescript(), &graphs, &directory).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
    assert!(!directory.exists(), "a refused write creates nothing");
}

#[test]
fn graph_names_that_differ_only_in_case_collide_with_a_caret() {
    let source = "graph Door\n  node a end\ngraph door\n  node b end\n";
    let graphs = compile(source, &Schema::default()).unwrap();
    let collision = check_outputs(typescript(), &graphs).unwrap_err();
    assert_eq!(
        collision.diagnostic(source).to_string(),
        "error: graph `door` writes `door.ts`, the same file as graph `Door`\n --> line 3, column 7\n  |\n3 | graph door\n  |       ^^^^ graph `door` writes `door.ts`, the same file as graph `Door`",
    );
}

#[test]
fn an_existing_output_file_is_refused_and_left_untouched() {
    let directory = scratch("pipeline-existing");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("door.ts"), "keep").unwrap();
    let graphs = compile("graph door\n  node closed end\n", &Schema::default()).unwrap();
    let error = write_modules(typescript(), &graphs, &directory).unwrap_err();
    let collision = error
        .get_ref()
        .and_then(|inner| inner.downcast_ref::<Collision>())
        .expect("the error carries the collision");
    assert_eq!(
        collision.to_string(),
        "graph `door` would overwrite the existing file `door.ts` at byte 6"
    );
    assert_eq!(
        std::fs::read_to_string(directory.join("door.ts")).unwrap(),
        "keep"
    );
    assert!(!directory.join("graphite.ts").exists());
}

#[test]
fn an_existing_runtime_file_is_refused_and_left_untouched() {
    let directory = scratch("pipeline-existing-runtime");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("graphite.ts"), "mine").unwrap();
    let graphs = compile("graph door\n  node closed end\n", &Schema::default()).unwrap();
    assert!(write_modules(typescript(), &graphs, &directory).is_err());
    assert_eq!(
        std::fs::read_to_string(directory.join("graphite.ts")).unwrap(),
        "mine"
    );
    assert!(!directory.join("door.ts").exists());
}

/// The TypeScript front end, except that one module's emit fails, as a full disk would.
struct FailingOn(Module);

impl Frontend for FailingOn {
    fn modules(&self, graph: &CompiledGraph) -> Vec<Module> {
        TYPESCRIPT.modules(graph)
    }

    fn file_name(&self, graph: &CompiledGraph, module: Module) -> String {
        TYPESCRIPT.file_name(graph, module)
    }

    fn emit_module(
        &self,
        graph: &CompiledGraph,
        module: Module,
        output: &mut dyn io::Write,
    ) -> io::Result<()> {
        if module == self.0 {
            return Err(io::Error::other("disk full"));
        }
        TYPESCRIPT.emit_module(graph, module, output)
    }

    fn emit_runtime(&self, output: &mut dyn io::Write) -> io::Result<()> {
        TYPESCRIPT.emit_runtime(output)
    }

    fn runtime_file_name(&self) -> &'static str {
        TYPESCRIPT.runtime_file_name()
    }
}

#[test]
fn a_failed_write_removes_what_it_created_so_a_retry_succeeds() {
    let directory = scratch("pipeline-failed-write");
    let graphs = compile(
        "graph door\n  node closed end\ngraph walk\n  node windup end\n",
        &Schema::default(),
    )
    .unwrap();
    let error = write_modules(&FailingOn(Module::Nodes(0)), &graphs, &directory).unwrap_err();
    assert_eq!(error.to_string(), "disk full");
    assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 0);

    write_modules(typescript(), &graphs, &directory).unwrap();
    assert!(directory.join("door.ts").exists());
}
