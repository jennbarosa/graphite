//! Unit tests for the durable runner's TypeScript helpers, run under Bun: the `debug --simulate`
//! stubs and their output lines, and the graph checks the entry points share.

#[allow(dead_code)]
#[path = "support/bun.rs"]
mod bun;

use std::path::{Path, PathBuf};

fn no_variables() -> Vec<(&'static str, PathBuf)> {
    Vec::new()
}

#[test]
fn simulate_stubs_and_their_lines_pass_their_bun_tests() {
    bun::assert_bun_test_passes_with(
        Path::new(env!("CARGO_MANIFEST_DIR")),
        "tests/typescript/simulate.test.ts",
        &no_variables(),
    );
}

#[test]
fn graph_checks_pass_their_bun_tests() {
    bun::assert_bun_test_passes_with(
        Path::new(env!("CARGO_MANIFEST_DIR")),
        "tests/typescript/graph_checks.test.ts",
        &no_variables(),
    );
}
