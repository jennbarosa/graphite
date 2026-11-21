use std::path::{Path, PathBuf};

use super::pure::purescript;
use super::{Frontend, rust, typescript};
use crate::kind::Kind;

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
pub static TARGETS: &[Target] = &[typescript::TARGET, rust::TARGET, purescript::TARGET];

/// The front end used when `--frontend` is not given.
pub const DEFAULT_FRONT_END: &str = "typescript";

/// The front end used for a pure file when `--frontend` is not given.
pub const DEFAULT_PURE_FRONT_END: &str = "purescript";

/// The front end whose runtime runs `graphite run`. The durable engine is written for it alone.
pub const DURABLE_FRONT_END: &str = "typescript";

pub fn lookup(key: &str) -> Option<&'static Target> {
    TARGETS.iter().find(|target| target.key == key)
}

/// The target whose extension is `infix` and whose kind is `kind`, if one is registered. A new
/// target joins inference by being in `TARGETS`; there is no separate table of infixes.
pub fn for_infix(infix: &str, kind: Kind) -> Option<&'static Target> {
    TARGETS
        .iter()
        .find(|target| target.frontend.extension() == infix && target.frontend.kind() == kind)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::{TARGETS, for_infix};
    use crate::kind::Kind;

    #[test]
    fn no_two_targets_share_an_extension_and_a_kind() {
        let mut seen = HashSet::new();
        for target in TARGETS {
            let key = (target.frontend.extension(), target.frontend.kind());
            assert!(seen.insert(key), "two targets write {key:?}");
        }
    }

    #[test]
    fn an_infix_finds_the_machine_target_of_its_extension() {
        assert_eq!(
            for_infix("ts", Kind::Machine).map(|t| t.key),
            Some("typescript")
        );
        assert_eq!(for_infix("rs", Kind::Machine).map(|t| t.key), Some("rust"));
        assert!(for_infix("ts", Kind::Pure).is_none());
    }
}
