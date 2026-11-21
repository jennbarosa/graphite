//! Which front end a file uses, and whether it is pure. The rules are in `docs/cli.md`; this module
//! applies them to a file name, the two command-line flags, and the file's directives.

use graphite::frontend::registry::{self, Target};
use graphite::kind::Kind;
use graphite::preprocess::Directives;

/// The front end and mode for one file.
pub struct Resolved {
    pub target: &'static Target,
    /// Whether the file is pure, so it is parsed as a pure file.
    pub pure: bool,
    /// What made the file pure, as a phrase such as "by `--pure`" or "because of `--frontend
    /// purescript`". `None` for a machine file.
    pub because: Option<String>,
}

/// Resolves `name` (the file's name, without its directory), the `--frontend` and `--pure` flags,
/// and the directives read from its source. Errors name the signals that conflict.
pub fn resolve(
    name: &str,
    flag_front_end: Option<&str>,
    flag_pure: bool,
    directives: &Directives<'_>,
) -> Result<Resolved, String> {
    let infix = name_infix(name);
    let directive_pure = directives
        .frontend
        .is_some_and(|(value, _)| value == "pure");
    // A directive loses to the flag, so it is ignored completely unless it is `@!frontend:pure`,
    // which names no target and always counts.
    let explicit = match (flag_front_end, directives.frontend) {
        (Some(key), _) => Some(Named {
            key,
            spelled: format!("`--frontend {key}`"),
            from_flag: true,
        }),
        (None, Some((value, _))) if value != "pure" => Some(Named {
            key: value,
            spelled: format!("`@!frontend:{value}`"),
            from_flag: false,
        }),
        _ => None,
    };
    let explicit_target = match &explicit {
        Some(named) => Some(lookup(named, name)?),
        None => None,
    };
    let explicit_pure = explicit_target.is_some_and(|target| target.frontend.kind() == Kind::Pure);

    let reason = pure_reason(
        name,
        flag_pure,
        directives,
        directive_pure,
        explicit_pure
            .then(|| explicit.as_ref().map(|named| named.spelled.as_str()))
            .flatten(),
        infix,
    );

    let target = match (&explicit, explicit_target) {
        (Some(named), Some(target)) => {
            let key = named.key;
            if let Some(reason) = &reason
                && target.frontend.kind() == Kind::Machine
            {
                return Err(format!(
                    "pure ({reason}), but `{key}` is a machine front end (pure front ends: {})",
                    pure_front_ends()
                ));
            }
            target
        }
        _ => match infix {
            Some(infix) => infix_target(name, infix, reason.as_deref())?,
            None => registry::lookup(if reason.is_some() {
                registry::DEFAULT_PURE_FRONT_END
            } else {
                registry::DEFAULT_FRONT_END
            })
            .expect("the default front end is registered"),
        },
    };
    Ok(Resolved {
        target,
        pure: reason.is_some(),
        because: reason,
    })
}

/// A front end named by a flag or a directive, and how the user wrote it.
struct Named<'a> {
    key: &'a str,
    spelled: String,
    from_flag: bool,
}

/// The infix of `name`, read as the rules say: strip `.graphite`, then an optional `.pure`, and the
/// part after the last `.` of what is left. A name without `.graphite` has no infix.
fn name_infix(name: &str) -> Option<&str> {
    let stem = name.strip_suffix(".graphite")?;
    let stem = stem.strip_suffix(".pure").unwrap_or(stem);
    stem.rsplit_once('.').map(|(_, infix)| infix)
}

/// The first pure signal that holds, as a phrase for a diagnostic. `None` for a machine file.
/// `explicit_pure` is how a pure front end named by a flag or directive is written, if it is one.
fn pure_reason(
    name: &str,
    flag_pure: bool,
    directives: &Directives<'_>,
    directive_pure: bool,
    explicit_pure: Option<&str>,
    infix: Option<&str>,
) -> Option<String> {
    if flag_pure {
        return Some("by `--pure`".to_owned());
    }
    if directives.pure.is_some() {
        return Some("by `@!pure`".to_owned());
    }
    if directive_pure {
        return Some("by `@!frontend:pure`".to_owned());
    }
    if name.ends_with(".pure.graphite") {
        return Some("by its `.pure` name".to_owned());
    }
    if let Some(spelled) = explicit_pure {
        return Some(format!("because of {spelled}"));
    }
    if let Some(infix) = infix
        && targets_with_extension(infix).next().is_some()
        && targets_with_extension(infix).all(|target| target.frontend.kind() == Kind::Pure)
    {
        return Some(format!("by its `.{infix}` infix"));
    }
    None
}

/// The target a flag or directive names, or an error that says which one is wrong.
fn lookup(named: &Named<'_>, name: &str) -> Result<&'static Target, String> {
    registry::lookup(named.key).ok_or_else(|| {
        let front_ends = front_end_keys();
        if named.from_flag {
            format!(
                "unknown front end \"{}\"; the front ends are {front_ends}",
                named.key
            )
        } else {
            format!(
                "`{name}`: `@!frontend:{}` names no front end; the front ends are {front_ends}",
                named.key
            )
        }
    })
}

/// The target for a file with an infix and no named front end.
fn infix_target(name: &str, infix: &str, reason: Option<&str>) -> Result<&'static Target, String> {
    let kind = if reason.is_some() {
        Kind::Pure
    } else {
        Kind::Machine
    };
    if let Some(target) = registry::for_infix(infix, kind) {
        return Ok(target);
    }
    if targets_with_extension(infix).next().is_none() {
        return Err(format!(
            "`{name}`: no front end for the infix `.{infix}`; pass `--frontend` or add `@!frontend:<key>` (known infixes: {})",
            known_infixes()
        ));
    }
    let reason = reason.unwrap_or_default();
    Err(format!(
        "`{name}` is pure ({reason}), but no pure front end writes `.{infix}` (pure front ends: {})",
        pure_front_ends()
    ))
}

