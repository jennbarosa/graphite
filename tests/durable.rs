//! The durable host: retries, timeouts, locking and crash recovery. Compiles the review workflow
//! with the `graphite` binary, as production does, then runs
//! `tests/typescript/durable_host.test.ts` on that output.

#[allow(dead_code)]
#[path = "support/bun.rs"]
mod bun;
#[allow(dead_code)]
#[path = "support/cli.rs"]
mod cli;

use cli::{graphite, scratch, stderr};

#[test]
fn the_workflow_needs_its_allowlist_for_the_retry_flag() {
    let workflow = cli::manifest().join("tests/fixtures/runtime/workflow.dsl");
    let output = graphite(["check".as_ref(), workflow.as_os_str()]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("unknown node flag"));
}

#[test]
fn the_durable_host_retries_recovers_and_never_reruns_a_task_it_recorded() {
    let fixtures = cli::manifest().join("tests/fixtures/runtime");
    let out = scratch("durable-review");
    let output = graphite([
        "compile".as_ref(),
        fixtures.join("workflow.dsl").as_os_str(),
        "--out".as_ref(),
        out.as_os_str(),
        "--allow".as_ref(),
        fixtures.join("workflow.toml").as_os_str(),
    ]);
    assert!(output.status.success(), "{}", stderr(&output));
    bun::assert_bun_test_passes_with(
        cli::manifest(),
        "tests/typescript/durable_host.test.ts",
        &[
            ("GRAPHITE_WORKFLOW", out.join("review.ts")),
            ("GRAPHITE_STATE_DIR", out.clone()),
        ],
    );
}

#[test]
fn a_passed_gate_on_a_middle_stage_does_not_skip_its_task() {
    let fixtures = cli::manifest().join("tests/fixtures/runtime");
    let out = scratch("durable-stage-order");
    let output = graphite([
        "compile".as_ref(),
        fixtures.join("stage_order.dsl").as_os_str(),
        "--out".as_ref(),
        out.as_os_str(),
        "--allow".as_ref(),
        fixtures.join("stage_order.toml").as_os_str(),
    ]);
    assert!(output.status.success(), "{}", stderr(&output));
    bun::assert_bun_test_passes_with(
        cli::manifest(),
        "tests/typescript/durable_stage_order.test.ts",
        &[
            ("GRAPHITE_STAGE_ORDER", out.join("stage_order.ts")),
            ("GRAPHITE_STATE_DIR", out.clone()),
        ],
    );
}
