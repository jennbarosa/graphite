use super::{Frontend, typescript};

pub struct Target {
    pub key: &'static str,
    pub frontend: &'static (dyn Frontend + Sync),
    /// The debugger script `graphite debug` runs for this target, if it has one. A path relative
    /// to the CLI's bundled scripts, such as `cli/debug.ts`.
    pub debugger: Option<&'static str>,
}

/// The only shared registration list. Each backend owns its registration.
pub static TARGETS: &[Target] = &[typescript::TARGET];

pub fn lookup(key: &str) -> Option<&'static Target> {
    TARGETS.iter().find(|target| target.key == key)
}
