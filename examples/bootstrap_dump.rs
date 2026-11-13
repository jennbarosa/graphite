//! Regeneration tool: compiles one `.graphite` file and writes a Rust module that rebuilds the
//! compiled graph without a parser. The steps (read, parse, emit, write) are the stage graph in
//! `src/graphs/regenerate.graphite`, run by `graphite::stage`; the task bodies are below. It
//! writes the bundled rule files in `src/generated/`; see CONTRIBUTING.md (Bootstrap).
//!
//! Usage: `bootstrap_dump <input.graphite> <output.rs>`

use std::path::PathBuf;
use std::process::ExitCode;

use graphite::graph::{CompiledGraph, Span, index};
use graphite::interner::StringId;
use graphite::parser::Schema;
use graphite::stage;

const REGENERATE: &str = include_str!("../src/graphs/regenerate.graphite");

#[derive(Clone, Copy)]
enum Task {
    Read,
    Parse,
    Emit,
    Write,
}

fn task_named(name: &str) -> Option<Task> {
    Some(match name {
        "read" => Task::Read,
        "parse" => Task::Parse,
        "emit" => Task::Emit,
        "write" => Task::Write,
        _ => return None,
    })
}

/// What the stages share. `error` holds the message for the `failed` end.
struct Job {
    input: PathBuf,
    output: PathBuf,
    source: String,
    graph: Option<CompiledGraph>,
    text: String,
    error: Option<String>,
}

impl Job {
    fn run(&mut self, task: Task) -> &'static [&'static str] {
        let result = match task {
            Task::Read => self.read(),
            Task::Parse => self.parse(),
            Task::Emit => self.emit(),
            Task::Write => self.write(),
        };
        match result {
            Ok(()) => &["passed"],
            Err(message) => {
                self.error = Some(message);
                &["failed"]
            }
        }
    }

    fn read(&mut self) -> Result<(), String> {
        self.source = std::fs::read_to_string(&self.input)
            .map_err(|error| format!("{}: {error}", self.input.display()))?;
        Ok(())
    }

    fn parse(&mut self) -> Result<(), String> {
        let graphs =
            graphite::pipeline::compile(&self.source, &Schema::default()).map_err(|error| {
                format!(
                    "{}: {}",
                    self.input.display(),
                    error.diagnostic(&self.source)
                )
            })?;
        let [graph] = <[CompiledGraph; 1]>::try_from(graphs)
            .map_err(|_| format!("{}: expected exactly one graph", self.input.display()))?;
        self.graph = Some(graph);
        Ok(())
    }

    fn emit(&mut self) -> Result<(), String> {
        let graph = self.graph.as_ref().ok_or("emit ran before parse")?;
        self.text = emit(graph);
        Ok(())
    }

    fn write(&mut self) -> Result<(), String> {
        std::fs::write(&self.output, &self.text)
            .map_err(|error| format!("{}: {error}", self.output.display()))
    }
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let (Some(input), Some(output), None) = (args.next(), args.next(), args.next()) else {
        eprintln!("usage: bootstrap_dump <input.graphite> <output.rs>");
        return ExitCode::from(2);
    };
    let graph = match graphite::pipeline::compile(REGENERATE, &Schema::default()) {
        Ok(graphs) => graphs
            .into_iter()
            .next()
            .expect("regenerate.graphite holds a graph"),
        Err(error) => {
            eprintln!("regenerate.graphite: {}", error.diagnostic(REGENERATE));
            return ExitCode::FAILURE;
        }
    };
    let mut job = Job {
        input: input.into(),
        output: output.into(),
        source: String::new(),
        graph: None,
        text: String::new(),
        error: None,
    };
    match stage::drive(&graph, task_named, |task| job.run(task)) {
        Ok(end) if end == "done" => ExitCode::SUCCESS,
        Ok(_) => {
            eprintln!(
                "{}",
                job.error.unwrap_or_else(|| "regeneration failed".into())
            );
            ExitCode::FAILURE
        }
        Err(stall) => {
            eprintln!("{stall}");
            ExitCode::FAILURE
        }
    }
}

