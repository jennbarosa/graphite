//! `graphite check`: validates a file and its output names, and writes nothing.

#[allow(dead_code)]
#[path = "support/cli.rs"]
mod cli;

use cli::{dsl, graphite, stderr, stdout, walk_spec};

#[test]
fn reports_the_graph_count() {
    let spec = walk_spec();
    let output = graphite(["check".as_ref(), spec.as_os_str()]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stdout(&output), "ok: 1 graph\n");
}

#[test]
fn rejects_two_graphs_whose_outputs_collide() {
    let source = dsl(
        "cli-collide/collide.graphite",
        "graph Walk\n  node a end\n\ngraph walk\n  node a end\n",
    );
    let output = graphite(["check".as_ref(), source.as_os_str()]);
    assert!(!output.status.success());
    assert!(stderr(&output).starts_with("graphite:"));
}

#[test]
fn rejects_a_dead_end_with_its_source_location() {
    let source = dsl("cli-dead-end/dead.graphite", "graph bad\n  node a\n");
    let output = graphite(["check".as_ref(), source.as_os_str()]);
    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).starts_with("graphite:"));
}

#[test]
fn takes_exactly_one_file() {
    let spec = walk_spec();
    let output = graphite([
        "check".as_ref(),
        spec.as_os_str(),
        "extra.graphite".as_ref(),
    ]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("check takes one file"));
}

const REVIEW: &str = "graph review\n  node draft @task:write\n  node check @task:lint\n  node ship end @task:deploy\n  edge draft -> check\n  edge check -> ship when passed @report:ok\n";

#[test]
fn allow_accepts_values_on_the_list() {
    let source = dsl("cli-allow-ok/review.graphite", REVIEW);
    let allow = dsl(
        "cli-allow-ok/allow.toml",
        "[annotations]\ntask = [\"write\", \"lint\", \"deploy\"]\nreport = [\"ok\"]\n",
    );
    let output = graphite([
        "check".as_ref(),
        source.as_os_str(),
        "--allow".as_ref(),
        allow.as_os_str(),
    ]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stdout(&output), "ok: 1 graph\n");
}

#[test]
fn allow_names_the_node_holding_a_value_off_the_list() {
    let source = dsl("cli-allow-bad/review.graphite", REVIEW);
    let allow = dsl(
        "cli-allow-bad/allow.toml",
        "[annotations]\ntask = [\"write\", \"lint\"]\n",
    );
    let output = graphite([
        "check".as_ref(),
        source.as_os_str(),
        "--allow".as_ref(),
        allow.as_os_str(),
    ]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("graph review, node ship: @task:deploy is not allowed"));
}

#[test]
fn allow_refuses_a_bare_annotation_under_a_listed_key() {
    let source = dsl(
        "cli-allow-bare/bare.graphite",
        "graph bare\n  node draft @task\n  node done end\n  edge draft -> done\n",
    );
    let allow = dsl(
        "cli-allow-bare/allow.toml",
        "[annotations]\ntask = [\"write\"]\n",
    );
    let output = graphite([
        "check".as_ref(),
        source.as_os_str(),
        "--allow".as_ref(),
        allow.as_os_str(),
    ]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("@task needs a value"));
}

#[test]
fn allow_rejects_sections_it_does_not_know() {
    let source = dsl("cli-allow-section/review.graphite", REVIEW);
    let allow = dsl(
        "cli-allow-section/allow.toml",
        "[annotations]\ntask = [\"write\"]\n[extra]\nx = 1\n",
    );
    let output = graphite([
        "check".as_ref(),
        source.as_os_str(),
        "--allow".as_ref(),
        allow.as_os_str(),
    ]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("unknown key [extra]"));
}

/// `check --json` prints `{"ok":true,"graphs":N}` when the graph checks, and
/// `{"ok":false,"error":...}` on stdout, with exit status 1, when it does not.
#[test]
fn check_json_reports_the_graph_count_or_the_error() {
    let spec = walk_spec();
    let good = graphite(["check".as_ref(), spec.as_os_str(), "--json".as_ref()]);
    assert!(good.status.success());
    assert_eq!(stdout(&good).trim(), r#"{"ok":true,"graphs":1}"#);

    let missing = graphite(["check", "no-such-file.graphite", "--json"]);
    assert_eq!(missing.status.code(), Some(1));
    assert!(stderr(&missing).is_empty());
    let line = stdout(&missing);
    assert!(
        line.starts_with(r#"{"ok":false,"error":"no-such-file.graphite"#),
        "{line}"
    );
}
