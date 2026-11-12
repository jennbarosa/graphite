//! The host-facing runtime, run under Bun against two compiled graphs: the workflow fixture,
//! which uses domain flags, annotations and edge annotations, and the walk spec.

#[allow(dead_code)]
#[path = "support/bun.rs"]
mod bun;
#[allow(dead_code)]
#[path = "support/cli.rs"]
mod cli;

use graphite::frontend::registry;
use graphite::parser::Schema;
use graphite::pipeline::{compile, write_modules};

#[test]
fn host_accessors_workflow_host_and_persistence_run_on_the_fixtures() {
    let out = cli::scratch("runtime-accessors");
    let workflow =
        std::fs::read_to_string(cli::manifest().join("tests/fixtures/runtime/workflow.graphite"))
            .unwrap();
    let mut graphs = compile(
        &workflow,
        &Schema {
            node_flags: &["retry"],
        },
    )
    .unwrap();
    let walk = std::fs::read_to_string(cli::walk_spec()).unwrap();
    graphs.extend(compile(&walk, &Schema::default()).unwrap());
    let frontend = registry::lookup("typescript").unwrap().frontend;
    write_modules(frontend, &graphs, &out).unwrap();
    for file in [
        "tests/typescript/graph_accessors.test.ts",
        "tests/typescript/workflow_host.test.ts",
    ] {
        bun::assert_bun_test_passes(
            cli::manifest(),
            file,
            "GRAPHITE_WORKFLOW",
            &out.join("review.ts"),
        );
    }
}
