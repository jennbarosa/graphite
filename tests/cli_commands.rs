//! The `debug`, `inspect --json` failure, and `run --each` paths that the other CLI tests do not
//! reach: a chosen graph, an allowlist, a front end and output directory, a failure as JSON, and
//! a missing item graph.

#[allow(dead_code)]
#[path = "support/cli.rs"]
mod cli;

use cli::{dsl, graphite, manifest, scratch, stderr, stdout};

fn door() -> std::path::PathBuf {
    manifest().join("examples/door.graphite")
}

#[test]
fn debug_runs_the_named_graph() {
    let output = graphite([
        "debug".as_ref(),
        door().as_os_str(),
        "--graph".as_ref(),
        "door".as_ref(),
        "show".as_ref(),
    ]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(stdout(&output).starts_with("node closed  status running  transitions 0  true: none"));
}

#[test]
fn debug_names_the_graphs_a_file_has_when_the_name_is_wrong() {
    let output = graphite([
        "debug".as_ref(),
        door().as_os_str(),
        "--graph".as_ref(),
        "nope".as_ref(),
        "show".as_ref(),
    ]);
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("no graph named \"nope\"; the file has door"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn debug_applies_an_allowlist_to_the_annotations_it_reads() {
    let allow = dsl("debug_allow.toml", "[annotations]\nanim = [\"idle\"]\n");
    let output = graphite([
        "debug".as_ref(),
        door().as_os_str(),
        "--allow".as_ref(),
        allow.as_os_str(),
        "show".as_ref(),
    ]);
    assert!(!output.status.success());
    assert!(
        stderr(&output)
            .contains("graph door, node opening: @anim:open is not allowed; allowed: idle"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn debug_writes_a_rust_debugger_into_the_output_directory() {
    let out = scratch("debug_rust_out");
    let output = graphite([
        "debug".as_ref(),
        door().as_os_str(),
        "--frontend".as_ref(),
        "rust".as_ref(),
        "--out".as_ref(),
        out.as_os_str(),
        "show".as_ref(),
    ]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(
        stdout(&output).starts_with("node closed"),
        "{}",
        stdout(&output)
    );
}

#[test]
fn inspect_reports_a_failure_as_json_on_stdout() {
    let file = dsl(
        "inspect_json_failure.graphite",
        "graph g\n  node a\n  edge a -> \n",
    );
    let output = graphite(["inspect".as_ref(), file.as_os_str(), "--json".as_ref()]);
    assert!(!output.status.success());
    let text = stdout(&output);
    assert!(
        text.starts_with("{\"ok\":false,\"error\":\"error: expected a node name"),
        "{text}"
    );
}

#[test]
fn run_each_names_an_item_graph_that_is_missing() {
    let items = dsl("run_each_missing_items.json", "{\"items\":[1]}");
    let output = graphite([
        "run".as_ref(),
        door().as_os_str(),
        "--input".as_ref(),
        items.as_os_str(),
        "--each".as_ref(),
        "items".as_ref(),
        "--key".as_ref(),
        "id".as_ref(),
    ]);
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("items: No such file or directory"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn version_prints_the_crate_version() {
    let output = graphite(["--version"]);
    assert!(output.status.success());
    assert_eq!(
        stdout(&output),
        format!("graphite {}\n", env!("CARGO_PKG_VERSION"))
    );
}
