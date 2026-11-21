//! One language's lexical rules, shared by every front end that writes that language.

pub mod rust_keywords;

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

#[cfg(test)]
mod tests {
    use super::{digit_guard, is_plain_ident};

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
}
