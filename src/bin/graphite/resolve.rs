//! Which front end a file uses, and so its mode: a pure file is one whose front end is pure. The
//! rules are in `docs/cli.md`; this module applies them to the `--frontend` flag, the file's
//! directives, and its name.

use graphite::frontend::registry::{self, Target};
use graphite::kind::Kind;
use graphite::pipeline::build::Chosen;
use graphite::preprocess::Directives;

/// What a command does with a file's front end. Every command honors a known infix, so a file has
/// one mode everywhere. Only a command that writes front-end output refuses an unknown infix.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Output {
    /// `compile` and `check` write front-end output, so an unknown infix is an error.
    Written,
    /// Every other command only reads the graphs, so an unknown infix falls back to the default.
    Unused,
}

/// Resolves the front end of `name` (the file's name, without its directory). The first of these
/// that applies decides it: the `--frontend` flag, the `@!frontend` directive, the file's infix
/// (only when `output` writes front-end output), and the default.
pub fn resolve(
    name: &str,
    flag_front_end: Option<&str>,
    directives: &Directives<'_>,
    output: Output,
) -> Result<Chosen, String> {
    let target = match (flag_front_end, directives.frontend) {
        (Some(key), _) => registry::lookup(key).ok_or_else(|| {
            format!(
                "unknown front end \"{key}\"; the front ends are {}",
                front_end_keys()
            )
        })?,
        (None, Some((key, _))) => registry::lookup(key).ok_or_else(|| {
            format!(
                "`{name}`: `@!frontend:{key}` names no front end; the front ends are {}",
                front_end_keys()
            )
        })?,
        (None, None) => match name_infix(name) {
            Some(infix) => match registry::for_infix(infix) {
                Some(target) => target,
                None if output == Output::Written => return Err(unknown_infix(name, infix)),
                None => default_target(),
            },
            None => default_target(),
        },
    };
    Ok(Chosen {
        target,
        pure: target.frontend.kind() == Kind::Pure,
    })
}

/// The infix of `name`: the part after the last `.` before `.graphite`. A name without
/// `.graphite`, or without a `.` before it, has no infix.
fn name_infix(name: &str) -> Option<&str> {
    let stem = name.strip_suffix(".graphite")?;
    stem.rsplit_once('.').map(|(_, infix)| infix)
}

fn unknown_infix(name: &str, infix: &str) -> String {
    format!(
        "`{name}`: no front end for the infix `.{infix}`; pass `--frontend` or add `@!frontend:<key>` (known infixes: {})",
        known_infixes()
    )
}

fn default_target() -> &'static Target {
    registry::lookup(registry::DEFAULT_FRONT_END).expect("the default front end is registered")
}

fn front_end_keys() -> String {
    registry::TARGETS
        .iter()
        .map(|target| target.key)
        .collect::<Vec<_>>()
        .join(", ")
}

fn known_infixes() -> String {
    registry::TARGETS
        .iter()
        .map(|target| target.frontend.extension())
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use graphite::preprocess::Directives;

    use super::{Output, resolve};

    fn none() -> Directives<'static> {
        Directives {
            frontend: None,
            body: 0,
        }
    }

    fn with_frontend(value: &'static str) -> Directives<'static> {
        Directives {
            frontend: Some((value, 0)),
            ..none()
        }
    }

    /// The resolved front end's key and mode, or the error.
    fn key(
        name: &str,
        flag: Option<&str>,
        directives: &Directives<'_>,
    ) -> Result<(&'static str, bool), String> {
        resolve(name, flag, directives, Output::Written).map(|r| (r.target.key, r.pure))
    }

    #[test]
    fn the_infix_and_the_default_pick_the_front_end_and_its_mode() {
        let none = none();
        assert_eq!(key("door.graphite", None, &none), Ok(("typescript", false)));
        assert_eq!(
            key("door.ts.graphite", None, &none),
            Ok(("typescript", false))
        );
        assert_eq!(key("door.rs.graphite", None, &none), Ok(("rust", false)));
        assert_eq!(
            key("combat.purs.graphite", None, &none),
            Ok(("purescript", true))
        );
    }

    #[test]
    fn a_pure_name_is_no_longer_a_signal() {
        assert_eq!(
            key("combat.pure.graphite", None, &none()),
            Err("`combat.pure.graphite`: no front end for the infix `.pure`; pass `--frontend` or add `@!frontend:<key>` (known infixes: ts, rs, purs)".to_owned())
        );
    }

    #[test]
    fn a_read_that_writes_no_output_falls_back_for_an_unknown_infix() {
        let resolved = resolve("my.walk.graphite", None, &none(), Output::Unused).unwrap();
        assert_eq!((resolved.target.key, resolved.pure), ("typescript", false));
    }

    #[test]
    fn a_read_that_writes_no_output_still_honors_a_known_infix() {
        let resolved = resolve("notes.purs.graphite", None, &none(), Output::Unused).unwrap();
        assert_eq!((resolved.target.key, resolved.pure), ("purescript", true));
    }

    #[test]
    fn an_unknown_infix_is_refused_unless_something_names_the_front_end() {
        assert_eq!(
            key("my.walk.graphite", None, &none()),
            Err("`my.walk.graphite`: no front end for the infix `.walk`; pass `--frontend` or add `@!frontend:<key>` (known infixes: ts, rs, purs)".to_owned())
        );
        assert_eq!(
            key("my.walk.graphite", Some("typescript"), &none()),
            Ok(("typescript", false))
        );
        assert_eq!(
            key("my.walk.graphite", None, &with_frontend("typescript")),
            Ok(("typescript", false))
        );
    }

    #[test]
    fn a_flag_beats_a_directive_without_error() {
        assert_eq!(
            key(
                "door.graphite",
                Some("typescript"),
                &with_frontend("purescript")
            ),
            Ok(("typescript", false))
        );
    }

    #[test]
    fn a_flag_or_directive_naming_a_pure_front_end_makes_the_file_pure() {
        assert_eq!(
            key("door.graphite", Some("purescript"), &none()),
            Ok(("purescript", true))
        );
        assert_eq!(
            key("door.graphite", None, &with_frontend("purescript")),
            Ok(("purescript", true))
        );
    }

    #[test]
    fn a_flag_or_directive_ignores_the_infix() {
        assert_eq!(
            key("door.purs.graphite", Some("typescript"), &none()),
            Ok(("typescript", false))
        );
    }

    #[test]
    fn the_old_pure_alias_is_an_unknown_front_end() {
        assert_eq!(
            key("door.graphite", None, &with_frontend("pure")),
            Err("`door.graphite`: `@!frontend:pure` names no front end; the front ends are typescript, rust, purescript".to_owned())
        );
    }

    #[test]
    fn an_unknown_directive_front_end_lists_the_keys() {
        assert_eq!(
            key("door.graphite", None, &with_frontend("cobol")),
            Err("`door.graphite`: `@!frontend:cobol` names no front end; the front ends are typescript, rust, purescript".to_owned())
        );
    }
}
