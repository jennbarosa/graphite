//! The durable workflow fixture needs its allowlist.

#[allow(dead_code)]
#[path = "support/cli.rs"]
mod cli;

use cli::{graphite, stderr};

#[test]
fn the_workflow_needs_its_allowlist_for_the_retry_flag() {
    let workflow = cli::manifest().join("tests/fixtures/runtime/workflow.graphite");
    let output = graphite(["check".as_ref(), workflow.as_os_str()]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("unknown node flag"));
}
