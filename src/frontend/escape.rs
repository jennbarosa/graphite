//! The quoted-string loop shared by every front end. A language supplies only its per-character
//! rule; the loop copies unescaped runs in borrowed chunks and writes each escape in place.

use std::io::{self, Write};

/// What a language does with one character of a string literal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Escape {
    /// Copy the character unchanged.
    Keep,
    /// Write this text in its place, such as `\"`.
    Text(&'static str),
    /// Write a hex escape: `prefix`, then the code point in lower-case hex, zero-padded to
    /// `width` digits. TypeScript uses `\u` with 4 digits; PureScript uses `\x` with 6.
    Hex { prefix: &'static str, width: usize },
}

/// Writes `value` as a double-quoted literal, with each character escaped by `rule`.
pub fn quoted(
    value: &str,
    output: &mut dyn Write,
    rule: impl Fn(char) -> Escape,
) -> io::Result<()> {
    output.write_all(b"\"")?;
    let mut start = 0;
    for (index, ch) in value.char_indices() {
        let escape = match rule(ch) {
            Escape::Keep => continue,
            escape => escape,
        };
        output.write_all(&value.as_bytes()[start..index])?;
        match escape {
            Escape::Text(text) => output.write_all(text.as_bytes())?,
            Escape::Hex { prefix, width } => {
                write!(output, "{prefix}{:0width$x}", ch as u32, width = width)?
            }
            Escape::Keep => unreachable!("kept characters never reach the writer"),
        }
        start = index + ch.len_utf8();
    }
    output.write_all(&value.as_bytes()[start..])?;
    output.write_all(b"\"")
}

#[cfg(test)]
mod tests {
    use super::{Escape, quoted};

    /// A toy rule: quote, backslash, and control characters.
    fn toy(ch: char) -> Escape {
        match ch {
            '"' => Escape::Text("\\\""),
            '\\' => Escape::Text("\\\\"),
            c if c < ' ' => Escape::Hex {
                prefix: "\\x",
                width: 2,
            },
            _ => Escape::Keep,
        }
    }

    fn render(value: &str) -> String {
        let mut out = Vec::new();
        quoted(value, &mut out, toy).unwrap();
        String::from_utf8(out).unwrap()
    }

    #[test]
    fn plain_text_is_one_borrowed_run_between_quotes() {
        assert_eq!(render("abc é中"), "\"abc é中\"");
        assert_eq!(render(""), "\"\"");
    }

    #[test]
    fn escapes_at_the_start_middle_and_end_are_written_in_place() {
        assert_eq!(render("\"a\""), "\"\\\"a\\\"\"");
        assert_eq!(render("x\\y"), "\"x\\\\y\"");
    }

    #[test]
    fn multibyte_text_around_an_escape_keeps_its_bytes() {
        assert_eq!(render("é\"中"), "\"é\\\"中\"");
    }

    #[test]
    fn hex_escapes_pad_to_the_width_in_lower_case() {
        assert_eq!(render("\u{1f}a"), "\"\\x1fa\"");
        fn wide(_: char) -> Escape {
            Escape::Hex {
                prefix: "\\x",
                width: 6,
            }
        }
        let mut out = Vec::new();
        quoted("\u{1f}", &mut out, wide).unwrap();
        assert_eq!(String::from_utf8(out).unwrap(), "\"\\x00001f\"");
    }
}
