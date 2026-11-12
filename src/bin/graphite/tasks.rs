//! `graphite run --tasks`: reads a `tasks.toml` that maps each task to the command that runs it,
//! and writes the JSON table the durable host reads. Commands run in the directory that holds the
//! manifest, so relative paths in it are relative to the manifest.

use std::path::Path;

use crate::json;

/// The task names the manifest at `path` declares: its `[tasks]` keys. A run given this manifest
/// accepts only these tasks in its graphs.
pub fn names(path: &Path) -> Result<Vec<String>, String> {
    Ok(tasks_table(path)?.keys().cloned().collect())
}

/// The `[tasks]` table of the manifest at `path`.
fn tasks_table(path: &Path) -> Result<toml::Table, String> {
    let text =
        std::fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let manifest: toml::Table =
        toml::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))?;
    manifest
        .get("tasks")
        .and_then(toml::Value::as_table)
        .cloned()
        .ok_or_else(|| format!("{}: expected a [tasks] table of commands", path.display()))
}

/// The JSON table for the manifest at `path`: `{"cwd": ..., "commands": {task: [argv...]}}`.
pub fn table_json(path: &Path) -> Result<String, String> {
    let tasks = tasks_table(path)?;
    let mut commands = Vec::new();
    for (name, value) in &tasks {
        commands.push((name.as_str(), json::array(&argv_json(path, name, value)?)));
    }
    let manifest_path =
        std::fs::canonicalize(path).map_err(|error| format!("{}: {error}", path.display()))?;
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
fn argv_json(path: &Path, name: &str, value: &toml::Value) -> Result<Vec<String>, String> {
    let invalid = || {
        format!(
            "{}: task {name} must be a non-empty list of strings, such as [\"npm\", \"run\", \"test\"]",
            path.display()
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
