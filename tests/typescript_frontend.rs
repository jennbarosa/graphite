//! Merged test file. Each section below was `tests/<name>.rs` before; see git history.

#[path = "support/parse.rs"]
mod fixture;

#[allow(dead_code)]
#[path = "support/bun.rs"]
mod bun;

#[allow(dead_code)]
#[path = "support/tools.rs"]
mod tools;

#[allow(dead_code)]
#[path = "support/cli.rs"]
mod cli;

// Merged from `tests/typescript_frontend.rs`.
mod typescript_frontend {

    #[allow(unused_imports)]
    use super::bun;
    #[allow(unused_imports)]
    use super::fixture;
    #[allow(unused_imports)]
    use super::tools;

    use std::io::{self, Write};

    use graphite::frontend::typescript::{RUNTIME_FILE, TYPESCRIPT, TypeScript, chunked};
    use graphite::frontend::{Frontend, Module, registry};
    use graphite::graph::CompiledGraph;

    const DEFAULT: TypeScript = TYPESCRIPT;

    /// Every module of every graph, each as (file name, text), in emission order.
    fn emit_files<const SHIFT: u32>(
        frontend: &TypeScript<SHIFT>,
        graphs: &[CompiledGraph],
    ) -> Vec<(String, Vec<u8>)> {
        let mut files = Vec::new();
        for graph in graphs {
            for module in frontend.modules(graph) {
                let mut bytes = Vec::new();
                frontend.emit_module(graph, module, &mut bytes).unwrap();
                files.push((frontend.file_name(graph, module), bytes));
            }
        }
        files
    }

    fn emitted_text<const SHIFT: u32>(frontend: &TypeScript<SHIFT>, source: &str) -> String {
        let graphs = fixture::parse(
            source,
            &graphite::parser::Schema {
                node_flags: &["looping"],
                ..Default::default()
            },
        )
        .unwrap();
        emit_files(frontend, &graphs)
            .into_iter()
            .map(|(_, bytes)| String::from_utf8(bytes).unwrap())
            .collect()
    }

    #[test]
    fn registry_lists_typescript_and_rust_with_no_unknown_fallback() {
        // TypeScript, Rust and PureScript: a pure target is one more entry in `TARGETS`.
        assert_eq!(registry::TARGETS.len(), 3);
        let target = registry::lookup("typescript").unwrap();
        assert_eq!(target.key, "typescript");
        assert!(registry::lookup("TypeScript").is_none());
        assert!(registry::lookup("").is_none());
        let mut direct = Vec::new();
        let mut registered = Vec::new();
        DEFAULT.emit_runtime(&mut direct).unwrap();
        target.frontend.emit_runtime(&mut registered).unwrap();
        assert_eq!(direct, registered);
        assert!(
            String::from_utf8(direct)
                .unwrap()
                .contains("export function start")
        );
    }

    #[test]
    fn a_graph_is_an_index_plus_chunks_with_stable_file_names() {
        let graphs =
            fixture::parse("graph door\n  node closed end\n", &Default::default()).unwrap();
        let graph = &graphs[0];
        assert_eq!(
            DEFAULT.modules(graph),
            vec![Module::Index, Module::Nodes(0)]
        );
        assert_eq!(DEFAULT.file_name(graph, Module::Index), "door.ts");
        assert_eq!(
            DEFAULT.file_name(graph, Module::Nodes(0)),
            "door.nodes.0.ts"
        );
        assert_eq!(
            DEFAULT.file_name(graph, Module::Choices(3)),
            "door.choices.3.ts"
        );
    }

    #[test]
    fn small_chunks_split_nodes_and_choices_across_modules() {
        let source = include_str!("fixtures/runtime/runtime.graphite");
        let graphs = fixture::parse(source, &Default::default()).unwrap();
        let wide = graphs
            .iter()
            .find(|graph| graph.strings.resolve(graph.name) == Some("wide"))
            .unwrap();
        let frontend = chunked::<2>();
        assert_eq!(
            frontend.modules(wide),
            vec![
                Module::Index,
                Module::Nodes(0),
                Module::Nodes(1),
                Module::Nodes(2),
                Module::Choices(0),
                Module::Choices(1),
            ]
        );
    }

    #[test]
    fn emission_is_deterministic_and_does_not_change_compiled_storage() {
        let graphs = fixture::parse(
            include_str!("fixtures/parser/walk.graphite"),
            &Default::default(),
        )
        .unwrap();
        let before: Vec<_> = graphs[0].strings.strings().collect();
        let first = emit_files(&DEFAULT, &graphs);
        let second = emit_files(&DEFAULT, &graphs);
        assert_eq!(first, second);
        assert_eq!(graphs[0].strings.strings().collect::<Vec<_>>(), before);
    }

