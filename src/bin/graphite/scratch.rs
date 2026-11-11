//! Private scratch directories for generated modules that a JavaScript runtime then imports.

use std::io;
use std::path::PathBuf;

/// Creates a new directory in the system temp directory that only this user can open. An existing
/// path is skipped, never reused, so a directory another user made in a shared `/tmp` cannot be
/// used to swap the generated modules.
pub fn create(prefix: &str) -> Result<PathBuf, String> {
    let mut builder = std::fs::DirBuilder::new();
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    let temp = std::env::temp_dir();
    for attempt in 0..100u32 {
        let path = temp.join(format!("{prefix}-{}-{attempt}", std::process::id()));
        match builder.create(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(format!("{}: {error}", path.display())),
        }
    }
    Err(format!(
        "cannot create a scratch directory for {prefix} in {}",
        temp.display()
    ))
}

#[cfg(test)]
mod tests {
    use super::create;

    #[test]
    fn skips_an_existing_directory_and_creates_a_private_one() {
        let taken =
            std::env::temp_dir().join(format!("graphite-scratch-test-{}-0", std::process::id()));
        std::fs::create_dir_all(&taken).unwrap();
        let path = create("graphite-scratch-test").unwrap();
        assert_ne!(path, taken);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o700);
        }
        std::fs::remove_dir_all(&path).unwrap();
        std::fs::remove_dir_all(&taken).unwrap();
    }
}
