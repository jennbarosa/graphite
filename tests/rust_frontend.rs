use graphite::frontend::{Module, registry};

#[path = "support/parse.rs"]
mod fixture;

fn emit(source: &str) -> String {
    let graphs = fixture::parse(source, &Default::default()).unwrap();
    let mut bytes = Vec::new();
    registry::lookup("rust")
        .unwrap()
        .frontend
        .emit_module(&graphs[0], Module::Index, &mut bytes)
        .unwrap();
    String::from_utf8(bytes).unwrap()
}

#[test]
fn door_emits_constants_edges_and_annotations() {
    let text = emit(
        "graph door\n  node closed @anim:idle\n  node open end\n  edge closed -> open when request\n",
    );
    assert!(text.contains("pub const CLOSED: super::NodeId = super::NodeId(0);"));
    assert!(text.contains("pub const OPEN: super::NodeId = super::NodeId(1);"));
    assert!(text.contains("pub const REQUEST: u32 = 0;"));
    assert!(text.contains(r#"Node { name: "closed", offset: "#));
    assert!(text.contains(r#"Annotation { key: "anim", value: Some("idle") }"#));
    assert!(text.contains("Edge { id: 0, target: NodeId(1), condition: Some(0)"));
}

#[test]
fn names_that_collide_after_rewrite_are_an_error() {
    let graphs = fixture::parse(
        "graph g\n  node a-b\n  node a_b end\n  edge a-b -> a_b\n",
        &Default::default(),
    )
    .unwrap();
    let mut bytes = Vec::new();
    let error = registry::lookup("rust")
        .unwrap()
        .frontend
        .emit_module(&graphs[0], Module::Index, &mut bytes)
        .unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
}

#[test]
fn runtime_is_the_graphite_module_and_is_registered() {
    let target = registry::lookup("rust").expect("rust is a registered front end");
    assert_eq!(target.frontend.runtime_file_name(), "graphite.rs");
    let mut runtime = Vec::new();
    target.frontend.emit_runtime(&mut runtime).unwrap();
    assert!(
        String::from_utf8(runtime)
            .unwrap()
            .contains("pub struct Graph")
    );
}

#[test]
fn a_graph_name_that_is_not_a_rust_module_is_refused() {
    for name in ["my-graph", "1st", "fn"] {
        let graphs = fixture::parse(
            &format!("graph {name}\n  node a end\n"),
            &Default::default(),
        )
        .unwrap();
        let mut bytes = Vec::new();
        let error = registry::lookup("rust")
            .unwrap()
            .frontend
            .emit_module(&graphs[0], Module::Index, &mut bytes)
            .unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData, "{name}");
    }
}
