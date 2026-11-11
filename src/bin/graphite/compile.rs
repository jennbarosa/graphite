//! `graphite compile`: writes the runtime and one module set per graph. Never overwrites.

use std::path::Path;

use graphite::pipeline::write_modules;

use crate::load;

pub fn run(input: &Path, out: &Path, target: &str, allow: Option<&Path>) -> Result<(), String> {
    let target = load::target(target)?;
    let (_, graphs) = load::graphs(input, allow)?;
    let written =
        write_modules(target.frontend, &graphs, out).map_err(|error| error.to_string())?;
    for path in written {
        println!("{}", path.display());
    }
    Ok(())
}
