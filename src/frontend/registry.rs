use std::path::{Path, PathBuf};

use super::{Frontend, rust, typescript};

/// A runtime that a front end's `debug` and `run` need on the machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Runtime {
    /// A JavaScript runtime: Bun, or `npx tsx`, or `GRAPHITE_JS_RUNNER`.
    JavaScript,
}

/// How `graphite debug` runs a front end's debugger.
#[derive(Clone, Copy)]
pub enum Debugger {
    /// A bundled script, run on the JavaScript runtime. A path relative to the CLI's bundled
    /// scripts, such as `cli/debug.ts`.
    Script(&'static str),
    /// A program the front end builds for one graph. It gets the output directory and the graph's
    /// name, and returns the program's path.
    Program(fn(&Path, &str) -> Result<PathBuf, String>),
}

pub struct Target {
    pub key: &'static str,
    pub frontend: &'static (dyn Frontend + Sync),
    /// The debugger `graphite debug` runs for this target, if it has one.
    pub debugger: Option<Debugger>,
    /// The runtime that `debug` runs the debugger on, if the target has one.
    pub runtime: Option<Runtime>,
}

/// The only shared registration list. Each backend owns its registration.
pub static TARGETS: &[Target] = &[typescript::TARGET, rust::TARGET];

/// The front end used when `--frontend` is not given.
pub const DEFAULT_FRONT_END: &str = "typescript";

/// The front end whose runtime runs `graphite run`. The durable engine is written for it alone.
pub const DURABLE_FRONT_END: &str = "typescript";

pub fn lookup(key: &str) -> Option<&'static Target> {
    TARGETS.iter().find(|target| target.key == key)
}
