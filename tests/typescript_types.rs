//! The generated TypeScript types must accept real names and reject typos. The bun tests only
//! run the code, so this test runs the TypeScript compiler (`tsc`) over a host file.

#[allow(dead_code)]
#[path = "support/tools.rs"]
mod tools;

use std::path::PathBuf;

use graphite::frontend::registry;
use graphite::parser::Schema;
use graphite::pipeline::{compile, write_modules};

/// Each `@ts-expect-error` line fails the check if its line has no error, so the typos must be
/// rejected and the real names must be accepted.
const HOST: &str = r#"import { graph } from "./walk";
import { start } from "./graphite";

const run = start(graph);
run.set("keyup", true);
const step = run.step();
const node: "windup" | "going" | "winddown" | "stopped" = step.node;
start(graph).choose("facing");

// @ts-expect-error unknown choice
start(graph).choose("facng");
// @ts-expect-error unknown condition
run.set("keyp", true);
// @ts-expect-error a node name is not a node
const wrong: "nowhere" = step.node;

export { node, wrong };
"#;

#[test]
fn generated_types_accept_real_names_and_reject_typos() {
    // `GRAPHITE_TSC` names the compiler binary; otherwise the copy in `tools/` is used.
    let directory = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("typescript-types");
    let _ = std::fs::remove_dir_all(&directory);
    let source = include_str!("fixtures/runtime/walk_spec.graphite");
    let graphs = compile(source, &Schema::default()).unwrap();
    let frontend = registry::lookup("typescript").unwrap().frontend;
    write_modules(frontend, &graphs, &directory).unwrap();
    std::fs::write(directory.join("host.ts"), HOST).unwrap();

    let output = tools::command("tsc", "GRAPHITE_TSC")
        .current_dir(&directory)
        .args([
            "--noEmit",
            "--strict",
            "--noUncheckedIndexedAccess",
            "--exactOptionalPropertyTypes",
            "--verbatimModuleSyntax",
            "--target",
            "es2022",
            "--module",
            "esnext",
            "--moduleResolution",
            "bundler",
            "host.ts",
        ])
        .output()
        .unwrap_or_else(|error| {
            panic!("cannot run tsc: {error}; run scripts/setup-tools.sh or set GRAPHITE_TSC")
        });
    assert!(
        output.status.success(),
        "tsc rejected the generated types:\n{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
