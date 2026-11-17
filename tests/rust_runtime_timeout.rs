//! `Node::timeout` in the generated Rust runtime: the same `@timeout:MS` values the TypeScript
//! durable engine accepts, and the same refusal for the rest.

#[path = "../src/frontend/rust/runtime.rs"]
mod graphite;

use graphite::{Annotation, Node};

/// The timeout of a node named `work` whose annotations are `annotations`.
fn timeout_for(annotations: Vec<Annotation>) -> Result<Option<u32>, String> {
    Node {
        name: "work",
        offset: 0,
        looping: false,
        end: false,
        sequence: None,
        flags: &[],
        annotations: Vec::leak(annotations),
        edges: &[],
    }
    .timeout()
}

fn timeout_of(value: &'static str) -> Result<Option<u32>, String> {
    timeout_for(vec![Annotation {
        key: "timeout",
        value: Some(value),
    }])
}

#[test]
fn a_whole_number_of_milliseconds_is_the_limit() {
    assert_eq!(timeout_of("20"), Ok(Some(20)));
    assert_eq!(timeout_of("2147483647"), Ok(Some(2_147_483_647)));
}

#[test]
fn a_node_without_the_annotation_has_no_limit() {
    assert_eq!(timeout_for(vec![]), Ok(None));
    assert_eq!(
        timeout_for(vec![Annotation {
            key: "task",
            value: Some("write")
        }]),
        Ok(None)
    );
}

#[test]
fn a_value_that_is_not_a_whole_number_of_milliseconds_is_refused() {
    for value in ["soon", "1.5", "0", "007", "2147483648", "-5", "+5", ""] {
        assert_eq!(
            timeout_of(value).unwrap_err(),
            format!(
                "node \"work\" has timeout:{value}; a timeout is a whole number of milliseconds, such as @timeout:30000"
            ),
            "value {value}"
        );
    }
}

#[test]
fn a_timeout_with_no_value_is_refused() {
    let error = timeout_for(vec![Annotation {
        key: "timeout",
        value: None,
    }])
    .unwrap_err();
    assert!(error.contains("has timeout:;"), "{error}");
}
