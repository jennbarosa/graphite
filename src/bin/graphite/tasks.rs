//! `graphite run --tasks`: reads a `tasks.toml` that maps each task to the command that runs it,
//! and writes the JSON table the durable host reads. Commands run in the directory that holds the
//! manifest, so relative paths in it are relative to the manifest.

use std::path::{Path, PathBuf};

use crate::json;

/// A manifest read once: its `[tasks]` table, and where it is.
pub struct Manifest {
    path: PathBuf,
    tasks: toml::Table,
}

impl Manifest {
    /// Reads and parses the manifest at `path`.
    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        let manifest: toml::Table =
            toml::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))?;
        let tasks = manifest
            .get("tasks")
            .and_then(toml::Value::as_table)
            .cloned()
            .ok_or_else(|| format!("{}: expected a [tasks] table of commands", path.display()))?;
        Ok(Self {
            path: path.to_path_buf(),
            tasks,
        })
    }

    /// The task names the manifest declares: its `[tasks]` keys. A run given this manifest accepts
    /// only these tasks in its graphs.
    pub fn names(&self) -> Vec<String> {
        self.tasks.keys().cloned().collect()
    }

    /// The JSON table for the host: `{"source", "cwd", "commands": {task: [argv...]}}`.
    pub fn table_json(&self) -> Result<String, String> {
        let mut commands = Vec::with_capacity(self.tasks.len());
        for (name, value) in &self.tasks {
            commands.push((name.as_str(), json::array(&self.argv_json(name, value)?)));
        }
        let manifest_path = std::fs::canonicalize(&self.path)
            .map_err(|error| format!("{}: {error}", self.path.display()))?;
        let directory = manifest_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default();
        Ok(json::object(&[
            ("source", json::quote(&manifest_path.to_string_lossy())),
            ("cwd", json::quote(&directory.to_string_lossy())),
            ("commands", json::object(&commands)),
        ]))
    }

    /// One task's argv, as JSON strings. It must be a non-empty list of strings.
    fn argv_json(&self, name: &str, value: &toml::Value) -> Result<Vec<String>, String> {
        let invalid = || {
            format!(
                "{}: task {name} must be a non-empty list of strings, such as [\"npm\", \"run\", \"test\"]",
                self.path.display()
            )
        };
        let items = value.as_array().ok_or_else(invalid)?;
        if items.is_empty() {
            return Err(invalid());
        }
        items
            .iter()
            .map(|item| item.as_str().map(json::quote).ok_or_else(invalid))
            .collect()
    }
}
