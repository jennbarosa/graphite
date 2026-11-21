//! The command-line entry scripts (`durable/cli.ts`, `durable/simulate.ts`, `cli/debug.ts`) run
//! in-process under Bun, so their code is measured. The graphs they read are compiled here, as
//! production does, and passed to `tests/typescript/entry_scripts.test.ts` by environment variable.

#[allow(dead_code)]
#[path = "support/bun.rs"]
mod bun;
#[allow(dead_code)]
#[path = "support/cli.rs"]
mod cli;
#[allow(dead_code)]
#[path = "support/tools.rs"]
mod tools;

use cli::{graphite, manifest, scratch, stderr};
use std::fs;
use std::path::{Path, PathBuf};

/// Compiles `source` with `allow` (if any) into a fresh `out` directory, and returns the graph's
/// module: the first file `compile` prints that is not the runtime. A module is named after its
/// graph, so the file name is not the source's.
fn compile(source: &Path, allow: Option<&Path>, out: &Path) -> PathBuf {
    let mut args: Vec<&std::ffi::OsStr> = vec![
        "compile".as_ref(),
        source.as_os_str(),
        "--out".as_ref(),
        out.as_os_str(),
    ];
    if let Some(allow) = allow {
        args.extend(["--allow".as_ref(), allow.as_os_str()]);
    }
    let output = graphite(args);
    assert!(output.status.success(), "{}", stderr(&output));
    let printed = cli::stdout(&output);
    printed
        .lines()
        .map(PathBuf::from)
        .find(|path| path.file_name().is_some_and(|name| name != "graphite.ts"))
        .expect("compile prints the graph's module")
}

/// The fan-out pair: a list graph that emits the items, and an item graph run once per item.
fn fan_out_graphs(dir: &Path) -> (PathBuf, PathBuf) {
    fs::create_dir_all(dir).unwrap();
    fs::write(
        dir.join("list.graphite"),
        "graph list\n  node find @task:discover\n  node done end @task:emit\n\n  edge find -> done when passed\n  edge find -> done when failed\n",
    )
    .unwrap();
    fs::write(
        dir.join("item.graphite"),
        "graph item\n  node audit @task:audit\n  node save end @task:save\n  node skip end\n\n  edge audit -> save when passed\n  edge audit -> skip when failed\n",
    )
    .unwrap();
    fs::write(
        dir.join("allow.toml"),
        "node_flags = []\n\n[annotations]\ntask = [\"discover\", \"emit\", \"audit\", \"save\"]\n",
    )
    .unwrap();
    let list = compile(
        &dir.join("list.graphite"),
        Some(&dir.join("allow.toml")),
        &dir.join("list"),
    );
    let item = compile(
        &dir.join("item.graphite"),
        Some(&dir.join("allow.toml")),
        &dir.join("item"),
    );
    (list, item)
}

#[test]
fn the_entry_scripts_pass_their_bun_tests() {
    let fixtures = manifest().join("tests/fixtures/runtime");
    let dir = scratch("entry-scripts");
    let workflow = compile(
        &fixtures.join("workflow.graphite"),
        Some(&fixtures.join("workflow.toml")),
        &dir.join("workflow"),
    );
    let walk = compile(
        &fixtures.join("walk_spec.graphite"),
        None,
        &dir.join("walk"),
    );
    let (list, item) = fan_out_graphs(&dir.join("fan"));
    bun::assert_bun_test_passes_with(
        manifest(),
        "tests/typescript/entry_scripts.test.ts",
        &[
            ("GRAPHITE_ENTRY_WORKFLOW", workflow),
            ("GRAPHITE_ENTRY_WALK", walk),
            ("GRAPHITE_ENTRY_LIST", list),
            ("GRAPHITE_ENTRY_ITEM", item),
            ("GRAPHITE_ENTRY_RUNNER", fixtures.join("pass_runner.ts")),
            ("GRAPHITE_ENTRY_DIR", dir),
        ],
    );
}
