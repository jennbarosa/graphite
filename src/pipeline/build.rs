//! The build pipeline as a stage graph. `build.graphite` holds the stages and their order: read the
//! source, parse it, check the output names, and write the modules. Each task here does one stage.

use std::path::{Path, PathBuf};

use crate::frontend::registry::Target;
use crate::graph::CompiledGraph;
use crate::parser::Schema;
use crate::pipeline::{Collision, check_outputs, compile, write_modules};
use crate::stage;

const BUILD: &str = include_str!("../graphs/build.graphite");

/// A check on the parsed graphs. An `Err` fails the build with its message.
pub type Verify<'a> = dyn Fn(&[CompiledGraph]) -> Result<(), String> + 'a;

/// What one build needs. `verify` runs on the parsed graphs before the output check.
pub struct Request<'a> {
    pub input: &'a Path,
    pub out: PathBuf,
    pub front_end: &'static Target,
    pub node_flags: Vec<String>,
    pub verify: Option<&'a Verify<'a>>,
}

/// Runs the build and returns the files it wrote. Refuses to overwrite anything.
pub fn run(request: Request<'_>) -> Result<Vec<PathBuf>, String> {
    let mut job = Job {
        request,
        source: String::new(),
        graphs: Vec::new(),
        written: Vec::new(),
        error: None,
    };
    let graph = stage_graph();
    let end = stage::drive(&graph, StageTask::named, |task| job.run_task(task))?;
    if end == "failed" {
        return Err(job.error.unwrap_or_else(|| "build failed".to_string()));
    }
    Ok(job.written)
}

fn stage_graph() -> CompiledGraph {
    let mut graphs = compile(BUILD, &Schema::default())
        .unwrap_or_else(|error| panic!("the bundled build.graphite does not parse: {error:?}"));
    graphs.remove(0)
}

/// The tasks of `build.graphite`, one per task node.
#[derive(Clone, Copy, Debug)]
enum StageTask {
    Read,
    Parse,
    Check,
    Write,
}

impl StageTask {
    fn named(name: &str) -> Option<StageTask> {
        Some(match name {
            "read" => StageTask::Read,
            "parse" => StageTask::Parse,
            "check" => StageTask::Check,
            "write" => StageTask::Write,
            _ => return None,
        })
    }
}

struct Job<'a> {
    request: Request<'a>,
    source: String,
    graphs: Vec<CompiledGraph>,
    written: Vec<PathBuf>,
    /// The message to show when the build ends at the `failed` node.
    error: Option<String>,
}

impl Job<'_> {
    fn run_task(&mut self, task: StageTask) -> &'static [&'static str] {
        let result = match task {
            StageTask::Read => self.read(),
            StageTask::Parse => self.parse(),
            StageTask::Check => self.check(),
            StageTask::Write => self.write(),
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

    fn parse(&mut self) -> Result<(), String> {
        let flags: Vec<&str> = self.request.node_flags.iter().map(String::as_str).collect();
        let schema = Schema { node_flags: &flags };
        self.graphs = compile(&self.source, &schema)
            .map_err(|error| error.diagnostic(&self.source).to_string())?;
        if let Some(verify) = self.request.verify {
            verify(&self.graphs)?;
        }
        Ok(())
    }

    fn check(&mut self) -> Result<(), String> {
        check_outputs(self.request.front_end.frontend, &self.graphs)
            .map_err(|collision: Collision| self.hinted(&collision))
    }

    fn write(&mut self) -> Result<(), String> {
        let written = write_modules(
            self.request.front_end.frontend,
            &self.graphs,
            &self.request.out,
        )
        .map_err(|error| {
            match error
                .get_ref()
                .and_then(|inner| inner.downcast_ref::<Collision>())
            {
                Some(collision) => self.hinted(collision),
                None => error.to_string(),
            }
        })?;
        self.written = written;
        Ok(())
    }

    /// A collision's diagnostic, with the hint for the output directory.
    fn hinted(&self, collision: &Collision) -> String {
        format!(
            "{}\nhint: remove {} or pass another --out to compile again",
            collision.diagnostic(&self.source),
            self.request.out.display()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{StageTask, stage_graph};
    use crate::stage;

    fn visit(fail_at: Option<&str>) -> (String, Vec<String>) {
        let graph = stage_graph();
        let mut seen = Vec::new();
        let end = stage::drive(&graph, StageTask::named, |task| {
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
