//! `graphite compile`: writes the runtime and graph modules, and never overwrites.

#[allow(dead_code)]
#[path = "support/cli.rs"]
mod cli;

use cli::{graphite, scratch, stderr, walk_spec};

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
