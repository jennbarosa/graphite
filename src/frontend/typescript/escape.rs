use std::io::{self, Write};

use crate::frontend::escape::{Escape, quoted};

/// Write ordinary UTF-8 in borrowed chunks, escaping only JS string delimiters.
pub(super) fn string(value: &str, output: &mut dyn Write) -> io::Result<()> {
    quoted(value, output, rule)
}

/// The JavaScript rule: the delimiters, backslash, the two line separators, and every C0 control.
fn rule(ch: char) -> Escape {
    match ch {
        '"' => Escape::Text("\\\""),
        '\\' => Escape::Text("\\\\"),
        '\u{2028}' => Escape::Text("\\u2028"),
        '\u{2029}' => Escape::Text("\\u2029"),
        ch if ch <= '\u{001f}' => Escape::Hex {
            prefix: "\\u",
            width: 4,
        },
        _ => Escape::Keep,
    }
}

#[cfg(test)]
mod tests {
    use super::string;

    fn escaped(value: &str) -> String {
        let mut out = Vec::new();
        string(value, &mut out).unwrap();
        String::from_utf8(out).unwrap()
    }

    #[test]
    fn every_c0_control_becomes_a_lowercase_unicode_escape() {
        for code in 0u32..0x20 {
            let ch = char::from_u32(code).unwrap();
            assert_eq!(escaped(&ch.to_string()), format!("\"\\u{code:04x}\""));
        }
    }

    #[test]
    fn printable_and_non_ascii_text_is_copied_unchanged() {
        assert_eq!(escaped("é_中-2 \u{7f}"), "\"é_中-2 \u{7f}\"");
        assert_eq!(escaped(""), "\"\"");
    }
}
