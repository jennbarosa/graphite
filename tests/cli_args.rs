//! Argument handling shared by every command.

#[allow(dead_code)]
#[path = "support/cli.rs"]
mod cli;

use cli::{graphite, stderr, walk_spec};

#[test]
fn an_unknown_option_is_rejected() {
    let spec = walk_spec();
    let output = graphite(["check".as_ref(), spec.as_os_str(), "--verbose".as_ref()]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("unknown option --verbose"));
}

#[test]
fn no_arguments_prints_usage() {
    let output = graphite(Vec::<&str>::new());
    assert!(!output.status.success());
    assert!(stderr(&output).contains("usage:"));
}