fn targets_with_extension(infix: &str) -> impl Iterator<Item = &'static Target> + '_ {
    registry::TARGETS
        .iter()
        .filter(move |target| target.frontend.extension() == infix)
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

fn pure_front_ends() -> String {
    registry::TARGETS
        .iter()
        .filter(|target| target.frontend.kind() == Kind::Pure)
        .map(|target| target.key)
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use graphite::preprocess::Directives;

    use super::resolve;

    fn none() -> Directives<'static> {
        Directives {
            pure: None,
            frontend: None,
            body: 0,
        }
    }

    fn with_pure() -> Directives<'static> {
        Directives {
            pure: Some(0),
            ..none()
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
        flag_pure: bool,
        directives: &Directives<'_>,
    ) -> Result<(&'static str, bool), String> {
        resolve(name, flag, flag_pure, directives).map(|r| (r.target.key, r.pure))
    }

    #[test]
    fn the_inference_table_from_the_plan() {
        let none = none();
        assert_eq!(
            key("door.graphite", None, false, &none),
            Ok(("typescript", false))
        );
        assert_eq!(
            key("combat.pure.graphite", None, false, &none),
            Ok(("purescript", true))
        );
        assert_eq!(
            key("door.ts.graphite", None, false, &none),
            Ok(("typescript", false))
        );
        assert_eq!(
            key("door.rs.graphite", None, false, &none),
            Ok(("rust", false))
        );
        assert_eq!(
            key("combat.purs.graphite", None, false, &none),
            Ok(("purescript", true))
        );
    }

    #[test]
    fn a_pure_name_with_a_machine_infix_is_refused() {
        assert_eq!(
            key("combat.ts.pure.graphite", None, false, &none()),
            Err("`combat.ts.pure.graphite` is pure (by its `.pure` name), but no pure front end writes `.ts` (pure front ends: purescript)".to_owned())
        );
    }

    #[test]
    fn an_unknown_infix_is_refused_unless_something_names_the_front_end() {
        let message = key("my.walk.graphite", None, false, &none()).unwrap_err();
        assert_eq!(
            message,
            "`my.walk.graphite`: no front end for the infix `.walk`; pass `--frontend` or add `@!frontend:<key>` (known infixes: ts, rs, purs)"
        );
        assert_eq!(
            key("my.walk.graphite", Some("typescript"), false, &none()),
            Ok(("typescript", false))
        );
        assert_eq!(
            key(
                "my.walk.graphite",
                None,
                false,
                &with_frontend("typescript")
            ),
            Ok(("typescript", false))
        );
        // `--pure` does not name a front end, so it does not silence the error.
        assert!(key("my.walk.graphite", None, true, &none()).is_err());
    }

    #[test]
    fn a_flag_beats_a_directive_without_error() {
        let purescript_directive = with_frontend("purescript");
        assert_eq!(
            key(
                "door.graphite",
                Some("typescript"),
                false,
                &purescript_directive
            ),
            Ok(("typescript", false))
        );
    }

    #[test]
    fn a_pure_flag_with_a_machine_front_end_names_both_sides() {
        assert_eq!(
            key("door.graphite", Some("typescript"), true, &none()),
            Err("pure (by `--pure`), but `typescript` is a machine front end (pure front ends: purescript)".to_owned())
        );
    }

    #[test]
    fn a_pure_directive_with_a_machine_front_end_is_refused() {
        let directives = Directives {
            pure: Some(0),
            ..with_frontend("typescript")
        };
        assert_eq!(
            key("door.graphite", None, false, &directives),
            Err("pure (by `@!pure`), but `typescript` is a machine front end (pure front ends: purescript)".to_owned())
        );
    }

    #[test]
    fn a_machine_file_sent_to_a_pure_front_end_is_pure_because_of_the_target() {
        let resolved = resolve("door.graphite", Some("purescript"), false, &none()).unwrap();
        assert!(resolved.pure);
        assert_eq!(
            resolved.because.as_deref(),
            Some("because of `--frontend purescript`")
        );
    }

    #[test]
    fn a_pure_directive_or_frontend_pure_makes_the_default_pure_front_end() {
        assert_eq!(
            key("door.graphite", None, false, &with_pure()),
            Ok(("purescript", true))
        );
        assert_eq!(
            key("door.graphite", None, false, &with_frontend("pure")),
            Ok(("purescript", true))
        );
    }

    #[test]
    fn an_unknown_directive_front_end_lists_the_keys() {
        assert_eq!(
            key("door.graphite", None, false, &with_frontend("cobol")),
            Err("`door.graphite`: `@!frontend:cobol` names no front end; the front ends are typescript, rust, purescript".to_owned())
        );
    }

    #[test]
    fn a_purs_infix_is_pure_by_rule_five() {
        let resolved = resolve("combat.purs.graphite", None, false, &none()).unwrap();
        assert_eq!(resolved.because.as_deref(), Some("by its `.purs` infix"));
    }
}
