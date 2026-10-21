//! A writer that reproduces the input byte for byte when nothing was edited. The
//! default copies the source; a caller that wants a reformatted file asks for it
//! explicitly.

use std::fmt::Write as _;

use crate::library::Library;

/// How much of the original formatting a rewrite keeps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WriteOptions {
    /// Copy the source, and any replaced entry in its new form, at its old offset.
    #[default]
    Preserve,
    /// Emit entries one after another with a fixed indent, discarding the rest.
    Canonical,
}

/// Writes a library back out.
pub fn write_str(library: &Library, options: &WriteOptions) -> String {
    match options {
        WriteOptions::Preserve => preserve(library),
        WriteOptions::Canonical => canonical(library),
    }
}

/// The source with each edited entry replaced by the bytes its edit left, and
/// everything else — an untouched entry, a comment, the blank lines — copied, so
/// a save changes only what was typed.
fn preserve(library: &Library) -> String {
    let replaced = library.replacements();
    if replaced.is_empty() {
        return library.source().to_owned();
    }
    let source = library.source();
    let mut out = String::with_capacity(source.len());
    let mut copied = 0usize;
    for (span, text) in replaced {
        out.push_str(&source[copied..span.start]);
        out.push_str(text);
        copied = span.end;
    }
    out.push_str(&source[copied..]);
    out
}

fn canonical(library: &Library) -> String {
    let mut out = String::with_capacity(library.source().len());
    for (row, entry) in library.entries().iter().enumerate() {
        let _ = writeln!(out, "@{}{{{},", entry.kind, entry.key);
        for (field, value) in entry
            .fields
            .iter()
            .zip(library.values(crate::library::EntryId(row)))
        {
            match field.delimiter {
                crate::model::Delimiter::Braces => {
                    let _ = writeln!(out, "  {} = {{{}}},", field.name_raw, value);
                }
                crate::model::Delimiter::Quotes => {
                    let _ = writeln!(out, "  {} = \"{}\",", field.name_raw, value);
                }
                crate::model::Delimiter::Bare => {
                    let _ = writeln!(out, "  {} = {},", field.name_raw, value);
                }
            }
        }
        out.push_str("}\n\n");
    }
    out
}