/// Rust source for `fn graph()`, which rebuilds `graph` field by field.
fn emit(graph: &CompiledGraph) -> String {
    let strings: Vec<&str> = graph.strings.strings().collect();
    let bytes: usize = strings.iter().map(|text| text.len()).sum();
    let quoted: Vec<String> = strings.iter().map(|text| format!("{text:?}")).collect();
    let list = |items: Vec<String>| items.join(", ");

    let nodes = list(
        graph
            .nodes
            .iter()
            .map(|node| {
                format!(
                    "Node {{ name: {}, looping: {}, end: {}, flags: {}, annotations: {}, outgoing: {}, sequence: {:?}, offset: {} }}",
                    id(node.name),
                    node.looping,
                    node.end,
                    span(&node.flags),
                    span(&node.annotations),
                    span(&node.outgoing),
                    node.sequence,
                    node.offset,
                )
            })
            .collect(),
    );
    let edges = list(
        graph
            .edges
            .iter()
            .map(|edge| {
                format!(
                    "Edge {{ source: {}, target: {}, condition: {:?}, annotations: {} }}",
                    edge.source,
                    edge.target,
                    edge.condition,
                    span(&edge.annotations)
                )
            })
            .collect(),
    );
    let choices = list(
        graph
            .choices
            .iter()
            .map(|choice| {
                format!(
                    "Choice {{ name: {}, arms: {} }}",
                    id(choice.name),
                    span(&choice.arms)
                )
            })
            .collect(),
    );
    let arms = list(
        graph
            .arms
            .iter()
            .map(|arm| {
                format!(
                    "Arm {{ condition: {:?}, tag: {}, annotations: {} }}",
                    arm.condition,
                    id(arm.tag),
                    span(&arm.annotations)
                )
            })
            .collect(),
    );
    let annotations = list(
        graph
            .annotations
            .iter()
            .map(|annotation| {
                let value = match annotation.value {
                    Some(value) => format!("Some({})", id(value)),
                    None => "None".to_string(),
                };
                format!(
                    "Annotation {{ key: {}, value: {value} }}",
                    id(annotation.key)
                )
            })
            .collect(),
    );
    let conditions = list(graph.conditions.iter().map(|&name| id(name)).collect());
    let flags = list(graph.flags.iter().map(|&name| id(name)).collect());
    let outgoing = list(graph.outgoing.iter().map(u32::to_string).collect());

    // One line per field, so a change to the compiled form shows up as a small diff.
    let lines = [
        "pub(crate) fn graph() -> CompiledGraph {".to_string(),
        format!("    const STRINGS: &[&str] = &[{}];", list(quoted)),
        "    CompiledGraph {".to_string(),
        format!("        name: {},", id(graph.name)),
        format!("        offset: {},", graph.offset),
        format!("        strings: interner(STRINGS, {bytes}),"),
        format!("        nodes: vec![{nodes}],"),
        format!("        edges: vec![{edges}],"),
        format!("        outgoing: vec![{outgoing}],"),
        format!("        conditions: vec![{conditions}],"),
        format!("        choices: vec![{choices}],"),
        format!("        arms: vec![{arms}],"),
        format!("        annotations: vec![{annotations}],"),
        format!("        flags: vec![{flags}],"),
        format!(
            "        graph_annotations: {}..{},",
            graph.graph_annotations.start, graph.graph_annotations.end
        ),
        "    }".to_string(),
        "}".to_string(),
    ];
    lines.join("\n") + "\n" + &task_source(graph)
}

/// The generated code's name for a string: its zero-based index in the string table.
fn id(name: StringId) -> String {
    format!("id({})", name.index())
}

fn span(span: &Span) -> String {
    format!("{}..{}", span.start, span.end)
}

/// The `Task` enum and `task` lookup for the graph's `@task:` names, in first-use order. The
/// library matches on `Task`, so a task that has no handler fails to compile.
fn task_source(graph: &CompiledGraph) -> String {
    let mut names: Vec<&str> = Vec::new();
    for node in &graph.nodes {
        let annotations = &graph.annotations[index(&node.annotations)];
        for annotation in annotations {
            if graph.text(annotation.key) != "task" {
                continue;
            }
            let value = annotation.value.map(|value| graph.text(value));
            let value = value.expect("a @task annotation names a task");
            if !names.contains(&value) {
                names.push(value);
            }
        }
    }
    let variants: Vec<String> = names.iter().map(|name| pascal(name)).collect();
    let mut out = String::new();
    out.push_str("\n#[derive(Clone, Copy, Debug, PartialEq, Eq)]\npub(crate) enum Task {\n");
    for variant in &variants {
        out.push_str(&format!("    {variant},\n"));
    }
    out.push_str("}\n\npub(crate) fn task(name: &str) -> Option<Task> {\n    Some(match name {\n");
    for (name, variant) in names.iter().zip(&variants) {
        out.push_str(&format!("        {name:?} => Task::{variant},\n"));
    }
    out.push_str("        _ => return None,\n    })\n}\n");
    out
}

/// `check_pending` becomes `CheckPending`.
fn pascal(name: &str) -> String {
    name.split('_')
        .map(|part| {
            let mut letters = part.chars();
            match letters.next() {
                Some(first) => first.to_ascii_uppercase().to_string() + letters.as_str(),
                None => String::new(),
            }
        })
        .collect()
}
