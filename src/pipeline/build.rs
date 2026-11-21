//! The build pipeline as a stage graph. `build.graphite` holds the stages and their order: read the
//! source, parse it, check the output names, and write the modules. Each task here does one stage.

use std::borrow::Cow;
use std::path::{Path, PathBuf};

use crate::bootstrap;
use crate::bootstrap::build::{Task, task};
use crate::frontend::registry::Target;
use crate::graph::CompiledGraph;
use crate::kind::is_machine_line_refusal;
use crate::lexer::Diagnostic;
use crate::parser::{ParseError, Schema};
use crate::pipeline::{Collision, check_outputs, compile_body, write_modules};
use crate::preprocess::{Directives, preprocess};
use crate::stage;

/// A check on the parsed graphs. An `Err` fails the build with its message.
pub type Verify<'a> = dyn Fn(&[CompiledGraph]) -> Result<(), String> + 'a;

/// Chooses the front end and mode of a file, from its name and its directives. An `Err` fails the
/// build with its message.
pub type Select<'a> = dyn Fn(&str, &Directives<'_>) -> Result<Chosen, String> + 'a;

/// The front end a file builds with, and whether the file is pure. `because` says what made it
/// pure, for a diagnostic.
pub struct Chosen {
    pub target: &'static Target,
    pub pure: bool,
    pub because: Option<String>,
}

/// Reads a file and compiles it: the directives are read, `select` chooses the front end and mode
/// from the file's name and those directives, and the graphs are compiled in that mode. This is the
/// one way a file is read, so the build and the CLI's read-only commands agree on it.
pub fn compile_file(
    source: &str,
    input: &Path,
    node_flags: &[&str],
    select: &Select<'_>,
) -> Result<(Chosen, Vec<CompiledGraph>), String> {
    let directives = preprocess(source).map_err(|error| error.diagnostic(source).to_string())?;
    let chosen = select(&file_name(input), &directives)?;
    let schema = Schema {
        node_flags,
        pure: chosen.pure,
    };
    let graphs = compile_body(source, directives.body, &schema)
        .map_err(|error| explain_pure(error, source, chosen.because.as_deref()))?;
    Ok((chosen, graphs))
}

/// The last component of `path`, or the empty string for a path with none.
pub fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// What one build needs. `select` chooses the front end (see [`compile_file`]), and `verify` runs
/// on the parsed graphs before the output check.
pub struct Request<'a> {
    pub input: &'a Path,
    pub out: PathBuf,
    pub node_flags: Vec<String>,
    pub select: &'a Select<'a>,
    pub verify: Option<&'a Verify<'a>>,
}

/// Runs the build and returns the files it wrote. Refuses to overwrite anything.
pub fn run(request: Request<'_>) -> Result<Vec<PathBuf>, String> {
    let mut job = Job {
        request,
        source: String::new(),
        chosen: None,
        graphs: Vec::new(),
        written: Vec::new(),
        error: None,
    };
    let graph = bootstrap::build::graph();
    let end = stage::drive(&graph, task, |task| job.run_task(task))?;
    if end == "failed" {
        return Err(job.error.unwrap_or_else(|| "build failed".to_string()));
    }
    Ok(job.written)
}

struct Job<'a> {
    request: Request<'a>,
    source: String,
    /// Set by the parse, which runs before every stage that needs the front end.
    chosen: Option<Chosen>,
    graphs: Vec<CompiledGraph>,
    written: Vec<PathBuf>,
    /// The message to show when the build ends at the `failed` node.
    error: Option<String>,
}

impl Job<'_> {
    fn run_task(&mut self, task: Task) -> &'static [&'static str] {
        let result = match task {
            Task::Read => self.read(),
            Task::Parse => self.parse(),
            Task::Check => self.check(),
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
        self.source = std::fs::read_to_string(self.request.input)
            .map_err(|error| format!("{}: {error}", self.request.input.display()))?;
        Ok(())
    }

    /// The front end the parse chose. The graph runs `parse` before every stage that needs it.
    fn chosen(&self) -> Result<&Chosen, String> {
        self.chosen
            .as_ref()
            .ok_or_else(|| "internal error: the build checked or wrote before it parsed".to_owned())
    }

    fn parse(&mut self) -> Result<(), String> {
        let flags: Vec<&str> = self.request.node_flags.iter().map(String::as_str).collect();
        let (chosen, graphs) = compile_file(
            &self.source,
            self.request.input,
            &flags,
            self.request.select,
        )?;
        if let Some(verify) = self.request.verify {
            verify(&graphs)?;
        }
        self.chosen = Some(chosen);
        self.graphs = graphs;
        Ok(())
    }

    fn check(&mut self) -> Result<(), String> {
        let frontend = self.chosen()?.target.frontend;
        check_outputs(frontend, &self.graphs)
            .map_err(|collision: Collision| self.hinted(&collision))
    }

    fn write(&mut self) -> Result<(), String> {
        let frontend = self.chosen()?.target.frontend;
        let written = write_modules(frontend, &self.graphs, &self.request.out).map_err(
            |error| match error
                .get_ref()
                .and_then(|inner| inner.downcast_ref::<Collision>())
            {
                Some(collision) => self.hinted(collision),
                None => error.to_string(),
            },
        )?;
        self.written = written;
        Ok(())
    }

    /// A collision's diagnostic. Only a clash with an existing file gets the hint to remove the
    /// output directory, since removing it is the only fix for that kind.
    fn hinted(&self, collision: &Collision) -> String {
        let diagnostic = collision.diagnostic(&self.source).to_string();
        if collision.is_existing() {
            format!(
                "{diagnostic}\nhint: remove {} or pass another --out to compile again",
                self.request.out.display()
            )
        } else {
            diagnostic
        }
    }
}

/// The diagnostic for a parse error. A machine line in a pure file gets the reason the file is
/// pure, so the fix is clear.
fn explain_pure(error: ParseError, source: &str, because: Option<&str>) -> String {
    let machine_line = is_machine_line_refusal(error.message);
    match (because, machine_line) {
        (Some(because), true) => {
            let message = format!("{} (pure {because})", error.message);
            Diagnostic::new(Cow::Owned(message), error.offset, source).to_string()
        }
        _ => error.diagnostic(source).to_string(),
    }
}

#[cfg(test)]
mod tests {
    use crate::bootstrap;
    use crate::bootstrap::build::task;
    use crate::stage;

    fn visit(fail_at: Option<&str>) -> (String, Vec<String>) {
        let graph = bootstrap::build::graph();
        let mut seen = Vec::new();
        let end = stage::drive(&graph, task, |task| {
            let name = format!("{task:?}").to_lowercase();
            let outcome = if Some(name.as_str()) == fail_at {
                &["failed"]
            } else {
                &["passed"]
            };
            seen.push(name);
            outcome
        })
        .unwrap();
        (end, seen)
    }

    #[test]
    fn every_stage_runs_in_order_when_all_pass() {
        let (end, seen) = visit(None);
        assert_eq!(end, "done");
        assert_eq!(seen, ["read", "parse", "check", "write"]);
    }

    #[test]
    fn a_failing_stage_routes_to_failed_and_stops_there() {
        let (end, seen) = visit(Some("check"));
        assert_eq!(end, "failed");
        assert_eq!(seen, ["read", "parse", "check"]);
    }
}
