//! Source-to-files pipeline: DSL text goes through the lexer and parser into checked graphs,
//! then through a front end into files. The lexer runs inside `parse`, which streams its
//! tokens straight into the compiled arrays.

use std::collections::HashMap;
use std::fmt;
use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

pub mod build;

use crate::frontend::registry;
use crate::frontend::{Frontend, NameClash};
use crate::graph::CompiledGraph;
use crate::kind::Kind;
use crate::lexer::Diagnostic;
use crate::parser::{ParseError, Schema, parse_body};
use crate::preprocess::{Directives, preprocess};

/// Whether a file's `@!frontend` directive names a pure front end. A file that picks its front end
/// by name or by flag is read by the CLI, which passes the mode in `Schema::pure`.
fn names_pure_front_end(directives: &Directives<'_>) -> bool {
    directives
        .frontend
        .and_then(|(key, _)| registry::lookup(key))
        .is_some_and(|target| target.frontend.kind() == Kind::Pure)
}

/// `schema` with its `pure` signal raised when the file's directives make it pure.
fn effective<'a>(schema: &Schema<'a>, pure: bool) -> Schema<'a> {
    Schema {
        node_flags: schema.node_flags,
        pure,
    }
}

/// Reads the directives, then lexes and parses the rest of `source` into checked graphs.
pub fn compile(source: &str, schema: &Schema<'_>) -> Result<Vec<CompiledGraph>, ParseError> {
    let directives = preprocess(source)?;
    let schema = effective(schema, schema.pure || names_pure_front_end(&directives));
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
    parse_body(source, body, schema)
}

/// Two graphs, or a graph and the runtime, would write the same output file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Collision {
    message: String,
    offset: usize,
    /// Whether the clash is with a file that already exists in the output directory. Only then
    /// does removing that directory help.
    existing: bool,
}

impl Collision {
    fn new(message: String, offset: usize) -> Self {
        Self {
            message,
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
/// runtime, and every front end accepts every graph. Names are compared case-insensitively, so a
/// graph set that builds on Linux also builds on macOS and Windows. Returns the first collision in
/// source order. Nothing is written.
pub fn check_outputs(frontend: &dyn Frontend, graphs: &[CompiledGraph]) -> Result<(), Collision> {
    check_plan(frontend, graphs, None)?;
    for graph in graphs {
        frontend
            .check_graph(graph)
            .map_err(|error| collision_at(graph, &error))?;
    }
    Ok(())
}

/// Renders every output in memory, so a clash found while rendering is reported before anything is
/// written. The caller checks the names first. Returns each output's file name and text, in write order.
/// A clash comes back as an I/O error wrapping a [`Collision`].
fn render_outputs(
    frontend: &dyn Frontend,
    graphs: &[CompiledGraph],
) -> io::Result<Vec<(String, Vec<u8>)>> {
    let mut outputs = Vec::new();
    if let Some(name) = frontend.runtime_file_name() {
        let mut text = Vec::new();
        frontend.emit_runtime(&mut text)?;
        outputs.push((name.to_owned(), text));
    }
    for graph in graphs {
        for module in frontend.modules(graph) {
            let mut text = Vec::new();
            frontend
                .emit_module(graph, module, &mut text)
                .map_err(|error| {
                    if is_name_clash(&error) {
                        io::Error::new(error.kind(), collision_at(graph, &error))
                    } else {
                        error
                    }
                })?;
            outputs.push((frontend.file_name(graph, module), text));
        }
    }
    Ok(outputs)
}

/// The I/O error that carries a collision, so callers can recover it with `downcast_ref`.
fn collision_error(collision: Collision) -> io::Error {
    io::Error::new(io::ErrorKind::AlreadyExists, collision)
}

fn is_name_clash(error: &io::Error) -> bool {
    name_clash(error).is_some()
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
/// [`Collision`], which callers can recover with `get_ref` and `downcast_ref`. Everything is
/// rendered before the directory is created, so a clash found while rendering leaves no trace.
/// If a write fails midway, the files this call created are removed, so a retry is not blocked
/// by them.
pub fn write_modules(
    frontend: &dyn Frontend,
    graphs: &[CompiledGraph],
    directory: &Path,
) -> io::Result<Vec<PathBuf>> {
    check_plan(frontend, graphs, Some(directory)).map_err(collision_error)?;
    let outputs = render_outputs(frontend, graphs)?;
    std::fs::create_dir_all(directory)?;
    let mut written = Vec::new();
    for (name, text) in outputs {
        let path = directory.join(name);
        if let Err(error) = create_new(&path, &text) {
            for path in &written {
                let _ = std::fs::remove_file(path);
            }
            return Err(error);
        }
        written.push(path);
    }
    Ok(written)
}

/// Creates a new file holding `text`. `create_new` makes the write fail, rather than replace, if
/// the file appeared after the check, so nothing is ever overwritten.
fn create_new(path: &Path, text: &[u8]) -> io::Result<()> {
    let mut file = File::create_new(path)?;
    let result = file.write_all(text);
    if result.is_err() {
        drop(file);
        let _ = std::fs::remove_file(path);
    }
    result
}
