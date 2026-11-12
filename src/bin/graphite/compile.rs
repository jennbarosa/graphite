//! `graphite compile`: runs the library's build pipeline. The allowlist is read here, because it is
//! a CLI file, and its flags and check are handed to the pipeline. Never overwrites.

use std::path::{Path, PathBuf};

use graphite::pipeline::build::{self, Request};

use crate::allow::Allowlist;
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
    let out = match out {
        Some(out) => out.to_path_buf(),
        None => default_out(input)?,
    };
    let allowlist = allow.map(Allowlist::load).transpose()?;
    let node_flags = allowlist.as_ref().map_or(Vec::new(), |list| {
        list.node_flags().into_iter().map(str::to_string).collect()
    });
    let verify = |graphs: &[graphite::graph::CompiledGraph]| match &allowlist {
        Some(list) => list.check(graphs),
        None => Ok(()),
    };
    let written = build::run(Request {
        input,
        out,
        front_end,
        node_flags,
        verify: Some(&verify),
    })?;
    for path in written {
        println!("{}", path.display());
    }
    Ok(())
}
