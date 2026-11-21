//! PureScript's lexical rules: its string escapes, `Nothing` and `Just`, its reserved words, and
//! how each role is spelled. The layout of a PureScript module is in `frontend/pure/purescript.rs`.

use super::{Language, Namespace, Role, camel, is_plain_ident, pascal};
use crate::frontend::escape::Escape;

/// The PureScript rules. Stateless, so one static value serves every module.
pub struct PureScript;

pub static PURESCRIPT: PureScript = PureScript;

/// PureScript's reserved words. An identifier spelled like one gets a `_` appended.
const KEYWORDS: &[&str] = &[
    "ado", "as", "case", "class", "data", "derive", "do", "else", "false", "forall", "foreign",
    "hiding", "if", "import", "in", "infix", "infixl", "infixr", "instance", "let", "module",
    "newtype", "of", "then", "true", "type", "where",
];

impl Language for PureScript {
    fn extension(&self) -> &'static str {
        "purs"
    }

    fn escape(&self, ch: char) -> Escape {
        match ch {
            '"' => Escape::Text("\\\""),
            '\\' => Escape::Text("\\\\"),
            '\n' => Escape::Text("\\n"),
            '\r' => Escape::Text("\\r"),
            '\t' => Escape::Text("\\t"),
            // PureScript reads `\x` greedily up to six digits, so the width is fixed.
            ch if ch < ' ' => Escape::Hex {
                prefix: "\\x",
                width: 6,
            },
            _ => Escape::Keep,
        }
    }

    fn none(&self) -> &'static str {
        "Nothing"
    }

    fn some(&self, inner: &str) -> String {
        format!("(Just {inner})")
    }

    fn keywords(&self) -> &'static [&'static str] {
        KEYWORDS
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

    fn is_valid(&self, role: Role, spelled: &str) -> bool {
        let plain = is_plain_ident(spelled, &[]);
        match role {
            // Modules, types and constructors must start with an upper-case letter.
            Role::Module | Role::Type | Role::Member => {
                plain && spelled.starts_with(|c: char| c.is_ascii_uppercase())
            }
            Role::Field | Role::Function | Role::Constant => plain,
        }
    }

    fn namespace(&self, role: Role) -> Namespace {
        match role {
            Role::Module => Namespace::Module("module"),
            // A member is a data constructor, which shares the type namespace.
            Role::Type | Role::Member => Namespace::Module("type"),
            // A field is a record label, which is local to its record.
            Role::Field => Namespace::Owner,
            Role::Function | Role::Constant => Namespace::Module("value"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::PURESCRIPT;
    use crate::frontend::lang::{Language, Namespace, Role};

    fn quoted(value: &str) -> String {
        let mut out = Vec::new();
        PURESCRIPT.string(value, &mut out).unwrap();
        String::from_utf8(out).unwrap()
    }

    #[test]
    fn control_characters_take_six_hex_digits() {
        assert_eq!(quoted("\u{1f}a"), "\"\\x00001fa\"");
        assert_eq!(quoted("\u{0}"), "\"\\x000000\"");
    }

    #[test]
    fn quote_backslash_and_the_named_controls_are_escaped() {
        assert_eq!(quoted("\"\\"), "\"\\\"\\\\\"");
        assert_eq!(quoted("\n\r\t"), "\"\\n\\r\\t\"");
    }

    #[test]
    fn every_keyword_is_escaped_with_an_underscore() {
        for keyword in super::KEYWORDS {
            assert_eq!(
                PURESCRIPT.ident(Role::Function, keyword),
                format!("{keyword}_")
            );
        }
    }

    #[test]
    fn each_role_is_spelled_with_its_case_rule() {
        assert_eq!(PURESCRIPT.spell(Role::Module, "combat"), "Combat");
        assert_eq!(PURESCRIPT.spell(Role::Type, "dir"), "Dir");
        assert_eq!(PURESCRIPT.spell(Role::Member, "home-east"), "HomeEast");
        assert_eq!(PURESCRIPT.spell(Role::Field, "home-east"), "homeEast");
        assert_eq!(PURESCRIPT.spell(Role::Function, "home-east"), "homeEast");
        assert_eq!(PURESCRIPT.spell(Role::Constant, "Home-East"), "homeEast");
    }

    #[test]
    fn each_role_has_its_namespace() {
        assert_eq!(
            PURESCRIPT.namespace(Role::Module),
            Namespace::Module("module")
        );
        assert_eq!(PURESCRIPT.namespace(Role::Type), Namespace::Module("type"));
        assert_eq!(
            PURESCRIPT.namespace(Role::Member),
            Namespace::Module("type")
        );
        assert_eq!(PURESCRIPT.namespace(Role::Field), Namespace::Owner);
        assert_eq!(
            PURESCRIPT.namespace(Role::Function),
            Namespace::Module("value")
        );
        assert_eq!(
            PURESCRIPT.namespace(Role::Constant),
            Namespace::Module("value")
        );
    }

    #[test]
    fn maybe_literals_are_nothing_and_a_parenthesised_just() {
        assert_eq!(PURESCRIPT.none(), "Nothing");
        assert_eq!(PURESCRIPT.some("\"x\""), "(Just \"x\")");
        assert_eq!(PURESCRIPT.extension(), "purs");
    }
}
