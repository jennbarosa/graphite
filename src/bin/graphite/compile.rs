//! `graphite compile`: runs the library's build pipeline. The allowlist is read here, because it is
//! a CLI file, and its flags and check are handed to the pipeline. Never overwrites.

use std::path::{Path, PathBuf};

use graphite::pipeline::build::{self, Request};

use crate::allow::Allowlist;
use crate::resolve::{self, Output};

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
    let select = |name: &str, directives: &graphite::preprocess::Directives<'_>| {
        resolve::resolve(name, front_end, pure, directives, Output::Written)
    };
    let written = build::run(Request {
        input,
        out,
        node_flags,
        select: &select,
        verify: Some(&verify),
    })?;
    for path in written {
        println!("{}", path.display());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::default_out;

    #[test]
    fn the_default_output_keeps_the_pure_name_and_the_infix() {
        assert_eq!(
            default_out(Path::new("examples/combat.pure.graphite")).unwrap(),
            Path::new("graphite-out").join("combat.pure")
        );
        assert_eq!(
            default_out(Path::new("door.ts.graphite")).unwrap(),
            Path::new("graphite-out").join("door.ts")
        );
    }
}
