use super::{Frontend, typescript};

/// A host runtime that a front end's `debug` and `run` need on the machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Runtime {
    /// A JavaScript runtime: Bun, or `npx tsx`, or `GRAPHITE_JS_RUNNER`.
    JavaScript,
}

pub struct Target {
    pub key: &'static str,
    pub frontend: &'static (dyn Frontend + Sync),
    /// The debugger script `graphite debug` runs for this target, if it has one. A path relative
    /// to the CLI's bundled scripts, such as `cli/debug.ts`.
    pub debugger: Option<&'static str>,
    /// The runtime that `debug` runs the debugger on, if the target has one.
    pub runtime: Option<Runtime>,
}

/// The only shared registration list. Each backend owns its registration.
pub static TARGETS: &[Target] = &[typescript::TARGET];

/// The front end used when `--frontend` is not given.
pub const DEFAULT_FRONT_END: &str = "typescript";

/// The front end whose host runs `graphite run`. The durable host is written for it alone.
pub const DURABLE_FRONT_END: &str = "typescript";

pub fn lookup(key: &str) -> Option<&'static Target> {
    TARGETS.iter().find(|target| target.key == key)
}
