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
    let mut out = String::with_capacity(library.source().len());
    if replaced.is_empty() {
        out.push_str(library.source());
    } else {
        let source = library.source();
        let mut copied = 0usize;
        for (span, text) in replaced {
            out.push_str(&source[copied..span.start]);
            out.push_str(text);
            copied = span.end;
        }
        out.push_str(&source[copied..]);
    }
    added(library, &mut out);
    out
}

/// Writes the entries `append` added, each on its own lines at the end of the file,
/// which is where a reader looks for one they imported and cannot lose another.
fn added(library: &Library, out: &mut String) {
    for row in library.added_rows() {
        let Some(entry) = library.entries().get(row) else {
            continue;
        };
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        let _ = writeln!(out, "@{}{{{},", entry.kind, entry.key);
        for field in &entry.fields {
            let _ = writeln!(out, "  {} = {},", field.name_raw, write_value(field));
        }
        out.push_str("}\n");
    }
}

/// A field's value inside the delimiters it is held in.
fn write_value(field: &crate::model::Field) -> String {
    match field.delimiter {
        crate::model::Delimiter::Braces => format!("{{{}}}", field.value),
        crate::model::Delimiter::Quotes => format!("\"{}\"", field.value),
        crate::model::Delimiter::Bare => field.value.to_string(),
    }
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
