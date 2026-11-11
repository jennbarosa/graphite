//! Runs a TypeScript test file with Bun from the crate root, pointing it at a generated output.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Asserts that `bun test <file>` passes, with `variable` set to `value` for the run.
pub fn assert_bun_test_passes(manifest: &Path, file: &str, variable: &str, value: &Path) {
    assert_bun_test_passes_with(manifest, file, &[(variable, value.to_path_buf())]);
}

/// Like [`assert_bun_test_passes`], with several environment variables set for the run.
pub fn assert_bun_test_passes_with(manifest: &Path, file: &str, variables: &[(&str, PathBuf)]) {
    let mut command = Command::new("bun");
    command
        .arg("test")
        .arg(manifest.join(file))
        .current_dir(manifest);
    for (name, value) in variables {
        command.env(name, value);
    }
    let output = command
        .output()
        .expect("bun must be installed to run the TypeScript tests");
    assert!(
        output.status.success(),
        "{file} failed:\n{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
