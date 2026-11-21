//! Workflow graphs copied from real projects that use graphite as a submodule: the cold-email
//! outreach pair (barosa-dev), the daily ads autopilot (gads), a game NPC brain (hotpotato-ai-RL)
//! and a player animation graph (barosafun). Every `.graphite` file in `tests/fixtures/workflows`
//! must check, using its sibling `.toml` allowlist when it has one, and must compile, so a language
//! change that breaks a real use fails the regression run. A new real-usage graph needs only a
//! new file in that directory.

#[allow(dead_code)]
#[path = "support/cli.rs"]
mod cli;

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Output;

use cli::{graphite, manifest, scratch, stderr, stdout};

/// The number of real-usage graphs the fixtures directory must hold, so a lost file fails loudly.
const REAL_WORKFLOW_COUNT: usize = 5;

/// Every workflow graph, sorted, with at least `REAL_WORKFLOW_COUNT` present.
fn workflows() -> Vec<PathBuf> {
    let dir = manifest().join("tests/fixtures/workflows");
    let mut graphs: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|error| panic!("{}: {error}", dir.display()))
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "graphite"))
        .collect();
    graphs.sort();
    assert!(
        graphs.len() >= REAL_WORKFLOW_COUNT,
        "expected the real-usage graphs, found {}",
        graphs.len()
    );
    graphs
}

/// The allowlist beside a graph (`name.toml` for `name.graphite`), when there is one.
fn allowlist(graph: &Path) -> Option<PathBuf> {
    let toml = graph.with_extension("toml");
    toml.exists().then_some(toml)
}

/// Runs `subcommand` on `graph`, with `--out` when given and the graph's sibling allowlist.
fn run(subcommand: &str, graph: &Path, out: Option<&Path>) -> Output {
    let mut args: Vec<&OsStr> = vec![subcommand.as_ref(), graph.as_os_str()];
    if let Some(out) = out {
        args.extend(["--out".as_ref(), out.as_os_str()]);
    }
    let allow = allowlist(graph);
    if let Some(allow) = &allow {
        args.extend(["--allow".as_ref(), allow.as_os_str()]);
    }
    graphite(args)
}

#[test]
fn every_real_workflow_checks() {
    for graph in workflows() {
        let output = run("check", &graph, None);
        assert!(
            output.status.success(),
            "{}: {}",
            graph.display(),
            stderr(&output)
        );
        assert!(
            stdout(&output).starts_with("ok:"),
            "{}: {}",
            graph.display(),
            stdout(&output)
        );
    }
}

#[test]
fn every_real_workflow_compiles() {
    for graph in workflows() {
        let name = graph.file_stem().unwrap().to_string_lossy().into_owned();
        let out = scratch(&format!("workflows-{name}"));
        let output = run("compile", &graph, Some(&out));
        assert!(
            output.status.success(),
            "{}: {}",
            graph.display(),
            stderr(&output)
        );
        assert!(
            out.join("graphite.ts").exists(),
            "{}: no runtime module written",
            graph.display()
        );
    }
}
