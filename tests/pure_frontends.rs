//! The PureScript front end, end to end through the library: the bundled example, written as the
//! CLI writes it (pure from its target), against a hand-written expected module.

use std::collections::HashSet;
use std::path::PathBuf;

use graphite::frontend::lang::purescript::PURESCRIPT;
use graphite::frontend::lang::{Language, Role};
use graphite::frontend::pure::PureSyntax;
use graphite::frontend::pure::decision::lower;
use graphite::frontend::pure::purescript::PureScriptSyntax;
use graphite::frontend::registry::{self, DEFAULT_PURE_FRONT_END, lookup};
use graphite::frontend::{Frontend, Module};
use graphite::graph::CompiledGraph;
use graphite::kind::Kind;
use graphite::parser::Schema;
use graphite::pipeline::compile;

fn manifest() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The graphs of `source`, parsed as pure, which is what the CLI does for a pure target.
fn pure_graphs(source: &str) -> Vec<CompiledGraph> {
    compile(
        source,
        &Schema {
            pure: true,
            ..Schema::default()
        },
    )
    .unwrap_or_else(|error| panic!("{}", error.diagnostic(source)))
}

fn purescript() -> &'static dyn Frontend {
    lookup("purescript")
        .expect("the PureScript target is registered")
        .frontend
}

fn emitted(frontend: &dyn Frontend, graph: &CompiledGraph) -> String {
    let mut out = Vec::new();
    frontend
        .emit_module(graph, Module::Index, &mut out)
        .unwrap_or_else(|error| panic!("{error}"));
    String::from_utf8(out).unwrap()
}

#[test]
fn the_combat_example_writes_the_expected_module() {
    let source = std::fs::read_to_string(manifest().join("examples/combat.pure.graphite")).unwrap();
    let graphs = pure_graphs(&source);
    let expected =
        std::fs::read_to_string(manifest().join("tests/fixtures/pure/Combat.purs")).unwrap();
    assert_eq!(emitted(purescript(), &graphs[0]), expected);
}

#[test]
fn the_file_name_is_the_module_name_with_the_purs_extension() {
    let graphs = pure_graphs(
        "graph combat\n  enum E a\n  input x : bool\n  choose c : E\n    when x -> a\n    else -> a\n",
    );
    let front = purescript();
    assert_eq!(front.file_name(&graphs[0], Module::Index), "Combat.purs");
    assert_eq!(front.kind(), Kind::Pure);
    assert_eq!(front.runtime_file_name_opt(), None);
}

/// The top-level names of a module: value signatures, types, and the constructors of `data`.
fn top_level_names(text: &str) -> HashSet<String> {
    let mut names = HashSet::new();
    for line in text.lines() {
        if let Some((name, _)) = line.split_once(" :: ")
            && !line.starts_with(' ')
            && !name.contains(' ')
        {
            names.insert(name.to_owned());
        }
        for keyword in ["type ", "data "] {
            if let Some(rest) = line.strip_prefix(keyword) {
                let head = rest.split([' ', '=']).next().unwrap();
                names.insert(head.to_owned());
                if let Some(constructors) = rest.split_once(" = ").filter(|_| keyword == "data ") {
                    names.extend(constructors.1.split(" | ").map(|c| c.to_owned()));
                }
            }
        }
    }
    names.remove("derive");
    names
}

