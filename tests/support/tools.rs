//! Finds the external tools the tests run. `scripts/setup-tools.py` installs the pinned versions
//! into `tools/`. An environment variable overrides one tool's path, and otherwise the program is
//! looked up on `PATH`.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The path to `name`: the value of `variable` when it is set, else the copy in `tools/`, else
/// the bare name, which the operating system resolves on `PATH`.
pub fn tool(name: &str, variable: &str) -> PathBuf {
    if let Some(path) = std::env::var_os(variable) {
        return PathBuf::from(path);
    }
    let root = tools_root();
    for dir in [root.join("bin"), root.join("node_modules/.bin")] {
        let candidate = dir.join(name);
        if candidate.exists() {
            return candidate;
        }
    }
    PathBuf::from(name)
}

/// A command for `name` that finds the tools it runs: the `tools/` copies are first on its `PATH`.
pub fn command(name: &str, variable: &str) -> Command {
    let mut command = Command::new(tool(name, variable));
    command.env("PATH", path_with_tools());
    command
}

/// The `PATH` with `tools/bin` and `tools/node_modules/.bin` in front, for a child process that
/// runs a tool by its bare name.
pub fn path_with_tools() -> OsString {
    let root = tools_root();
    let mut dirs = vec![root.join("bin"), root.join("node_modules/.bin")];
    dirs.extend(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    ));
    std::env::join_paths(dirs).expect("PATH entries contain no separator")
}

fn tools_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tools")
}
