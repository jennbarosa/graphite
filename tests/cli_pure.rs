//! Pure files through the command line: how a file's name, its directives and the flags pick the
//! front end and the mode, and what each command does with a pure file.

#[allow(dead_code)]
#[path = "support/cli.rs"]
mod cli;

use cli::{dsl, graphite, manifest, scratch, stderr, stdout};

const PURE: &str =
    "graph g\n  enum E a\n  input x : bool\n  choose c : E\n    when x -> a\n    else -> a\n";

fn combat() -> std::path::PathBuf {
    manifest().join("examples/combat.purs.graphite")
}

#[test]
fn a_pure_name_checks_as_a_purescript_file() {
    let output = graphite(["check".as_ref(), combat().as_os_str()]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stdout(&output), "ok: 1 graph\n");
}

#[test]
fn a_purs_infix_is_pure_by_rule_five() {
    let source = dsl("pure-infix/combat.purs.graphite", PURE);
    let output = graphite(["check".as_ref(), source.as_os_str()]);
    assert!(output.status.success(), "{}", stderr(&output));
}

#[test]
fn only_the_last_infix_names_the_front_end() {
    let source = dsl("pure-infix/combat.ts.purs.graphite", PURE);
    let output = graphite(["check".as_ref(), source.as_os_str()]);
    assert!(output.status.success(), "{}", stderr(&output));
}

#[test]
fn a_pure_name_is_no_longer_a_pure_signal() {
    let source = dsl("pure-infix/combat.pure.graphite", PURE);
    let output = graphite(["check".as_ref(), source.as_os_str()]);
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("no front end for the infix `.pure`"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn an_unknown_infix_names_the_known_ones() {
    let source = dsl("pure-infix/my.walk.graphite", "graph w\n  node a end\n");
    let output = graphite(["check".as_ref(), source.as_os_str()]);
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("no front end for the infix `.walk`"),
        "{}",
        stderr(&output)
    );
    assert!(stderr(&output).contains("(known infixes: ts, rs, purs)"));
}

#[test]
fn a_machine_node_in_a_purescript_file_says_why_it_is_pure() {
    let source = dsl("pure-flag/door.graphite", "graph door\n  node a end\n");
    let output = graphite([
        "check".as_ref(),
        source.as_os_str(),
        "--frontend".as_ref(),
        "purescript".as_ref(),
    ]);
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains(
            "`node` is not allowed in a pure file (the `purescript` front end writes pure files)"
        ),
        "{}",
        stderr(&output)
    );
}

#[test]
fn the_pure_flag_is_gone_from_every_command_that_had_it() {
    let source = dsl("pure-flag/gone.graphite", PURE);
    for command in ["check", "compile", "inspect"] {
        let output = graphite([command.as_ref(), source.as_os_str(), "--pure".as_ref()]);
        assert_eq!(output.status.code(), Some(2), "{command}");
        assert!(
            stderr(&output).contains("unknown option --pure"),
            "{command}: {}",
            stderr(&output)
        );
    }
}

#[test]
fn a_flag_beats_a_purescript_directive_and_the_file_builds_as_machine() {
    let source = dsl(
        "pure-flag/beats.graphite",
        "@!frontend:purescript\ngraph door\n  node a end\n",
    );
    let output = graphite([
        "check".as_ref(),
        source.as_os_str(),
        "--frontend".as_ref(),
        "typescript".as_ref(),
    ]);
    assert!(output.status.success(), "{}", stderr(&output));
}