#[test]
fn every_top_level_name_is_a_source_name_or_a_generated_one() {
    let source = std::fs::read_to_string(manifest().join("examples/combat.pure.graphite")).unwrap();
    let graphs = pure_graphs(&source);
    let text = std::fs::read_to_string(manifest().join("tests/fixtures/pure/Combat.purs")).unwrap();
    let decisions = lower(&graphs[0]);
    let language = &PURESCRIPT;
    // Source names, spelled as the module spells them.
    let mut known: HashSet<String> = HashSet::new();
    for item in &decisions.enums {
        known.insert(language.ident(Role::Type, item.name));
        known.extend(
            item.members
                .iter()
                .map(|member| language.ident(Role::Member, member.name)),
        );
    }
    known.extend(
        decisions
            .functions
            .iter()
            .map(|f| language.ident(Role::Function, f.name)),
    );
    // The generated names, which the layout also emits.
    let generated: HashSet<String> = PureScriptSyntax
        .generated(&decisions)
        .into_iter()
        .map(|generated| generated.name)
        .collect();
    for name in top_level_names(&text) {
        assert!(
            known.contains(&name) || generated.contains(&name),
            "`{name}` is neither a source name nor generated"
        );
    }
    // Every capitalised word of the module is a name the module declares, generates, or imports
    // (or the module's own name and its imported modules' names). The layout's lowercase
    // reference from Prelude, `otherwise`, is listed too.
    let imported: HashSet<String> = PureScriptSyntax
        .imported()
        .into_iter()
        .map(|(_, name)| name.to_owned())
        .collect();
    let module_path: HashSet<&str> = ["Prelude", "Data", "Maybe"].into_iter().collect();
    let module_name = language.ident(Role::Module, decisions.name);
    assert!(imported.contains("otherwise"));
    for word in text
        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|word| word.starts_with(|c: char| c.is_ascii_uppercase()))
    {
        assert!(
            known.contains(word)
                || generated.contains(word)
                || imported.contains(word)
                || module_path.contains(word)
                || word == module_name,
            "`{word}` is used by the module but is not a source, generated or imported name"
        );
    }
}

#[test]
fn the_purescript_target_is_inferred_for_its_extension_and_kind() {
    assert_eq!(
        registry::for_infix("purs", Kind::Pure).map(|target| target.key),
        Some("purescript")
    );
    assert_eq!(DEFAULT_PURE_FRONT_END, "purescript");
    assert!(lookup(DEFAULT_PURE_FRONT_END).is_some());
}

#[test]
fn a_source_name_that_clashes_with_a_generated_type_is_refused() {
    // The record type `Input` is generated, so an enum named `Input` clashes with it.
    let graphs = pure_graphs(
        "graph g\n  enum Input a\n  input x : bool\n  choose c : Input\n    when x -> a\n    else -> a\n",
    );
    let mut out = Vec::new();
    let refused = purescript()
        .emit_module(&graphs[0], Module::Index, &mut out)
        .unwrap_err();
    assert_eq!(
        refused.to_string(),
        "`Input` clashes with the generated name `Input`"
    );
}

fn refused_by_purescript(source: &str) -> String {
    let graphs = pure_graphs(source);
    let mut out = Vec::new();
    purescript()
        .emit_module(&graphs[0], Module::Index, &mut out)
        .expect_err("expected the name check to refuse the module")
        .to_string()
}

#[test]
fn a_member_spelled_like_a_generated_type_is_refused() {
    let message = refused_by_purescript(
        "graph g\n  enum E Input\n  input x : bool\n  choose c : E\n    when x -> Input\n    else -> Input\n",
    );
    assert_eq!(message, "`Input` clashes with the generated name `Input`");
}

#[test]
fn a_choice_named_like_a_generated_helper_is_refused() {
    let message = refused_by_purescript(
        "graph g\n  enum E a\n  input x : bool\n  choose tactic : E\n    when x -> a\n    else -> a\n  choose tacticArm : E\n    when x -> a\n    else -> a\n",
    );
    assert_eq!(
        message,
        "`tacticArm` clashes with the name `tacticArm`, which the choice `tactic` generates"
    );
}

#[test]
fn one_member_in_two_enums_is_refused_in_purescript() {
    let message = refused_by_purescript(
        "graph g\n  enum A x\n  enum B x\n  input k : bool\n  choose c : A\n    when k -> x\n    else -> x\n",
    );
    assert_eq!(message, "`x` of `B` and `x` of `A` both become `X`");
}

