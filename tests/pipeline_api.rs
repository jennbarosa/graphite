//! The pipeline's library entry points, used without the command line: capacity bounds for a
//! source, compiling a body, naming a file, and reporting an output collision with its caret.

use std::path::Path;

use graphite::frontend::registry;
use graphite::parser::Schema;
use graphite::pipeline::build::{Chosen, compile_file, file_name};
use graphite::pipeline::{block_capacities, check_outputs, compile_body};

const TWO_GRAPHS: &str =
    "graph a\n  node x\n  node y end\n  edge x -> y\n\ngraph b\n  node p end\n";

fn typescript() -> &'static registry::Target {
    registry::lookup("typescript").expect("the typescript front end is registered")
}

#[test]
fn block_capacities_gives_one_bound_per_graph() {
    let bounds = block_capacities(TWO_GRAPHS, &Schema::default());
    assert_eq!(bounds.len(), 2);
    assert!(bounds[0].nodes >= 2, "{:?}", bounds[0]);
    assert!(bounds[0].edges >= 1, "{:?}", bounds[0]);
    assert!(bounds[1].nodes >= 1, "{:?}", bounds[1]);
}

#[test]
fn block_capacities_of_text_before_any_graph_is_empty() {
    assert!(block_capacities("", &Schema::default()).is_empty());
}

#[test]
fn compile_body_parses_the_graphs_after_the_directives() {
    let graphs = compile_body(TWO_GRAPHS, 0, &Schema::default()).unwrap();
    let names: Vec<&str> = graphs.iter().map(|graph| graph.text(graph.name)).collect();
    assert_eq!(names, ["a", "b"]);
}

#[test]
fn compile_file_chooses_a_front_end_and_compiles_its_graphs() {
    let select = |name: &str, _: &graphite::preprocess::Directives<'_>| -> Result<Chosen, String> {
        assert_eq!(name, "door.graphite");
        Ok(Chosen {
            target: typescript(),
            pure: false,
            because: None,
        })
    };
    let source = "@!frontend:typescript\ngraph door\n  node a\n  node b end\n  edge a -> b\n";
    let (chosen, graphs) =
        compile_file(source, Path::new("dir/door.graphite"), &[], &select).unwrap();
    assert_eq!(chosen.target.key, "typescript");
    assert_eq!(graphs.len(), 1);
}

#[test]
fn file_name_is_the_last_component_or_empty() {
    assert_eq!(file_name(Path::new("dir/door.graphite")), "door.graphite");
    assert_eq!(file_name(Path::new("door.graphite")), "door.graphite");
    assert_eq!(file_name(Path::new("/")), "");
}

#[test]
fn a_collision_names_its_graphs_and_renders_at_the_later_one() {
    let source = "graph door\n  node a\n  node b end\n  edge a -> b\n\ngraph Door\n  node a\n  node b end\n  edge a -> b\n";
    let graphs = compile_body(source, 0, &Schema::default()).unwrap();
    let collision = check_outputs(typescript().frontend, &graphs).unwrap_err();
    assert!(!collision.is_existing(), "no output directory was given");
    assert!(
        collision
            .to_string()
            .starts_with("graph `Door` writes `Door.ts`, the same file as graph `door` at byte "),
        "{collision}"
    );
    let rendered = collision.diagnostic(source).to_string();
    assert!(rendered.contains("--> line 6, column 7"), "{rendered}");
}

#[test]
fn distinct_graphs_pass_the_output_check() {
    let graphs = compile_body(TWO_GRAPHS, 0, &Schema::default()).unwrap();
    assert!(check_outputs(typescript().frontend, &graphs).is_ok());
}
