//! The `@timeout:MS` node annotation as the Rust stage module reads it: the same values the
//! TypeScript durable engine accepts, and the same refusal for the rest.

#[allow(dead_code)]
#[path = "support/parse.rs"]
mod fixture;

use fixture::parse;
use graphite::parser::Schema;
use graphite::stage::timeout_of;

/// The timeout of the one node in a graph whose only annotation is `@timeout` with `value`.
fn timeout_for(annotation: &str) -> Result<Option<u32>, String> {
    let source = format!("graph timed\n  node work {annotation}\n  node done end\n\n  edge work -> done\n");
    let graphs = parse(&source, &Schema::default()).unwrap();
    timeout_of(&graphs[0], 0)
}

#[test]
fn a_whole_number_of_milliseconds_is_the_limit() {
    assert_eq!(timeout_for("@timeout:20"), Ok(Some(20)));
    assert_eq!(timeout_for("@timeout:2147483647"), Ok(Some(2_147_483_647)));
}

#[test]
fn a_node_without_the_annotation_has_no_limit() {
    assert_eq!(timeout_for(""), Ok(None));
}

#[test]
fn a_value_that_is_not_a_whole_number_of_milliseconds_is_refused() {
    for value in ["soon", "1.5", "0", "007", "2147483648", "-5"] {
        let error = timeout_for(&format!("@timeout:{value}")).unwrap_err();
        assert_eq!(
            error,
            format!(
                "node \"work\" has timeout:{value}; a timeout is a whole number of milliseconds, such as @timeout:30000"
            ),
            "value {value}"
        );
    }
}

#[test]
fn a_timeout_with_no_value_is_refused() {
    let error = timeout_for("@timeout").unwrap_err();
    assert!(error.contains("has timeout:;"), "{error}");
}