#[test]
fn an_input_named_tag_builds_because_fields_are_their_own_namespace() {
    let graphs = pure_graphs(
        "graph g\n  enum E a\n  input tag : bool\n  choose c : E\n    when tag -> a\n    else -> a\n",
    );
    let text = emitted(purescript(), &graphs[0]);
    assert!(text.contains("type Input = { tag :: Boolean }"), "{text}");
    assert!(text.contains("i.tag"), "{text}");
}

#[test]
fn a_member_spelled_like_an_imported_constructor_is_refused() {
    let message = refused_by_purescript(
        "graph g\n  enum E Nothing b\n  input x : bool\n  choose c : E\n    when x -> Nothing\n    else -> b\n",
    );
    assert_eq!(
        message,
        "`Nothing` clashes with the imported name `Nothing`"
    );
}

#[test]
fn an_enum_named_like_an_imported_type_is_refused() {
    let message = refused_by_purescript(
        "graph g\n  enum Maybe a\n  input x : bool\n  choose c : Maybe\n    when x -> a\n    else -> a\n",
    );
    assert_eq!(message, "`Maybe` clashes with the imported name `Maybe`");
}

#[test]
fn a_choice_named_otherwise_is_refused_because_its_guards_use_the_prelude_one() {
    let message = refused_by_purescript(
        "graph g\n  enum E a\n  input x : bool\n  choose otherwise : E\n    when x -> a\n    else -> a\n",
    );
    assert_eq!(
        message,
        "`otherwise` clashes with the imported name `otherwise`"
    );
}

#[test]
fn a_name_with_no_letters_or_digits_is_refused() {
    let message = refused_by_purescript(
        "graph g\n  enum E a\n  input _ : bool\n  choose c : E\n    when _ -> a\n    else -> a\n",
    );
    assert_eq!(message, "`_` has no letters or digits to make a name from");
}

#[test]
fn a_member_that_starts_with_a_digit_is_refused_as_a_constructor() {
    let message = refused_by_purescript(
        "graph g\n  enum E 9a b\n  input x : bool\n  choose c : E\n    when x -> b\n    else -> b\n",
    );
    assert_eq!(
        message,
        "`9a` becomes `_9a`, which cannot be used as this name"
    );
}

#[test]
fn two_declarations_that_generate_the_same_helper_name_are_refused_by_their_sources() {
    let message = refused_by_purescript(
        "graph g\n  enum E a\n  input x : bool @src:s\n  choose x : E @k:v\n    when x -> a\n    else -> a\n",
    );
    assert_eq!(
        message,
        "the choice `x` and the input `x` both generate `xAnnotations`; rename one of them"
    );
}

#[test]
fn a_graph_named_like_an_imported_module_is_refused() {
    let message = refused_by_purescript(
        "graph prelude\n  enum E a\n  input x : bool\n  choose c : E\n    when x -> a\n    else -> a\n",
    );
    assert_eq!(
        message,
        "`prelude` clashes with the imported name `Prelude`"
    );
}

#[test]
fn an_enum_named_like_a_prelude_type_is_refused() {
    let message = refused_by_purescript(
        "graph g\n  enum Ordering a\n  input x : bool\n  choose c : Ordering\n    when x -> a\n    else -> a\n",
    );
    assert_eq!(
        message,
        "`Ordering` clashes with the imported name `Ordering`"
    );
}

#[test]
fn a_choice_named_i_is_refused_because_its_dispatch_parameter_is_i() {
    let message = refused_by_purescript(
        "graph g\n  enum E a\n  input x : bool\n  choose i : E\n    when x -> a\n    else -> a\n",
    );
    assert_eq!(message, "`i` clashes with the imported name `i`");
}

#[test]
fn an_enum_named_like_a_built_in_prim_type_is_refused() {
    let message = refused_by_purescript(
        "graph g\n  enum Int a\n  input x : bool\n  choose c : Int\n    when x -> a\n    else -> a\n",
    );
    assert_eq!(message, "`Int` clashes with the imported name `Int`");
}
