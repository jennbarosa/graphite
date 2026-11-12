//! `graphite compile`: writes the runtime and one module set per graph. Never overwrites.

use std::path::{Path, PathBuf};

use graphite::pipeline::{Collision, write_modules};

use crate::load;

/// Where `compile` writes when no `--out` is given: `graphite-out/<file stem>`, beside the cwd.
pub fn default_out(input: &Path) -> Result<PathBuf, String> {
    let stem = input
        .file_stem()
        .ok_or_else(|| format!("{}: not a file name", input.display()))?;
    Ok(PathBuf::from("graphite-out").join(stem))
}

pub fn run(
    input: &Path,
    out: Option<&Path>,
    front_end: &str,
    allow: Option<&Path>,
) -> Result<(), String> {
    let front_end = load::front_end(front_end)?;
    let (source, graphs) = load::graphs(input, allow)?;
    let out = match out {
        Some(out) => out.to_path_buf(),
        None => default_out(input)?,
    };
    let written = write_modules(front_end.frontend, &graphs, &out).map_err(|error| match error
        .get_ref()
        .and_then(|inner| inner.downcast_ref::<Collision>())
    {
        Some(collision) => format!(
            "{}\nhint: remove {} or pass another --out to compile again",
            collision.diagnostic(&source),
            out.display()
        ),
        None => error.to_string(),
    })?;
    for path in written {
        println!("{}", path.display());
    }
    Ok(())
}
