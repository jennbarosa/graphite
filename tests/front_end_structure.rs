//! Every front end must describe the same graph. Each bundled graph file is compiled with every
//! front end. A dumper, run through that front end's own runtime, prints the structure: graph
//! annotations, conditions, nodes with their flags, annotations, sequence and outgoing edges, and
//! choices with their arms. The TypeScript and Rust dumps must match line for line.

#[allow(dead_code)]
#[path = "support/cli.rs"]
mod cli;

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use cli::{graphite, manifest, scratch, stderr, stdout};

/// The shared runtime's file, which is not a graph module.
const TS_RUNTIME: &str = "graphite.ts";
const RS_RUNTIME: &str = "graphite.rs";
const TS_DUMPER: &str = "structure_dump.ts";
const RS_DUMPER: &str = "structure_dump.rs";
const RS_DUMPER_PROGRAM: &str = "structure_dump";

/// Every bundled graph file: the examples, the workflow examples, and the runtime fixtures.
fn graph_files() -> Vec<PathBuf> {
    let root = manifest();
    let mut files = Vec::new();
    for dir in ["examples", "examples/workflow", "tests/fixtures/runtime"] {
        for entry in fs::read_dir(root.join(dir)).expect("graph directory") {
            let path = entry.expect("directory entry").path();
            if path.extension().is_some_and(|ext| ext == "graphite") {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

/// The allowlist a bundled graph file compiles under, when it uses flags or task names that need one.
fn allowlist(file: &Path) -> Option<PathBuf> {
    let root = manifest();
    let stem = file.file_stem()?.to_str()?;
    if file.parent()? == root.join("examples/workflow") {
        return Some(root.join("examples/workflow/workflow.toml"));
    }
    let fixtures = root.join("tests/fixtures/runtime");
    match stem {
        "workflow" | "workflow_variant" => Some(fixtures.join("workflow.toml")),
        "gads_chain" | "gads_expanded" => Some(fixtures.join("gads_allow.toml")),
        "stage_order" => Some(fixtures.join("stage_order.toml")),
        "timeouts" | "timeouts_invalid" => Some(fixtures.join("timeouts.toml")),
        _ => None,
    }
}

/// Compiles `file` with `front_end` into `dir`, which must be empty.
fn compile(file: &Path, front_end: Option<&str>, dir: &Path) {
    fs::create_dir_all(dir).expect("output directory");
    let mut args: Vec<String> = vec!["compile".into()];
    if let Some(allow) = allowlist(file) {
        args.extend(["--allow".into(), allow.display().to_string()]);
    }
    if let Some(front_end) = front_end {
        args.extend(["--frontend".into(), front_end.into()]);
    }
    args.extend([
        file.display().to_string(),
        "--out".into(),
        dir.display().to_string(),
    ]);
    let output = graphite(args);
    assert!(
        output.status.success(),
        "compiling {} with {front_end:?} failed:\n{}",
        file.display(),
        stderr(&output)
    );
}

/// The graph modules in `dir`: the files named `<stem>.<extension>` other than the runtime. The
/// TypeScript index of a graph is `<stem>.ts`, and its chunks have a dot in the name, so they are
/// skipped.
fn modules(dir: &Path, extension: &str, runtime: &str) -> BTreeSet<String> {
    let mut stems = BTreeSet::new();
    for entry in fs::read_dir(dir).expect("output directory") {
        let name = entry.expect("directory entry").file_name();
        let name = name.to_str().expect("utf-8 name");
        if name == runtime {
            continue;
        }
        let Some(stem) = name.strip_suffix(&format!(".{extension}")) else {
            continue;
        };
        if !stem.contains('.') {
            stems.insert(stem.to_string());
        }
    }
    stems
}

/// The TypeScript dumper: loads each graph's index through the TypeScript runtime and prints it.
fn typescript_dumper(stems: &BTreeSet<String>) -> String {
    let mut source = String::new();
    for (index, stem) in stems.iter().enumerate() {
        source += &format!("import {{ graph as g{index} }} from \"./{stem}\";\n");
    }
    let graphs: Vec<String> = (0..stems.len()).map(|index| format!("g{index}")).collect();
    source += &format!("const graphs: any[] = [{}];\n", graphs.join(", "));
    source += r#"
const text = (value: string | null | undefined) => (value === null || value === undefined ? "null" : value);
const pairs = (list: { key: string; value?: string | null }[]) =>
  "[" + list.map((item) => `${item.key}=${text(item.value)}`).join(",") + "]";

function dump(g: any) {
  console.log(`graph ${g.name}`);
  console.log(`graph-annotations ${pairs(g.annotations())}`);
  g.conditionNames().forEach((name: string, index: number) => console.log(`condition ${index} ${name}`));
  for (let id = 0; id < g.data.nodeCount; id++) {
    const name = g.nodeName(id);
    const record = g.nodeRecord(id);
    const chunk = g.nodeChunk(id);
    const seq = record.sequence === null ? "none" : String(chunk.edges.get(record.sequence).id);
    console.log(
      `node ${id} ${name} loop=${g.nodeLooping(name)} end=${g.nodeEnd(name)} seq=${seq} ` +
        `flags=[${g.nodeFlags(name).join(",")}] annotations=${pairs(g.nodeAnnotations(name))}`
    );
    const [start, end] = record.outgoing;
    for (let index = start; index < end; index++) {
      const edge = chunk.edges.get(index);
      const info = g.edge(edge.id);
      console.log(
        `edge ${id} ${edge.id} ${info.target} when=${info.condition ?? "none"} annotations=${pairs(info.annotations)}`
      );
    }
  }
  for (let id = 0; id < g.data.choiceCount; id++) {
    const chunk = g.choiceChunk(id);
    const choice = chunk.choices[g.choiceIndex(id)];
    console.log(`choice ${id} ${g.text(choice.name)}`);
    const [start, end] = choice.arms;
    for (let index = start; index < end; index++) {
      const arm = g.armIn(chunk, index);
      console.log(`arm ${id} ${arm.tag} when=${arm.condition ?? "none"} annotations=${pairs(arm.annotations)}`);
    }
  }
}

graphs.forEach(dump);
"#;
    source
}

/// The Rust dumper: prints each graph's static data the same way as the TypeScript dumper.
fn rust_dumper(stems: &BTreeSet<String>) -> String {
    let mut source = String::from(
        r#"mod graphite;
"#,
    );
    for stem in stems {
        source += &format!("mod {stem};\n");
    }
    source += r#"
use graphite::{Annotation, Graph};

fn text(value: Option<&str>) -> String {
    value.unwrap_or("null").to_string()
}

fn pairs(list: &[Annotation]) -> String {
    let items: Vec<String> = list
        .iter()
        .map(|item| format!("{}={}", item.key, text(item.value)))
        .collect();
    format!("[{}]", items.join(","))
}

fn when(graph: &Graph, condition: Option<u32>) -> String {
    condition.map_or("none".to_string(), |id| graph.conditions[id as usize].to_string())
}

fn dump(graph: &Graph) {
    println!("graph {}", graph.name);
    println!("graph-annotations {}", pairs(graph.annotations));
    for (index, name) in graph.conditions.iter().enumerate() {
        println!("condition {index} {name}");
    }
    for (id, node) in graph.nodes.iter().enumerate() {
        let seq = node.sequence.map_or("none".to_string(), |edge| edge.to_string());
        println!(
            "node {id} {} loop={} end={} seq={seq} flags=[{}] annotations={}",
            node.name,
            node.looping,
            node.end,
            node.flags.join(","),
            pairs(node.annotations)
        );
        for edge in node.edges {
            let target = graph.nodes[edge.target.0 as usize].name;
            println!(
                "edge {id} {} {target} when={} annotations={}",
                edge.id,
                when(graph, edge.condition),
                pairs(edge.annotations)
            );
        }
    }
    for (id, choice) in graph.choices.iter().enumerate() {
        println!("choice {id} {}", choice.name);
        for arm in choice.arms {
            println!(
                "arm {id} {} when={} annotations={}",
                arm.tag,
                when(graph, arm.condition),
                pairs(arm.annotations)
            );
        }
    }
}

fn main() {
"#;
    for stem in stems {
        source += &format!("    dump(&{stem}::GRAPH);\n");
    }
    source += "}\n";
    source
}

/// Runs the TypeScript dumper in `dir` with the bundled Bun, and returns what it printed.
fn run_typescript(dir: &Path, stems: &BTreeSet<String>) -> String {
    fs::write(dir.join(TS_DUMPER), typescript_dumper(stems)).expect("write TypeScript dumper");
    let output = Command::new("bun")
        .arg(TS_DUMPER)
        .current_dir(dir)
        .output()
        .expect("bun must run; install Bun to run this test");
    assert!(
        output.status.success(),
        "the TypeScript dumper failed in {}:\n{}",
        dir.display(),
        stderr(&output)
    );
    stdout(&output)
}

/// Compiles the Rust dumper against the emitted modules in `dir`, runs it, and returns its output.
fn run_rust(dir: &Path, stems: &BTreeSet<String>) -> String {
    fs::write(dir.join(RS_DUMPER), rust_dumper(stems)).expect("write Rust dumper");
    let program = dir.join(format!(
        "{RS_DUMPER_PROGRAM}{}",
        std::env::consts::EXE_SUFFIX
    ));
    let build = Command::new("rustc")
        .args(["--edition", "2021", "-A", "warnings"])
        .arg(dir.join(RS_DUMPER))
        .arg("-o")
        .arg(&program)
        .output()
        .expect("rustc must run");
    assert!(
        build.status.success(),
        "the Rust dumper did not build in {}:\n{}",
        dir.display(),
        stderr(&build)
    );
    let output = Command::new(&program).output().expect("dumper must run");
    assert!(
        output.status.success(),
        "the Rust dumper failed:\n{}",
        stderr(&output)
    );
    stdout(&output)
}

/// A readable description of where two dumps first differ.
fn first_difference(left: &str, right: &str) -> String {
    let left_lines: Vec<&str> = left.lines().collect();
    let right_lines: Vec<&str> = right.lines().collect();
    for index in 0..left_lines.len().max(right_lines.len()) {
        let (a, b) = (left_lines.get(index), right_lines.get(index));
        if a != b {
            return format!(
                "line {}:\n  typescript: {}\n  rust:       {}",
                index + 1,
                a.unwrap_or(&"(missing)"),
                b.unwrap_or(&"(missing)")
            );
        }
    }
    "no difference".to_string()
}

#[test]
fn every_front_end_describes_each_graph_the_same() {
    let files = graph_files();
    assert!(
        files.len() >= 5,
        "expected the bundled graph files, found {}",
        files.len()
    );
    let mut checked = 0;
    for file in &files {
        let name = file.file_stem().unwrap().to_string_lossy().into_owned();
        // A pure file compiles only to PureScript, so no machine front end has it to compare.
        if name.ends_with(".purs") {
            continue;
        }
        let ts_dir = scratch(&format!("structure-ts-{name}"));
        let rs_dir = scratch(&format!("structure-rs-{name}"));
        compile(file, None, &ts_dir);
        compile(file, Some("rust"), &rs_dir);

        let ts_stems = modules(&ts_dir, "ts", TS_RUNTIME);
        let rs_stems = modules(&rs_dir, "rs", RS_RUNTIME);
        assert_eq!(
            ts_stems,
            rs_stems,
            "{} produces different graphs in each front end",
            file.display()
        );
        assert!(
            !ts_stems.is_empty(),
            "{} produced no graphs",
            file.display()
        );

        let typescript = run_typescript(&ts_dir, &ts_stems);
        let rust = run_rust(&rs_dir, &rs_stems);
        assert!(
            typescript == rust,
            "{} is described differently by the two front ends; {}",
            file.display(),
            first_difference(&typescript, &rust)
        );
        checked += ts_stems.len();
    }
    assert!(
        checked >= 10,
        "expected at least 10 graphs to be checked, got {checked}"
    );
}
