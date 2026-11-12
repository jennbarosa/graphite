//! `graphite compile`: writes the runtime and graph modules, and never overwrites.

#[allow(dead_code)]
#[path = "support/cli.rs"]
mod cli;

use cli::{graphite, manifest, scratch, stderr, walk_spec};

#[test]
fn writes_the_runtime_and_the_graph_modules() {
    let out = scratch("cli-compile");
    let spec = walk_spec();
    let output = graphite([
        "compile".as_ref(),
        spec.as_os_str(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(out.join("graphite.ts").is_file());
    assert!(out.join("walk.ts").is_file());
}

#[test]
fn refuses_to_overwrite_an_existing_output() {
    let out = scratch("cli-overwrite");
    let spec = walk_spec();
    let args = [
        "compile".as_ref(),
        spec.as_os_str(),
        "--out".as_ref(),
        out.as_os_str(),
    ];
    assert!(graphite(args).status.success());
    assert!(!graphite(args).status.success());
}

#[test]
fn rejects_an_unknown_front_end_without_writing() {
    let out = scratch("cli-unknown-front-end");
    let spec = walk_spec();
    let output = graphite([
        "compile".as_ref(),
        spec.as_os_str(),
        "--frontend".as_ref(),
        "cobol".as_ref(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("unknown front end \"cobol\""));
    assert!(!out.exists());
}

/// `--allow` applies to `compile`: a graph that uses an unlisted flag is refused, and the same graph
/// compiles once the allowlist names the flag. `--frontend typescript` names the default explicitly.
#[test]
fn compile_applies_the_allowlist_and_accepts_the_front_end_by_name() {
    let fixtures = manifest().join("tests/fixtures/runtime");
    let dsl = fixtures.join("workflow.dsl");
    let allow = fixtures.join("workflow.toml");
    let out = scratch("cli-compile-allow");

    let refused = graphite([
        "compile".as_ref(),
        dsl.as_os_str(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);
    assert!(!refused.status.success());
    assert!(stderr(&refused).contains("unknown node flag"));
    assert!(!out.exists());

    let accepted = graphite([
        "compile".as_ref(),
        dsl.as_os_str(),
        "--frontend".as_ref(),
        "typescript".as_ref(),
        "--allow".as_ref(),
        allow.as_os_str(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);
    assert!(accepted.status.success(), "{}", stderr(&accepted));
    assert!(out.join("review.ts").exists());
}
