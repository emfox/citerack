//! The entry and field shapes, and the citation key each entry is known by.

use std::fmt;

/// An interned-in-spirit field name: lowercased ASCII, stored once per library.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FieldName(Box<str>);

impl FieldName {
    /// Normalises to lowercase, which is how BibTeX compares field names.
    pub fn new(raw: &str) -> Self {
        Self(raw.to_ascii_lowercase().into_boxed_str())
    }

    /// The normalised name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for FieldName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// One field exactly as the file spelled it, plus its value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    /// The name in the case the file used, so a rewrite reproduces it.
    pub name_raw: Box<str>,
    /// The name normalised for lookup.
    pub name: FieldName,
    /// The value with outer delimiters already removed.
    pub value: Box<str>,
    /// Whether the value was braced, quoted, bare or a number.
    pub delimiter: Delimiter,
}

/// How a field value was written, which a rewrite has to reproduce.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delimiter {
    /// `{value}`
    Braces,
    /// `"value"`
    Quotes,
    /// `value` with no delimiter, legal only for a number or a macro name.
    Bare,
}

/// The entry type, kept in the case the file used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryKind(Box<str>);

impl EntryKind {
    /// Stores the spelling as given; BibTeX itself compares these case-insensitively.
    pub fn new(raw: &str) -> Self {
        Self(raw.into())
    }

    /// The spelling as it appeared in the file.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Whether this is one of the regular entry types, as opposed to `@comment`,
    /// `@preamble` or `@string`.
    pub fn is_regular(&self) -> bool {
        !matches!(
            self.0.to_ascii_lowercase().as_str(),
            "comment" | "preamble" | "string"
        )
    }
}

impl fmt::Display for EntryKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A regular entry: a type, a citation key, and its fields in file order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The type as spelled, for example `Article`.
    pub kind: EntryKind,
    /// The citation key, which may legitimately be empty in a malformed file.
    pub key: Box<str>,
    /// Fields in the order the file listed them, which a rewrite preserves.
    pub fields: Vec<Field>,
}

impl Entry {
    /// The value of the first field with this name, compared case-insensitively.
    pub fn field(&self, name: &str) -> Option<&str> {
        self.fields
            .iter()
            .find(|f| f.name.as_str().eq_ignore_ascii_case(name))
            .map(|f| f.value.as_ref())
    }
}
