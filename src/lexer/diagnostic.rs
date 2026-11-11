use std::borrow::Cow;
use std::fmt;

/// An error shown with its source line and a caret under the offending text.
/// Built only when formatted, so the success path never pays for it.
pub struct Diagnostic<'s> {
    message: Cow<'static, str>,
    offset: usize,
    source: &'s str,
}

impl<'s> Diagnostic<'s> {
    pub(crate) fn new(
        message: impl Into<Cow<'static, str>>,
        offset: usize,
        source: &'s str,
    ) -> Self {
        Self {
            message: message.into(),
            offset,
            source,
        }
    }
}

impl fmt::Display for Diagnostic<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Offsets from the lexer and parser sit on ASCII bytes or line starts, so slicing is safe.
        let source = self.source;
        let offset = self.offset.min(source.len());
        let line_start = source[..offset].rfind('\n').map_or(0, |index| index + 1);
        let line_end = source[offset..]
            .find('\n')
            .map_or(source.len(), |index| offset + index);
        let line = source[line_start..line_end].trim_end_matches('\r');
        let line_number = source[..line_start].matches('\n').count() + 1;
        let column = source[line_start..offset].chars().count() + 1;

        // Skip indentation so the caret lands on the token itself, then cover its run.
        let bytes = line.as_bytes();
        let mut begin = offset - line_start;
        while bytes.get(begin) == Some(&b' ') {
            begin += 1;
        }
        let mut end = begin;
        while bytes
            .get(end)
            .is_some_and(|byte| !matches!(byte, b' ' | b'\t'))
        {
            end += 1;
        }
        let lead = line.get(..begin).map_or(0, |text| text.chars().count());
        let carets = line
            .get(begin..end)
            .map_or(0, |text| text.chars().count())
            .max(1);

        let width = line_number.to_string().len();
        let pad = " ".repeat(width);
        writeln!(formatter, "error: {}", self.message)?;
        writeln!(formatter, "{pad}--> line {line_number}, column {column}")?;
        writeln!(formatter, "{pad} |")?;
        // Tabs are shown as one space so the caret column matches the character count.
        writeln!(formatter, "{line_number} | {}", line.replace('\t', " "))?;
        write!(
            formatter,
            "{pad} | {}{} {}",
            " ".repeat(lead),
            "^".repeat(carets),
            self.message
        )
    }
}
