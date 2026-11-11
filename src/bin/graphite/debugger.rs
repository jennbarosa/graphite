//! Runs a target's debugger script with a JavaScript runtime, on one generated module.

use std::ffi::OsString;
use std::path::Path;

use crate::js;

pub fn launch(script: &Path, module: &Path, commands: &[String]) -> Result<(), String> {
    let mut args = vec![module.as_os_str().to_owned()];
    args.extend(commands.iter().map(OsString::from));
    let status = js::launch(script, &args)?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("debugger exited with {status}"))
    }
}
