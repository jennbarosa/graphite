//! Helpers for driving the `graphite` binary from integration tests.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Output;

pub fn manifest() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

pub fn walk_spec() -> PathBuf {
    manifest().join("tests/fixtures/runtime/walk_spec.graphite")
}

/// A fresh path under the test scratch directory, with any earlier run removed.
pub fn scratch(name: &str) -> PathBuf {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&path);
    path
}

/// A DSL file with `source` as its content, in the scratch directory.
pub fn dsl(name: &str, source: &str) -> PathBuf {
    let path = scratch(name);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, source).unwrap();
    path
}

pub fn graphite<I, S>(args: I) -> Output
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    std::process::Command::new(env!("CARGO_BIN_EXE_graphite"))
        .args(args)
        .output()
        .expect("graphite binary must run")
}

pub fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

pub fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// The keys of every registered front end that has a debugger, in registry order. The
/// TypeScript front end comes first, because it is the reference the others must match.
pub fn debugging_front_ends() -> Vec<&'static str> {
    graphite::frontend::registry::TARGETS
        .iter()
        .filter(|target| target.debugger.is_some())
        .map(|target| target.key)
        .collect()
}
