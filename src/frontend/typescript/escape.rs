use std::io::{self, Write};

/// Write ordinary UTF-8 in borrowed chunks, escaping only JS string delimiters.
pub(super) fn string(value: &str, output: &mut dyn Write) -> io::Result<()> {
    output.write_all(b"\"")?;
    let mut start = 0;
    for (index, ch) in value.char_indices() {
        let escaped = match ch {
            '"' => Some("\\\""),
            '\\' => Some("\\\\"),
            '\u{2028}' => Some("\\u2028"),
            '\u{2029}' => Some("\\u2029"),
            _ => None,
        };
        if escaped.is_none() && ch > '\u{001f}' {
            continue;
        }
        output.write_all(&value.as_bytes()[start..index])?;
        if let Some(escaped) = escaped {
            output.write_all(escaped.as_bytes())?;
        } else {
            write!(output, "\\u{:04x}", ch as u32)?;
        }
        start = index + ch.len_utf8();
    }
    output.write_all(&value.as_bytes()[start..])?;
    output.write_all(b"\"")
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
