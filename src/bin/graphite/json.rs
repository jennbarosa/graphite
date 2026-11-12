//! JSON text for `--json` output. The commands write only strings, booleans, `null`, arrays and
//! objects, so a few helpers replace a serialization dependency.

use std::fmt::Write;

/// A JSON string literal: quoted, with quotes, backslashes and control characters escaped.
pub fn quote(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// A JSON string, or `null` for `None`.
pub fn optional(text: Option<&str>) -> String {
    text.map_or_else(|| "null".to_string(), quote)
}

/// An object from `(key, value)` pairs. Each value is already rendered JSON.
pub fn object(fields: &[(&str, String)]) -> String {
    let body: Vec<String> = fields
        .iter()
        .map(|(key, value)| format!("{}:{value}", quote(key)))
        .collect();
    format!("{{{}}}", body.join(","))
}

/// An array from items that are already rendered JSON.
pub fn array(items: &[String]) -> String {
    format!("[{}]", items.join(","))
}

#[cfg(test)]
mod tests {
    use super::quote;

    #[test]
    fn escapes_what_json_requires() {
        assert_eq!(quote("a\"b\\c\n\u{1}é"), "\"a\\\"b\\\\c\\n\\u0001é\"");
    }
}