#[test]
fn a_rust_infix_builds_rust_and_keeps_the_infix_out_of_the_output_name() {
    let source = dsl("rust-infix/door.rs.graphite", "graph door\n  node a end\n");
    let out = scratch("rust-infix/out");
    let _ = std::fs::remove_dir_all(&out);
    let output = graphite([
        "compile".as_ref(),
        source.as_os_str(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(
        out.join("door.rs").exists(),
        "{:?}",
        std::fs::read_dir(&out)
    );
}

#[test]
fn compile_writes_the_purescript_module_and_no_runtime() {
    let out = scratch("pure-compile/out");
    let _ = std::fs::remove_dir_all(&out);
    let output = graphite([
        "compile".as_ref(),
        combat().as_os_str(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);
    assert!(output.status.success(), "{}", stderr(&output));
    let mut names: Vec<String> = std::fs::read_dir(&out)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, ["Combat.purs"]);
    let expected =
        std::fs::read_to_string(manifest().join("tests/fixtures/pure/Combat.purs")).unwrap();
    assert_eq!(
        std::fs::read_to_string(out.join("Combat.purs")).unwrap(),
        expected
    );
}

#[test]
fn run_and_debug_refuse_a_pure_file() {
    for command in ["run", "debug"] {
        let output = graphite([command.as_ref(), combat().as_os_str()]);
        assert!(!output.status.success(), "{command}");
        assert!(
            stderr(&output)
                .contains("is a pure file; `graphite run` and `graphite debug` run machine graphs"),
            "{command}: {}",
            stderr(&output)
        );
    }
}

#[test]
fn run_refuses_the_removed_pure_flag() {
    let output = graphite(["run".as_ref(), combat().as_os_str(), "--pure".as_ref()]);
    assert_eq!(output.status.code(), Some(2));
    assert!(
        stderr(&output).contains("unknown option --pure"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn inspect_reports_the_kind_and_the_declarations_of_a_pure_file() {
    let output = graphite(["inspect".as_ref(), combat().as_os_str(), "--json".as_ref()]);
    assert!(output.status.success(), "{}", stderr(&output));
    let text = stdout(&output);
    assert!(text.contains("\"kind\":\"pure\""), "{text}");
    assert!(text.contains("\"enums\":[{\"name\":\"Tactic\""), "{text}");
    assert!(
        text.contains("\"inputs\":[{\"name\":\"homeEast\",\"type\":\"bool\""),
        "{text}"
    );
}

#[test]
fn inspect_text_marks_a_pure_graph_and_leaves_a_machine_graph_alone() {
    let pure = graphite(["inspect".as_ref(), combat().as_os_str()]);
    assert!(stdout(&pure).contains("  kind pure\n  enums\n    Tactic = flee | hit | hold\n"));
    let door = graphite([
        "inspect".as_ref(),
        manifest().join("examples/door.graphite").as_os_str(),
    ]);
    assert!(!stdout(&door).contains("kind"), "{}", stdout(&door));
}

#[test]
fn an_allowlist_refuses_a_disallowed_value_on_an_input_line() {
    let allow = dsl(
        "pure-allow/list.toml",
        "[annotations]\nsource = [\"stats\"]\n",
    );
    let source = dsl(
        "pure-allow/bad.purs.graphite",
        "graph g\n  enum E a\n  input x : bool @source:guess\n  choose c : E\n    when x -> a\n    else -> a\n",
    );
    let output = graphite([
        "check".as_ref(),
        source.as_os_str(),
        "--allow".as_ref(),
        allow.as_os_str(),
    ]);
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("input x")
            && stderr(&output).contains("@source:guess is not allowed"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn doctor_lists_the_purescript_front_end() {
    let output = graphite(["doctor"]);
    assert!(
        stdout(&output).contains("purescript"),
        "{}",
        stdout(&output)
    );
}

#[test]
fn inspect_reads_a_dotted_name_with_an_unknown_infix_without_a_flag() {
    // Only compile and check write front-end output, so only they refuse an unknown infix.
    let path = dsl("infix-read/my.walk.graphite", "graph door\n  node a end\n");
    let output = graphite(["inspect".as_ref(), path.as_os_str()]);
    assert!(output.status.success(), "{}", stderr(&output));
    let output = graphite(["check".as_ref(), path.as_os_str()]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("no front end for the infix `.walk`"));
}

#[test]
fn check_refuses_a_name_clash_that_compile_refuses() {
    let path = dsl(
        "clash/clash.purs.graphite",
        "graph g\n  enum E a b\n  input home-east : bool\n  input home_east : bool\n  choose c : E\n    when home-east -> a\n    else -> b\n",
    );
    let output = graphite(["check".as_ref(), path.as_os_str()]);
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("both become `homeEast`"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn a_name_clash_points_at_the_later_declaration() {
    let path = dsl(
        "clash-line/clash.purs.graphite",
        "graph g\n  enum E a b\n  input home-east : bool\n  input home_east : bool\n  choose c : E\n    when home-east -> a\n    else -> b\n",
    );
    let output = graphite(["check".as_ref(), path.as_os_str()]);
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("--> line 4"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn a_name_clash_gets_no_remove_the_output_hint() {
    let path = dsl(
        "clash-hint/clash.purs.graphite",
        "graph g\n  enum Tactic flee\n  input x : bool\n  choose tactic : Tactic\n    when x -> flee\n    else -> flee\n  choose tacticArm : Tactic\n    when x -> flee\n    else -> flee\n",
    );
    let out = scratch("clash-hint-out");
    let output = graphite([
        "compile".as_ref(),
        path.as_os_str(),
        "--out".as_ref(),
        out.as_os_str(),
    ]);
    assert!(!output.status.success());
    assert!(!stderr(&output).contains("hint:"), "{}", stderr(&output));
}

#[test]
fn a_front_end_named_by_a_flag_ignores_a_pure_infix() {
    let path = dsl(
        "flag-infix/door.purs.graphite",
        "graph door\n  node closed end\n",
    );
    let output = graphite([
        "check".as_ref(),
        path.as_os_str(),
        "--frontend".as_ref(),
        "typescript".as_ref(),
    ]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(stdout(&output), "ok: 1 graph\n");
}

#[test]
fn check_refuses_a_rust_constant_clash_that_compile_refuses() {
    // `a-b` and `a_b` both become the Rust constant `A_B`. Only rendering finds this, so `check`
    // must render too.
    let path = dsl(
        "rust-clash/door.graphite",
        "graph door\n  node a-b\n  node a_b end\n  edge a-b -> a_b when go\n",
    );
    let check = graphite([
        "check".as_ref(),
        path.as_os_str(),
        "--frontend".as_ref(),
        "rust".as_ref(),
    ]);
    assert!(!check.status.success());
    assert!(
        stderr(&check).contains("both become the Rust constant `A_B`"),
        "{}",
        stderr(&check)
    );
    assert!(stderr(&check).contains("--> line 1"), "{}", stderr(&check));
}