    #[test]
    fn each_graph_index_names_only_itself_and_imports_its_chunks() {
        let graphs = fixture::parse(
            "graph 10\n  node a end\ngraph 2\n  node b end\n",
            &Default::default(),
        )
        .unwrap();
        let files = emit_files(&DEFAULT, &graphs);
        let index = |name: &str| {
            files
                .iter()
                .find(|(file, _)| file == name)
                .map(|(_, bytes)| String::from_utf8(bytes.clone()).unwrap())
                .unwrap()
        };
        let ten = index("10.ts");
        let two = index("2.ts");
        assert!(ten.contains(r#"import { defineGraph } from "./graphite";"#));
        assert!(ten.contains(r#"import { chunk as nodes0 } from "./10.nodes.0";"#));
        assert!(ten.contains("export const graph = defineGraph<Vocab>({\nname:\"10\","));
        assert!(!ten.contains("\"2\""));
        assert!(two.contains("export const graph = defineGraph<Vocab>({\nname:\"2\","));
        assert!(!two.contains("\"10\""));
    }

    #[test]
    fn typescript_propagates_a_sink_failure_after_partial_output() {
        struct Limited(usize);

        impl Write for Limited {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                if self.0 == 0 {
                    return Err(io::Error::new(io::ErrorKind::StorageFull, "full sink"));
                }
                let count = self.0.min(bytes.len());
                self.0 -= count;
                Ok(count)
            }

            fn flush(&mut self) -> io::Result<()> {
                panic!("front end must not flush")
            }
        }

        let graphs =
            fixture::parse("graph door\n  node closed end\n", &Default::default()).unwrap();
        let graph = &graphs[0];
        let full = {
            let mut bytes = Vec::new();
            DEFAULT
                .emit_module(graph, Module::Index, &mut bytes)
                .unwrap();
            bytes.len()
        };
        for capacity in [0, 12, full - 1] {
            let mut sink = Limited(capacity);
            let error = DEFAULT
                .emit_module(graph, Module::Index, &mut sink)
                .unwrap_err();
            assert_eq!(sink.0, 0);
            assert_eq!(error.kind(), io::ErrorKind::StorageFull);
            assert_eq!(error.to_string(), "full sink");
        }
    }

    /// Writes every fixture twice: default chunks under `<dir>/<label>/` and two-item chunks under
    /// `<dir>/chunked/<label>/`, so `bun test tests/typescript` covers both layouts.
    /// The runtime imports `RUNTIME_FILE` from each folder.
    #[test]
    fn export_modules_for_cross_language_checks() {
        let Some(directory) = std::env::var_os("GRAPHITE_CODEGEN_TEST_OUTPUT") else {
            return;
        };
        write_fixtures(std::path::Path::new(&directory));
    }

    /// Runs the runtime's Bun tests against the fixtures this file writes, so `cargo test` covers them
    /// without the manual export step.
    #[test]
    #[ignore = "needs the test tools: scripts/setup-tools.py"]
    fn the_runtime_passes_on_the_exported_modules() {
        let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        write_fixtures(&manifest.join("target/ts-fixtures"));
        bun::assert_bun_test_passes_with(manifest, "tests/typescript/runtime.test.ts", &[]);
    }

    /// Writes every fixture under `directory`, for the runtime's Bun tests to load.
    fn write_fixtures(directory: &std::path::Path) {
        let inputs = [
            ("door", include_str!("fixtures/parser/door.graphite")),
            ("walk", include_str!("fixtures/parser/walk.graphite")),
            (
                "interleaved",
                include_str!("fixtures/parser/interleaved.graphite"),
            ),
            ("scopes", include_str!("fixtures/parser/scopes.graphite")),
            (
                "domain_flags",
                include_str!("fixtures/parser/domain_flags.graphite"),
            ),
            ("empty", ""),
            (
                "names",
                "graph 10\n  node é_中-2 end\ngraph 2\n  node closed end\n",
            ),
            (
                "prototype",
                "graph __proto__\n  node constructor end\ngraph constructor\n  node closed end\n",
            ),
            (
                "escaping",
                "graph door\n  node closed end @anim:\"\\idle\u{0001}\u{2028}\u{2029}</script>\n",
            ),
            (
                "pairs",
                "graph walk\n  node closed end @anim:idle @sfx:idle @time.delay @anim:open @anim:idle\n  choose facing\n    when left -> side @flip:true @flip:true\n    else -> side @anim:idle\n",
            ),
            ("runtime", include_str!("fixtures/runtime/runtime.graphite")),
        ];
        for (root, frontend_shift) in [
            (directory.to_path_buf(), 12),
            (directory.join("chunked"), 2),
        ] {
            for (label, source) in inputs {
                let folder = root.join(label);
                std::fs::create_dir_all(&folder).unwrap();
                let mut runtime = std::io::BufWriter::new(
                    std::fs::File::create(folder.join(RUNTIME_FILE)).unwrap(),
                );
                if frontend_shift == 12 {
                    DEFAULT.emit_runtime(&mut runtime).unwrap();
                } else {
                    chunked::<2>().emit_runtime(&mut runtime).unwrap();
                }
                runtime.flush().unwrap();
                let graphs = if source.is_empty() {
                    Vec::new()
                } else {
                    fixture::parse(
                        source,
                        &graphite::parser::Schema {
                            node_flags: &["looping"],
                            ..Default::default()
                        },
                    )
                    .unwrap()
                };
                let files = if frontend_shift == 12 {
                    emit_files(&DEFAULT, &graphs)
                } else {
                    emit_files(&chunked::<2>(), &graphs)
                };
                for (name, bytes) in files {
                    std::fs::write(folder.join(name), bytes).unwrap();
                }
            }
        }
    }

    #[test]
    fn choice_arms_keep_their_own_annotations_and_null_fallback() {
        let text = emitted_text(&DEFAULT, include_str!("fixtures/parser/walk.graphite"));
        assert!(text.contains(
            r#"readonly ["facing"]: never | { readonly condition: "left"; readonly tag: "side"; readonly annotations: readonly [{ readonly key: "flip"; readonly value: "true" },] } | { readonly condition: "right"; readonly tag: "side"; readonly annotations: readonly [] } | { readonly condition: "up"; readonly tag: "up"; readonly annotations: readonly [] } | { readonly condition: null; readonly tag: "down"; readonly annotations: readonly [] };"#
        ));
    }

    #[test]
    fn annotation_alternatives_are_unique_pairs_in_first_appearance_order() {
        let text = emitted_text(
            &DEFAULT,
            "graph walk\n  node closed end @anim:idle @sfx:idle @time.delay @anim:open @anim:idle\n  choose facing\n    when left -> side @flip:true @flip:true\n    else -> side @anim:idle\n",
        );
        assert!(text.contains(
            r#"readonly annotations: never | { readonly key: "anim"; readonly value: "idle" } | { readonly key: "sfx"; readonly value: "idle" } | { readonly key: "time.delay"; readonly value: null } | { readonly key: "anim"; readonly value: "open" } | { readonly key: "flip"; readonly value: "true" };"#
        ));
        assert!(text.contains(
            r#"annotations: readonly [{ readonly key: "flip"; readonly value: "true" },{ readonly key: "flip"; readonly value: "true" },] } | { readonly condition: null; readonly tag: "side"; readonly annotations: readonly [{ readonly key: "anim"; readonly value: "idle" },] };"#
        ));
    }

    #[test]
    fn string_escaping_covers_quotes_backslashes_controls_and_separators() {
        let text = emitted_text(
            &DEFAULT,
            "graph door\n  node closed end @anim:\"\\idle\u{0001}\u{2028}\u{2029}</script>\n",
        );
        assert!(text.contains(r#""\"\\idle\u0001\u2028\u2029</script>""#));
    }
}

// Merged from `tests/typescript_durable_units.rs`.
mod typescript_durable_units {
    //! Unit tests for the durable runner's TypeScript helpers, run under Bun: the `debug --simulate`
    //! stubs and their output lines, and the graph checks the entry points share.
    #[allow(unused_imports)]
    use super::bun;
    #[allow(unused_imports)]
    use super::tools;

    use std::path::{Path, PathBuf};

    fn no_variables() -> Vec<(&'static str, PathBuf)> {
        Vec::new()
    }

    #[test]
    #[ignore = "needs the test tools: scripts/setup-tools.py"]
    fn simulate_stubs_and_their_lines_pass_their_bun_tests() {
        bun::assert_bun_test_passes_with(
            Path::new(env!("CARGO_MANIFEST_DIR")),
            "tests/typescript/simulate.test.ts",
            &no_variables(),
        );
    }

    #[test]
    #[ignore = "needs the test tools: scripts/setup-tools.py"]
    fn the_durable_results_file_pass_its_bun_tests() {
        bun::assert_bun_test_passes_with(
            Path::new(env!("CARGO_MANIFEST_DIR")),
            "tests/typescript/durable_store.test.ts",
            &no_variables(),
        );
    }

    #[test]
    #[ignore = "needs the test tools: scripts/setup-tools.py"]
    fn graph_checks_pass_their_bun_tests() {
        bun::assert_bun_test_passes_with(
            Path::new(env!("CARGO_MANIFEST_DIR")),
            "tests/typescript/graph_checks.test.ts",
            &no_variables(),
        );
    }
}

// Merged from `tests/typescript_entry_scripts.rs`.
mod typescript_entry_scripts {
    //! The command-line entry scripts (`durable/cli.ts`, `durable/simulate.ts`, `cli/debug.ts`) run
    //! in-process under Bun, so their code is measured. The graphs they read are compiled here, as
    //! production does, and passed to `tests/typescript/entry_scripts.test.ts` by environment variable.
    #[allow(unused_imports)]
    use super::bun;
    #[allow(unused_imports)]
    use super::cli;
    #[allow(unused_imports)]
    use super::tools;

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
    #[ignore = "needs the test tools: scripts/setup-tools.py"]
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
}

// Merged from `tests/typescript_types.rs`.
mod typescript_types {
    //! The generated TypeScript types must accept real names and reject typos. The bun tests only
    //! run the code, so this test runs the TypeScript compiler (`tsc`) over a host file.
    #[allow(unused_imports)]
    use super::tools;

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
    #[ignore = "needs the test tools: scripts/setup-tools.py"]
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
                panic!("cannot run tsc: {error}; run scripts/setup-tools.py or set GRAPHITE_TSC")
            });
        assert!(
            output.status.success(),
            "tsc rejected the generated types:\n{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
