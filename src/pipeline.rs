//! Source-to-files pipeline: DSL text goes through the lexer and parser into checked graphs,
//! then through a front end into files. The lexer runs inside `parse`, which streams its
//! tokens straight into the compiled arrays.

use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt;
use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};

pub mod build;

use crate::frontend::{Frontend, NameClash};
use crate::graph::CompiledGraph;
use crate::lexer::Diagnostic;
use crate::parser::{Capacity, GRAPH_KEYWORD, ParseError, Schema, parse_body};
use crate::preprocess::preprocess;

/// Storage bounds for each graph block in `source`, in source order.
///
/// A block starts at a `graph` line and runs to the next one. Every declaration in a block
/// occupies at least one line of that block, so its line count bounds the node, choice,
/// arm and condition counts. An edge line may chain several hops, so each `->` bounds one hop,
/// and an `else` adds at most one edge per hop: twice the arrow count bounds the edges.
/// Annotations, flags and strings are bounded by the block's bytes, and every node may also copy
/// each `default` annotation.
/// Text before the first graph gets no bounds, so `parse` rejects it. The directives at the top
/// of the file are not counted: the bounds are those of the body that [`compile`] parses.
pub fn block_capacities(source: &str, schema: &Schema<'_>) -> Vec<Capacity> {
    let directives = preprocess(source).ok();
    let body = directives.as_ref().map_or(0, |found| found.body);
    let pure = schema.pure || directives.is_some_and(|found| found.is_pure());
    capacities_of(&source[body..], &effective(schema, pure))
}

/// `schema` with its `pure` signal raised when the file's directives make it pure.
fn effective<'a>(schema: &Schema<'a>, pure: bool) -> Schema<'a> {
    Schema {
        node_flags: schema.node_flags,
        pure,
    }
}

/// [`block_capacities`] for text that has no directives.
fn capacities_of(source: &str, schema: &Schema<'_>) -> Vec<Capacity> {
    let mut blocks: Vec<Block> = Vec::new();
    let mut current: Option<Block> = None;
    for line in source.split_inclusive('\n') {
        if opens_graph(line) {
            blocks.extend(current.take());
            current = Some(Block::default());
        }
        if let Some(block) = current.as_mut() {
            block.lines += 1;
            block.bytes += line.len();
            block.arrows += line.matches("->").count();
            if line.trim_start().starts_with("default ") {
                block.defaults += line.matches('@').count();
            }
        }
    }
    blocks.extend(current);
    blocks
        .into_iter()
        .map(|block| Capacity {
            nodes: block.lines,
            edges: 2 * block.arrows,
            conditions: block.lines,
            choices: block.lines,
            arms: block.lines,
            // A declaration takes a line, and one line may hold several members.
            enums: if schema.pure { block.lines } else { 0 },
            members: if schema.pure {
                block.bytes.div_ceil(2)
            } else {
                0
            },
            inputs: if schema.pure { block.lines } else { 0 },
            annotations: block.bytes + block.lines * block.defaults,
            flags: if schema.node_flags.is_empty() {
                0
            } else {
                block.bytes
            },
            strings: block.bytes,
            string_bytes: block.bytes,
        })
        .collect()
}

/// What one graph block's text measures, for sizing its storage.
#[derive(Default)]
struct Block {
    lines: usize,
    bytes: usize,
    arrows: usize,
    /// Annotations on `default` lines, which every node may copy.
    defaults: usize,
}

/// Whether `line` opens a graph block: the keyword followed by a space.
fn opens_graph(line: &str) -> bool {
    line.strip_prefix(GRAPH_KEYWORD)
        .is_some_and(|rest| rest.starts_with(' '))
}

/// Reads the directives, then lexes and parses the rest of `source` into checked graphs. The
/// storage is sized by [`block_capacities`], which counts the same body.
pub fn compile(source: &str, schema: &Schema<'_>) -> Result<Vec<CompiledGraph>, ParseError> {
    let directives = preprocess(source)?;
    let schema = effective(schema, schema.pure || directives.is_pure());
    compile_body(source, directives.body, &schema)
}

