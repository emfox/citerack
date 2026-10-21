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
        WriteOptions::Preserve => library.source().to_owned(),
        WriteOptions::Canonical => canonical(library),
    }
}

fn canonical(library: &Library) -> String {
    let mut out = String::with_capacity(library.source().len());
    for entry in library.entries() {
        let _ = writeln!(out, "@{}{{{},", entry.kind, entry.key);
        for field in &entry.fields {
            match field.delimiter {
                crate::model::Delimiter::Braces => {
                    let _ = writeln!(out, "  {} = {{{}}},", field.name_raw, field.value);
                }
                crate::model::Delimiter::Quotes => {
                    let _ = writeln!(out, "  {} = \"{}\",", field.name_raw, field.value);
                }
                crate::model::Delimiter::Bare => {
                    let _ = writeln!(out, "  {} = {},", field.name_raw, field.value);
                }
            }
        }
        out.push_str("}\n\n");
    }
    out
}
