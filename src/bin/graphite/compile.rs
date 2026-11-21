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
    front_end: Option<&str>,
    allow: Option<&Path>,
    pure: bool,
) -> Result<(), String> {
    let allowlist = allow.map(Allowlist::load).transpose()?;
    // Reading first resolves the front end and explains a conflict before the build starts.
    let loaded = load::read(input, allowlist.as_ref(), front_end, pure)?;
    let out = match out {
        Some(out) => out.to_path_buf(),
        None => default_out(input)?,
    };
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
        front_end: loaded.front_end,
        node_flags,
        pure: loaded.pure,
        verify: Some(&verify),
    })?;
    for path in written {
        println!("{}", path.display());
    }
    Ok(())
}