/// [`compile`] for a caller that has already read the directives: `body` is their
/// [`Directives::body`](crate::preprocess::Directives), and `schema.pure` must already hold the
/// file's mode. Reading the directives again would repeat the work of the caller's read.
pub fn compile_body(
    source: &str,
    body: usize,
    schema: &Schema<'_>,
) -> Result<Vec<CompiledGraph>, ParseError> {
    parse_body(
        source,
        body,
        schema,
        &capacities_of(&source[body..], schema),
    )
}

/// Two graphs, or a graph and the runtime, would write the same output file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Collision {
    message: Cow<'static, str>,
    offset: usize,
    /// Whether the clash is with a file that already exists in the output directory. Only then
    /// does removing that directory help.
    existing: bool,
}

impl Collision {
    fn new(message: String, offset: usize) -> Self {
        Self {
            message: Cow::Owned(message),
            offset,
            existing: false,
        }
    }

    /// Whether removing the output directory would clear this collision.
    pub fn is_existing(&self) -> bool {
        self.existing
    }
}

impl Collision {
    /// Renders the collision with its source line and a caret under the graph's name.
    /// `source` must be the text the graphs were parsed from.
    pub fn diagnostic<'s>(&self, source: &'s str) -> Diagnostic<'s> {
        Diagnostic::new(self.message.clone(), self.offset, source)
    }
}

impl fmt::Display for Collision {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} at byte {}", self.message, self.offset)
    }
}

impl std::error::Error for Collision {}

/// Checks that the graphs can be written: no two outputs share a file name, none replaces the
/// runtime, and every front end accepts every graph (each graph is rendered into nowhere, so a
/// clash found while rendering is reported too).
///
/// Names are compared case-insensitively, so a graph set that builds on Linux also builds on
/// macOS and Windows. Returns the first collision in source order. Nothing is written.
pub fn check_outputs(frontend: &dyn Frontend, graphs: &[CompiledGraph]) -> Result<(), Collision> {
    check_plan(frontend, graphs, None)?;
    for graph in graphs {
        frontend
            .check_graph(graph)
            .map_err(|error| collision_at(graph, &error))?;
    }
    Ok(())
}

/// A front end's error for `graph`. A name clash carries the offset of the declaration that
/// clashes; any other error is placed at the graph's offset.
fn collision_at(graph: &CompiledGraph, error: &io::Error) -> Collision {
    let offset = name_clash(error).map_or(graph.offset, |clash| clash.offset);
    Collision::new(error.to_string(), offset)
}

/// The name clash inside a front end's error, if the error is one.
fn name_clash(error: &io::Error) -> Option<&NameClash> {
    error.get_ref()?.downcast_ref::<NameClash>()
}

fn is_name_clash(error: &io::Error) -> bool {
    name_clash(error).is_some()
}

/// [`check_outputs`], plus a refusal for any output that already exists in `directory`.
fn check_plan(
    frontend: &dyn Frontend,
    graphs: &[CompiledGraph],
    directory: Option<&Path>,
) -> Result<(), Collision> {
    let exists = |file: &str| directory.is_some_and(|directory| directory.join(file).exists());
    let runtime = frontend.runtime_file_name();
    // The runtime has no source of its own, so a conflict on it points at the first graph.
    let first = graphs.first().map_or(0, |graph| graph.offset);
    if let Some(runtime) = runtime.filter(|runtime| exists(runtime)) {
        let message =
            format!("the runtime file `{runtime}` already exists in the output directory");
        return Err(Collision {
            existing: true,
            ..Collision::new(message, first)
        });
    }
    // Lowercased output name -> owning graph, or `None` for the runtime.
    let mut owners: Owners = HashMap::new();
    if let Some(runtime) = runtime {
        owners.insert(runtime.to_lowercase(), None);
    }
    let runtime = runtime.unwrap_or_default();
    for (index, graph) in graphs.iter().enumerate() {
        let name = graph.text(graph.name);
        if graph.kind != frontend.kind() {
            return Err(Collision::new(
                format!(
                    "graph `{name}` is a {} graph, but this front end writes {} graphs",
                    graph.kind.label(),
                    frontend.kind().label()
                ),
                graph.offset,
            ));
        }
        for module in frontend.modules(graph) {
            let file = frontend.file_name(graph, module);
            let existing = exists(&file);
            let clash = if existing {
                Some(format!(
                    "graph `{name}` would overwrite the existing file `{file}`"
                ))
            } else {
                claim_output(&mut owners, graphs, index, &file, runtime).err()
            };
            if let Some(message) = clash {
                return Err(Collision {
                    existing,
                    ..Collision::new(message, graph.offset)
                });
            }
        }
    }
    Ok(())
}

