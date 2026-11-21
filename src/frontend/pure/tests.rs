//! A test-only pure language. It proves the recipe: one `Language`, one `PureSyntax`, and no edit
//! anywhere else. Its members live in an `Owner` namespace, so one member may appear in two enums,
//! which the shared check's other branch must allow.

use std::io::{self, Write};

use super::decision::Decisions;
use super::{Pure, PureSyntax};
use crate::frontend::Frontend;
use crate::frontend::escape::Escape;
use crate::frontend::lang::{Language, Namespace, Role, camel, pascal};
use crate::kind::Kind;
use crate::parser::Schema;
use crate::pipeline::compile;

struct Toy;

impl Language for Toy {
    fn extension(&self) -> &'static str {
        "toy"
    }

    fn escape(&self, ch: char) -> Escape {
        match ch {
            '"' => Escape::Text("\\\""),
            '\\' => Escape::Text("\\\\"),
            _ => Escape::Keep,
        }
    }

    fn none(&self) -> &'static str {
        "none"
    }

    fn some(&self, inner: &str) -> String {
        format!("(some {inner})")
    }

    fn keywords(&self) -> &'static [&'static str] {
        &["end"]
    }

    fn spell(&self, role: Role, name: &str) -> String {
        match role {
            Role::Module | Role::Type | Role::Member => pascal(name),
            Role::Field | Role::Function | Role::Constant => camel(name),
        }
    }

    fn escape_keyword(&self, spelled: String) -> String {
        format!("{spelled}_")
    }

    fn namespace(&self, role: Role) -> Namespace {
        match role {
            Role::Module => Namespace::Module("module"),
            Role::Type => Namespace::Module("type"),
            Role::Member | Role::Field => Namespace::Owner,
            Role::Function | Role::Constant => Namespace::Module("value"),
        }
    }
}

static TOY: Toy = Toy;

struct ToySyntax;

impl PureSyntax for ToySyntax {
    fn language(&self) -> &'static dyn Language {
        &TOY
    }

    fn generated(&self, decisions: &Decisions) -> Vec<(Role, String)> {
        decisions
            .functions
            .iter()
            .map(|function| (Role::Function, format!("{}Tag", function.name)))
            .collect()
    }

    fn module(&self, decisions: &Decisions, output: &mut dyn Write) -> io::Result<()> {
        let language = self.language();
        for function in &decisions.functions {
            let name = language.ident(Role::Function, function.name);
            writeln!(output, "{name} arms={}", function.cases.len())?;
        }
        Ok(())
    }
}

fn toy() -> Pure<ToySyntax> {
    Pure::new(ToySyntax)
}

fn graph(source: &str) -> Vec<crate::graph::CompiledGraph> {
    compile(
        source,
        &Schema {
            pure: true,
            ..Schema::default()
        },
    )
    .unwrap()
}

fn emitted(source: &str) -> io::Result<String> {
    let graphs = graph(source);
    let mut out = Vec::new();
    toy().emit_module(&graphs[0], crate::frontend::Module::Index, &mut out)?;
    Ok(String::from_utf8(out).unwrap())
}

#[test]
fn the_toy_language_registers_through_the_recipe_alone() {
    let front = toy();
    assert_eq!(front.extension(), "toy");
    assert_eq!(front.kind(), Kind::Pure);
    assert_eq!(front.runtime_file_name_opt(), None);
    let graphs = graph(
        "graph door\n  enum E a\n  input x : bool\n  choose c : E\n    when x -> a\n    else -> a\n",
    );
    assert_eq!(
        front.file_name(&graphs[0], crate::frontend::Module::Index),
        "Door.toy"
    );
}

#[test]
fn a_module_is_written_from_the_decisions() {
    let text = emitted(
        "graph door\n  enum E a b\n  input x : bool\n  choose tactic : E\n    when x -> a\n    else -> b\n",
    )
    .unwrap();
    assert_eq!(text, "tactic arms=2\n");
}

#[test]
fn one_member_may_appear_in_two_enums() {
    let text = emitted(
        "graph g\n  enum A x y\n  enum B x z\n  input k : bool\n  choose c : A\n    when k -> x\n    else -> y\n",
    );
    assert!(text.is_ok(), "{text:?}");
}

#[test]
fn two_source_names_that_spell_the_same_identifier_are_refused() {
    let error = emitted("graph g\n  enum home-east a\n  enum home_east b\n").unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    assert_eq!(
        error.to_string(),
        "`home_east` and `home-east` both become `HomeEast`"
    );
}

#[test]
fn a_source_name_that_clashes_with_a_generated_name_is_refused() {
    let error = emitted(
        "graph g\n  enum E a\n  input k : bool\n  choose tactic : E\n    when k -> a\n    else -> a\n  choose tacticTag : E\n    when k -> a\n    else -> a\n",
    )
    .unwrap_err();
    assert_eq!(
        error.to_string(),
        "`tacticTag` clashes with the generated name `tacticTag`"
    );
}

#[test]
fn a_keyword_is_escaped_in_the_identifier() {
    let text = emitted(
        "graph g\n  enum E a\n  input k : bool\n  choose end : E\n    when k -> a\n    else -> a\n",
    )
    .unwrap();
    assert_eq!(text, "end_ arms=2\n");
}

#[test]
fn the_annotation_literal_is_written_once_for_every_language() {
    let mut out = Vec::new();
    TOY.annotation("ui", Some("icon"), &mut out).unwrap();
    assert_eq!(
        String::from_utf8(out).unwrap(),
        "{ key: \"ui\", value: (some \"icon\") }"
    );
    let mut out = Vec::new();
    TOY.annotation("flag", None, &mut out).unwrap();
    assert_eq!(
        String::from_utf8(out).unwrap(),
        "{ key: \"flag\", value: none }"
    );
}

#[test]
fn a_machine_graph_is_refused_by_a_pure_front_end_before_anything_is_written() {
    let graphs = compile("graph door\n  node a end\n", &Schema::default()).unwrap();
    let error = crate::pipeline::check_outputs(&toy(), &graphs).unwrap_err();
    assert_eq!(
        error.to_string(),
        "graph `door` is a machine graph, but this front end writes pure graphs at byte 6"
    );
}

#[test]
fn a_pure_graph_is_refused_by_a_machine_front_end() {
    let graphs = graph(
        "graph door\n  enum E a\n  input x : bool\n  choose c : E\n    when x -> a\n    else -> a\n",
    );
    let error = crate::pipeline::check_outputs(&crate::frontend::typescript::TYPESCRIPT, &graphs)
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "graph `door` is a pure graph, but this front end writes machine graphs at byte 6"
    );
}

#[test]
fn a_pure_front_end_writes_its_module_and_no_runtime() {
    let graphs = graph(
        "graph door\n  enum E a\n  input x : bool\n  choose c : E\n    when x -> a\n    else -> a\n",
    );
    let directory = std::env::temp_dir().join(format!("graphite-pure-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    let written = crate::pipeline::write_modules(&toy(), &graphs, &directory).unwrap();
    let names: Vec<String> = written
        .iter()
        .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    let _ = std::fs::remove_dir_all(&directory);
    assert_eq!(names, ["Door.toy"]);
}
