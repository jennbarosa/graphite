//! Writers shared by the index and chunk modules.

use std::fmt::Display;
use std::io::{self, Write};

use crate::frontend::layout::AnnotationRow;
use crate::interner::StringId;

use super::{EXTENSION, RUNTIME_FILE, escape::string};

/// Writes `import <clause> from "./<runtime>";` and a newline, where `clause` is the import
/// clause, such as `{ defineGraph }`. The path is derived from `RUNTIME_FILE`, so renaming the
/// runtime file cannot leave the imports behind.
pub(super) fn runtime_import(clause: &str, output: &mut dyn Write) -> io::Result<()> {
    let module = RUNTIME_FILE
        .strip_suffix(&format!(".{EXTENSION}"))
        .expect("the runtime file name ends with the module extension");
    write!(output, "import {clause} from ")?;
    string(&format!("./{module}"), output)?;
    output.write_all(b";\n")
}

/// Writes `value`, or `null` when there is none.
pub(super) fn optional(value: Option<impl Display>, output: &mut dyn Write) -> io::Result<()> {
    match value {
        Some(value) => write!(output, "{value}"),
        None => output.write_all(b"null"),
    }
}

/// Writes one entry of a chunk's `annotations` table.
pub(super) fn annotation_row(entry: &AnnotationRow, output: &mut dyn Write) -> io::Result<()> {
    write!(output, "{{key:{},value:", entry.key.index())?;
    optional(entry.value.map(StringId::index), output)?;
    output.write_all(b"},\n")
}