/// Lowercased output name -> owning graph, or `None` for the runtime.
type Owners = HashMap<String, Option<usize>>;

/// Records that graph `index` writes `file`. Returns why it cannot: another graph or the runtime
/// already writes that name, ignoring case. Writing the same file twice for one graph is fine.
fn claim_output(
    owners: &mut Owners,
    graphs: &[CompiledGraph],
    index: usize,
    file: &str,
    runtime: &str,
) -> Result<(), String> {
    let name = graphs[index].text(graphs[index].name);
    let key = file.to_lowercase();
    match owners.get(&key) {
        None => {
            owners.insert(key, Some(index));
            Ok(())
        }
        Some(Some(owner)) if *owner == index => Ok(()),
        Some(None) => Err(format!(
            "graph `{name}` would overwrite the runtime file `{runtime}`"
        )),
        Some(Some(other)) => {
            let other = graphs[*other].text(graphs[*other].name);
            Err(format!(
                "graph `{name}` writes `{file}`, the same file as graph `{other}`"
            ))
        }
    }
}

/// Writes the runtime and every module of every graph into `directory`, and returns the paths
/// in the order written. Nothing is overwritten: if any output would replace an existing file,
/// or two outputs share a name, this fails before writing anything. The error wraps a
/// [`Collision`], which callers can recover with `get_ref` and `downcast_ref`. If a write fails
/// midway, the files this call created are removed, so a retry is not blocked by them.
pub fn write_modules(
    frontend: &dyn Frontend,
    graphs: &[CompiledGraph],
    directory: &Path,
) -> io::Result<Vec<PathBuf>> {
    check_plan(frontend, graphs, Some(directory))
        .map_err(|collision| io::Error::new(io::ErrorKind::AlreadyExists, collision))?;
    std::fs::create_dir_all(directory)?;
    let mut written = Vec::new();
    let result = write_each(frontend, graphs, directory, &mut written);
    if result.is_err() {
        for path in &written {
            let _ = std::fs::remove_file(path);
        }
    }
    result.map(|()| written)
}

fn write_each(
    frontend: &dyn Frontend,
    graphs: &[CompiledGraph],
    directory: &Path,
    written: &mut Vec<PathBuf>,
) -> io::Result<()> {
    if let Some(name) = frontend.runtime_file_name() {
        let runtime = directory.join(name);
        write_file(&runtime, |output| frontend.emit_runtime(output))?;
        written.push(runtime);
    }
    for graph in graphs {
        for module in frontend.modules(graph) {
            let path = directory.join(frontend.file_name(graph, module));
            // A name clash found while rendering is located at its declaration. Other errors pass
            // through unchanged.
            write_file(&path, |output| frontend.emit_module(graph, module, output)).map_err(
                |error| {
                    if is_name_clash(&error) {
                        io::Error::new(error.kind(), collision_at(graph, &error))
                    } else {
                        error
                    }
                },
            )?;
            written.push(path);
        }
    }
    Ok(())
}

/// Creates a new file. `create_new` makes the write fail, rather than replace, if the file
/// appeared after the check, so nothing is ever overwritten. If the emit fails, the partial
/// file this call created is removed; a file that was never created is never touched.
fn write_file(path: &Path, emit: impl FnOnce(&mut dyn Write) -> io::Result<()>) -> io::Result<()> {
    let mut output = BufWriter::new(File::create_new(path)?);
    let result = emit(&mut output).and_then(|()| output.flush());
    if result.is_err() {
        drop(output);
        let _ = std::fs::remove_file(path);
    }
    result
}
