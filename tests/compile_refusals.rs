//! `graphite compile` refuses outputs it cannot write, and says which graph and file clash. Each
//! case runs the binary against a fresh output directory.

#[allow(dead_code)]
#[path = "support/cli.rs"]
mod cli;

use cli::{dsl, graphite, scratch, stderr, stdout};
use std::fs;

const DOOR: &str = "graph door\n  node a\n  node b end\n  edge a -> b\n";

#[test]
fn graphs_whose_names_differ_only_in_case_clash() {
    let file = dsl(
        "compile_case_clash.graphite",
        &format!("{DOOR}\ngraph Door\n  node a\n  node b end\n  edge a -> b\n"),
    );
    let out = scratch("compile_case_clash_out");
    let output = graphite([
        "compile".as_ref(),
        file.as_os_str(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("graph `Door` writes `Door.ts`, the same file as graph `door`"),
        "{}",
        stderr(&output)
    );
    assert!(!out.exists(), "a refused compile writes nothing");
}

#[test]
fn a_graph_named_like_the_runtime_is_refused() {
    let file = dsl(
        "compile_runtime_name.graphite",
        "graph graphite\n  node a\n  node b end\n  edge a -> b\n",
    );
    let out = scratch("compile_runtime_name_out");
    let output = graphite([
        "compile".as_ref(),
        file.as_os_str(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("would overwrite the runtime file `graphite.ts`"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn an_existing_module_is_never_overwritten() {
    let file = dsl("compile_existing_module.graphite", DOOR);
    let out = scratch("compile_existing_module_out");
    fs::create_dir_all(&out).unwrap();
    fs::write(out.join("door.ts"), "kept").unwrap();
    let output = graphite([
        "compile".as_ref(),
        file.as_os_str(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);
    assert!(!output.status.success());
    let text = stderr(&output);
    assert!(
        text.contains("would overwrite the existing file `door.ts`"),
        "{text}"
    );
    assert!(text.contains("hint: remove"), "{text}");
    assert_eq!(fs::read_to_string(out.join("door.ts")).unwrap(), "kept");
}

#[test]
fn an_existing_runtime_is_never_overwritten() {
    let file = dsl("compile_existing_runtime.graphite", DOOR);
    let out = scratch("compile_existing_runtime_out");
    fs::create_dir_all(&out).unwrap();
    fs::write(out.join("graphite.ts"), "kept").unwrap();
    let output = graphite([
        "compile".as_ref(),
        file.as_os_str(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);
    assert!(!output.status.success());
    assert!(
        stderr(&output)
            .contains("the runtime file `graphite.ts` already exists in the output directory"),
        "{}",
        stderr(&output)
    );
    assert_eq!(fs::read_to_string(out.join("graphite.ts")).unwrap(), "kept");
}

#[test]
fn a_pure_file_is_refused_by_a_machine_front_end() {
    let file = dsl(
        "compile_pure_on_machine.pure.graphite",
        "graph combat\n  enum E\n    a\n  input i : E\n  choose c : E\n    else -> a\n",
    );
    let out = scratch("compile_pure_on_machine_out");
    let output = graphite([
        "compile".as_ref(),
        file.as_os_str(),
        "--frontend".as_ref(),
        "typescript".as_ref(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("is a machine front end (pure front ends: purescript)"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn the_rust_front_end_writes_its_module_and_runtime_and_refuses_a_second_run() {
    let file = dsl("compile_rust_twice.graphite", DOOR);
    let out = scratch("compile_rust_twice_out");
    let first = graphite([
        "compile".as_ref(),
        file.as_os_str(),
        "--frontend".as_ref(),
        "rust".as_ref(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);
    assert!(first.status.success(), "{}", stderr(&first));
    assert!(stdout(&first).contains("door.rs"));
    assert!(out.join("graphite.rs").is_file());
    let second = graphite([
        "compile".as_ref(),
        file.as_os_str(),
        "--frontend".as_ref(),
        "rust".as_ref(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);
    assert!(!second.status.success());
    // The runtime is checked before the modules, so it is the file named.
    assert!(stderr(&second).contains("the runtime file `graphite.rs` already exists"));
}
