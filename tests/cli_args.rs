//! Argument handling shared by every command.

#[allow(dead_code)]
#[path = "support/cli.rs"]
mod cli;

use cli::{graphite, stderr, stdout, walk_spec};

#[test]
fn an_unknown_option_is_rejected() {
    let spec = walk_spec();
    let output = graphite(["check".as_ref(), spec.as_os_str(), "--verbose".as_ref()]);
    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("unknown option --verbose"));
}

#[test]
fn no_arguments_prints_usage() {
    let output = graphite(Vec::<&str>::new());
    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("usage:"));
}

#[test]
fn an_unknown_command_is_rejected() {
    let spec = walk_spec();
    let output = graphite(["frobnicate".as_ref(), spec.as_os_str()]);
    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("unknown command \"frobnicate\""));
}

#[test]
fn an_option_without_its_value_is_rejected() {
    let spec = walk_spec();
    let output = graphite(["check".as_ref(), spec.as_os_str(), "--allow".as_ref()]);
    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("--allow needs a value"));
}

#[test]
fn a_command_without_its_file_is_rejected() {
    let output = graphite(["check"]);
    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("check needs a file"));
}

#[test]
fn doctor_takes_no_arguments() {
    let output = graphite(["doctor", "extra"]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("doctor takes no arguments"));
}

#[test]
fn version_prints_the_package_version_for_both_flags() {
    let expected = format!("graphite {}\n", env!("CARGO_PKG_VERSION"));
    for flag in ["--version", "-V"] {
        let output = graphite([flag]);
        assert!(output.status.success());
        assert_eq!(stdout(&output), expected);
    }
}
