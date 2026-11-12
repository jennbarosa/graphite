//! The TypeScript durable host and debugger, compiled into the binary. They are written to a
//! private scratch directory before each run, so `graphite` works after `cargo install`, not only
//! from a checkout.

use std::path::{Path, PathBuf};

use crate::scratch;

/// Each file's path inside the bundle, and its contents. The bundle is the host's whole import
/// graph: every relative import in these files must name another entry here.
pub const FILES: &[(&str, &str)] = &[
    // Without `"type": "module"`, `tsx` would treat the host as CommonJS and reject its top-level
    // `await`. Bun reads ESM either way.
    ("package.json", "{\"type\": \"module\"}\n"),
    ("host/cli.ts", include_str!("../../../host/cli.ts")),
    (
        "host/commands.ts",
        include_str!("../../../host/commands.ts"),
    ),
    ("host/fanout.ts", include_str!("../../../host/fanout.ts")),
    ("host/durable.ts", include_str!("../../../host/durable.ts")),
    ("host/run-log.ts", include_str!("../../../host/run-log.ts")),
    ("cli/debug.ts", include_str!("../../../cli/debug.ts")),
    ("cli/format.ts", include_str!("../../../cli/format.ts")),
    ("cli/session.ts", include_str!("../../../cli/session.ts")),
    ("cli/types.ts", include_str!("../../../cli/types.ts")),
];

/// The durable host's entry point, relative to the bundle root.
pub const HOST_ENTRY: &str = "host/cli.ts";

/// Writes the bundle into a new private scratch directory and returns its root. The caller removes
/// the directory when the run is over.
pub fn materialize() -> Result<PathBuf, String> {
    let root = scratch::create("graphite-host")?;
    for (path, text) in FILES {
        let file = root.join(path);
        if let Some(parent) = file.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("{}: {error}", parent.display()))?;
        }
        std::fs::write(&file, text).map_err(|error| format!("{}: {error}", file.display()))?;
    }
    Ok(root)
}

/// Removes a directory made by [`materialize`]. Failure to clean up is not an error for the run.
pub fn discard(root: &Path) {
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(test)]
mod tests {
    use super::FILES;

    /// Every relative `import` or `import(...)` in the bundle names a file in the bundle.
    /// Dynamic imports of generated modules use absolute paths, so they are skipped.
    #[test]
    fn every_relative_import_is_in_the_bundle() {
        for (path, text) in FILES.iter().filter(|(path, _)| path.ends_with(".ts")) {
            for specifier in relative_specifiers(text) {
                let base = path.rsplit_once('/').map_or("", |(dir, _)| dir);
                let joined = format!("{base}/{specifier}");
                let resolved = normalize(&joined);
                let found = FILES
                    .iter()
                    .any(|(name, _)| *name == format!("{resolved}.ts") || *name == resolved);
                assert!(
                    found,
                    "{path} imports {specifier}, which is not in the bundle"
                );
            }
        }
    }

    fn relative_specifiers(text: &str) -> Vec<&str> {
        let mut found = Vec::new();
        for quote in ['\'', '"'] {
            let mut rest = text;
            while let Some(start) = rest.find(&format!("{quote}.")) {
                let after = &rest[start + 1..];
                if let Some(end) = after.find(quote) {
                    let specifier = &after[..end];
                    if specifier.starts_with("./") || specifier.starts_with("../") {
                        found.push(specifier);
                    }
                    rest = &after[end..];
                } else {
                    break;
                }
            }
        }
        found
    }

    fn normalize(path: &str) -> String {
        let mut parts: Vec<&str> = Vec::new();
        for part in path.split('/') {
            match part {
                "" | "." => {}
                ".." => {
                    parts.pop();
                }
                other => parts.push(other),
            }
        }
        parts.join("/")
    }
}
