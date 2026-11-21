//! One language's lexical rules, shared by every front end that writes that language. A
//! [`Language`] states only its facts: the escape rule, the literals for `none` and `some`, its
//! keywords, and how each kind of name is spelled and which namespace it lives in. The loops, the
//! keyword lookup and the annotation literal are provided methods, so no language copies them.

pub mod rust_keywords;

use std::io::{self, Write};

use super::escape::{Escape, quoted};

/// The kind of name being spelled. A language maps each role to a case rule and a namespace.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Role {
    Module,
    Type,
    Member,
    Field,
    Function,
    Constant,
}

/// Where a spelled name must be unique. `Module` names are unique across the module, shared by
/// every role with the same label. `Owner` names are unique only within their enum or record.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Namespace {
    Module(&'static str),
    Owner,
}

pub trait Language: Sync {
    /// The extension of the language's files, without the dot.
    fn extension(&self) -> &'static str;
    /// The rule for one character of a string literal.
    fn escape(&self, ch: char) -> Escape;
    /// The literal for "no value", such as `Nothing`.
    fn none(&self) -> &'static str;
    /// The literal for a value, given the literal of its contents, such as `(Just x)`.
    fn some(&self, inner: &str) -> String;
    fn keywords(&self) -> &'static [&'static str];
    /// The case rule for a name in `role`. Use one of the converters in this module.
    fn spell(&self, role: Role, name: &str) -> String;
    /// Makes a keyword spelling safe, such as by appending `_`.
    fn escape_keyword(&self, spelled: String) -> String;
    fn namespace(&self, role: Role) -> Namespace;

    /// A string literal with its quotes. Written once here, from [`Language::escape`].
    fn string(&self, value: &str, out: &mut dyn Write) -> io::Result<()> {
        quoted(value, out, |ch| self.escape(ch))
    }

    /// The identifier for a name: its spelling, with a keyword made safe.
    fn ident(&self, role: Role, name: &str) -> String {
        let spelled = self.spell(role, name);
        if self.keywords().contains(&spelled.as_str()) {
            self.escape_keyword(spelled)
        } else {
            spelled
        }
    }

    /// An annotation as a literal: its key, and its value or `none`.
    fn annotation(&self, key: &str, value: Option<&str>, out: &mut dyn Write) -> io::Result<()> {
        out.write_all(b"{ key: ")?;
        self.string(key, out)?;
        out.write_all(b", value: ")?;
        match value {
            None => out.write_all(self.none().as_bytes())?,
            Some(value) => {
                let mut literal = Vec::new();
                self.string(value, &mut literal)?;
                let literal = String::from_utf8(literal).map_err(io::Error::other)?;
                out.write_all(self.some(&literal).as_bytes())?;
            }
        }
        out.write_all(b" }")
    }
}

/// Writes `_` when `name` starts with a digit, so a name can start an identifier.
pub fn digit_guard(name: &str, out: &mut String) {
    if name.starts_with(|c: char| c.is_ascii_digit()) {
        out.push('_');
    }
}

/// Whether `name` is an identifier as written (ASCII letters, digits and `_`, starting with a
/// letter or `_`) and not one of `keywords`.
pub fn is_plain_ident(name: &str, keywords: &[&str]) -> bool {
    let mut characters = name.chars();
    let starts_well = characters
        .next()
        .is_some_and(|first| first == '_' || first.is_ascii_alphabetic());
    starts_well
        && characters.all(|c| c == '_' || c.is_ascii_alphanumeric())
        && !keywords.contains(&name)
}

/// The lower-case words of `name`. A word ends at `-`, at `_`, or before an upper-case letter that
/// follows a lower-case letter or a digit. Empty words are dropped.
fn words(name: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut previous: Option<char> = None;
    for ch in name.chars() {
        if ch == '-' || ch == '_' {
            words.extend((!current.is_empty()).then(|| std::mem::take(&mut current)));
            previous = None;
            continue;
        }
        if ch.is_uppercase() && previous.is_some_and(|p| p.is_lowercase() || p.is_ascii_digit()) {
            words.extend((!current.is_empty()).then(|| std::mem::take(&mut current)));
        }
        current.extend(ch.to_lowercase());
        previous = Some(ch);
    }
    words.extend((!current.is_empty()).then_some(current));
    words
}

/// Upper-cases the first character of `word`, leaving the rest as it is.
fn capitalize(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// `homeEast` and `home-east` become `HomeEast`, with a leading `_` before a digit.
pub fn pascal(name: &str) -> String {
    let mut out = String::new();
    digit_guard(name, &mut out);
    for word in words(name) {
        out.push_str(&capitalize(&word));
    }
    out
}

/// `homeEast` and `home-east` become `homeEast`, with a leading `_` before a digit.
pub fn camel(name: &str) -> String {
    let mut out = String::new();
    digit_guard(name, &mut out);
    for (index, word) in words(name).iter().enumerate() {
        if index == 0 {
            out.push_str(word);
        } else {
            out.push_str(&capitalize(word));
        }
    }
    out
}

/// `homeEast` and `home-east` become `home_east`, with a leading `_` before a digit.
pub fn snake(name: &str) -> String {
    let mut out = String::new();
    digit_guard(name, &mut out);
    out.push_str(&words(name).join("_"));
    out
}

#[cfg(test)]
mod tests {
    use super::{camel, digit_guard, is_plain_ident, pascal, snake};

    #[test]
    fn digit_guard_prefixes_only_a_leading_digit() {
        let mut out = String::new();
        digit_guard("9lives", &mut out);
        assert_eq!(out, "_");
        out.clear();
        digit_guard("lives9", &mut out);
        assert_eq!(out, "");
        out.clear();
        digit_guard("", &mut out);
        assert_eq!(out, "");
    }

    #[test]
    fn plain_ident_accepts_letters_digits_and_underscores() {
        assert!(is_plain_ident("door_2", &[]));
        assert!(is_plain_ident("_x", &[]));
        assert!(!is_plain_ident("", &[]));
        assert!(!is_plain_ident("2door", &[]));
        assert!(!is_plain_ident("a-b", &[]));
        assert!(!is_plain_ident("é", &[]));
    }

    #[test]
    fn plain_ident_refuses_listed_keywords() {
        assert!(!is_plain_ident("type", &["type"]));
        assert!(is_plain_ident("types", &["type"]));
    }

    #[test]
    fn converters_split_on_dashes_underscores_and_case_changes() {
        assert_eq!(pascal("home-east"), "HomeEast");
        assert_eq!(pascal("homeEast"), "HomeEast");
        assert_eq!(pascal("home_east"), "HomeEast");
        assert_eq!(camel("home-east"), "homeEast");
        assert_eq!(camel("HomeEast"), "homeEast");
        assert_eq!(snake("homeEast"), "home_east");
        assert_eq!(snake("Home-East"), "home_east");
    }

    #[test]
    fn converters_guard_a_leading_digit_and_skip_empty_parts() {
        assert_eq!(pascal("9lives"), "_9lives");
        assert_eq!(camel("--a__b--"), "aB");
        assert_eq!(snake("a--b"), "a_b");
        assert_eq!(pascal(""), "");
        assert_eq!(camel("-"), "");
    }
}
