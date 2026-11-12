//! Every bundled example is a valid graph, so the examples in the docs stay correct.

#[allow(dead_code)]
#[path = "support/cli.rs"]
mod cli;

use cli::{graphite, manifest, stderr};

#[test]
fn every_example_graph_checks() {
    let examples = manifest().join("examples");
    let cases = [
        (examples.join("door.graphite"), None),
        (examples.join("walk.graphite"), None),
        (examples.join("order.graphite"), None),
        (
            examples.join("workflow/workflow.graphite"),
            Some(examples.join("workflow/workflow.toml")),
        ),
    ];
    for (dsl, allow) in cases {
        let mut args = vec!["check".as_ref(), dsl.as_os_str()];
        if let Some(allow) = &allow {
            args.extend(["--allow".as_ref(), allow.as_os_str()]);
        }
        let output = graphite(args);
        assert!(
            output.status.success(),
            "{}: {}",
            dsl.display(),
            stderr(&output)
        );
    }
}
