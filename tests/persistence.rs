//! Persistence across processes: a snapshot written by one process resumes in another, and a
//! changed workflow refuses it. Runs `tests/typescript/persistence.test.ts` on compiled output.

#[allow(dead_code)]
#[path = "support/bun.rs"]
mod bun;
#[allow(dead_code)]
#[path = "support/cli.rs"]
mod cli;

use std::path::{Path, PathBuf};

use graphite::frontend::registry;
use graphite::parser::Schema;
use graphite::pipeline::{compile, write_modules};

/// Compiles the workflow DSL at `path` into a fresh directory and returns that directory.
fn compile_workflow(name: &str, path: &Path) -> PathBuf {
    let out = cli::scratch(name);
    let source = std::fs::read_to_string(path).unwrap();
    let graphs = compile(
        &source,
        &Schema {
            node_flags: &["retry"],
        },
    )
    .unwrap();
    let frontend = registry::lookup("typescript").unwrap().frontend;
    write_modules(frontend, &graphs, &out).unwrap();
    out
}

#[test]
fn a_snapshot_survives_a_process_restart_and_a_changed_workflow_refuses_it() {
    let fixtures = cli::manifest().join("tests/fixtures/runtime");
    let review = compile_workflow("persistence-review", &fixtures.join("workflow.dsl"));
    let variant = compile_workflow(
        "persistence-variant",
        &fixtures.join("workflow_variant.dsl"),
    );
    bun::assert_bun_test_passes_with(
        cli::manifest(),
        "tests/typescript/persistence.test.ts",
        &[
            ("GRAPHITE_WORKFLOW", review.join("review.ts")),
            ("GRAPHITE_VARIANT", variant.join("review.ts")),
            ("GRAPHITE_STATE_DIR", review.clone()),
        ],
    );
}
