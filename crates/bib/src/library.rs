//! What a parse produces: the entries, plus every byte it decided not to model.
//! An edit is recorded as the replacement text of whole entries, since an entry is
//! the unit a save writes and an entry is at most a few kilobytes.

use crate::edit::{Layout, is_key_end, new_field, render, usable};
use crate::model::Entry;
use crate::parse::{ParseOptions, parse_str};

/// A stable handle to one entry, valid for the library that produced it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EntryId(pub usize);

/// A span of the source text, as byte offsets into the original input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    /// First byte of the span.
    pub start: usize,
    /// One past the last byte of the span.
    pub end: usize,
}

/// Why an edit was refused. Every one of these leaves the library untouched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditError {
    /// No such entry or field.
    Absent,
    /// The entry was added rather than read, so it has no bytes to edit in place.
    NotInFile,
    /// The value cannot be written inside the delimiters it is held in.
    Unwritable,
    /// A citation key may be neither empty nor hold a byte an entry cannot carry.
    BadKey,
    /// A field name is empty or holds a byte a name cannot carry.
    BadName,
}

impl EditError {
    /// A short English phrase for a front end to put beside the box that failed.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Absent => "that field is not in this entry",
            Self::NotInFile => "this entry is not in the file",
            Self::Unwritable => "the text cannot be written as it stands",
            Self::BadKey => "a key cannot be empty or hold braces, commas or spaces",
            Self::BadName => "a name holds letters, digits, and _ - : + . /",
        }
    }
}

impl std::fmt::Display for EditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::error::Error for EditError {}

/// Every entry in one file, in file order, with the edits made since it opened.
#[derive(Debug, Clone, Default)]
pub struct Library {
    source: String,
    entries: Vec<Entry>,
    spans: Vec<Option<Span>>,
    skipped: Vec<Span>,
    layouts: Vec<Option<Layout>>,
    /// The text each edited entry is written as, indexed like `entries`.
    changed: Vec<Option<Box<str>>>,
    edited: usize,
}

impl Library {
    /// Builds a library from the parts a parser collected.
    pub(crate) fn new(
        source: String,
        entries: Vec<Entry>,
        spans: Vec<Span>,
        skipped: Vec<Span>,
    ) -> Self {
        let layouts = spans
            .iter()
            .map(|span| source.get(span.start..span.end).and_then(Layout::of))
            .collect();
        let len = entries.len();
        Self {
            source,
            entries,
            spans: spans.into_iter().map(Some).collect(),
            skipped,
            layouts,
            changed: vec![None; len],
            edited: 0,
        }
    }

    /// The text this library was parsed from, kept so a rewrite is lossless.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Every regular entry, in file order.
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// How many regular entries the file holds.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the file held no regular entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The entry behind an id, or `None` if the id came from another library.
    pub fn entry(&self, id: EntryId) -> Option<&Entry> {
        self.entries.get(id.0)
    }

    /// The byte range an entry occupied, which is what an editor scrolls to.
    pub fn span(&self, id: EntryId) -> Option<Span> {
        self.spans.get(id.0).copied().flatten()
    }

    /// The source text of one entry, byte for byte.
    pub fn source_of(&self, id: EntryId) -> Option<&str> {
        let span = self.span(id)?;
        self.source.get(span.start..span.end)
    }

    /// Blocks the parser passed over: `@comment`, `@preamble`, `@string`.
    pub fn skipped_spans(&self) -> &[Span] {
        &self.skipped
    }

    /// Whether any entry has been edited since the file was parsed.
    pub fn is_edited(&self) -> bool {
        self.edited > 0
    }

    /// How many entries were edited.
    pub fn edited_count(&self) -> usize {
        self.edited
    }

    /// The entry whose citation key matches, compared exactly.
    pub fn find_by_key(&self, key: &str) -> Option<EntryId> {
        self.entries
            .iter()
            .position(|e| e.key.as_ref() == key)
            .map(EntryId)
    }

    /// The value of one field as the file writes it now, edited or not.
    pub fn field(&self, id: EntryId, field: usize) -> Option<&str> {
        if self.is_changed(id) {
            return Some(&self.entries.get(id.0)?.fields.get(field)?.value);
        }
        // An unedited value is the file's own bytes, which is what a save writes.
        let entry = self.entries.get(id.0)?;
        let at = self.layouts.get(id.0)?.as_ref()?.fields.get(field);
        match self.source_of(id).zip(at) {
            Some((text, at)) => text.get(at.value.start..at.value.end),
            None => Some(&entry.fields.get(field)?.value),
        }
    }

    /// Every field value of one entry as the file writes it now.
    pub fn values(&self, id: EntryId) -> Vec<&str> {
        (0..self.entries.get(id.0).map_or(0, |e| e.fields.len()))
            .filter_map(|field| self.field(id, field))
            .collect()
    }

    /// Replaces the value of one field, at the delimiters it already has.
    pub fn set_field(&mut self, id: EntryId, field: usize, value: &str) -> Result<(), EditError> {
        let current = self.entries.get(id.0).ok_or(EditError::Absent)?;
        current.fields.get(field).ok_or(EditError::Absent)?;
        let mut next = current.clone();
        next.fields[field].value = value.into();
        self.commit(id, next)
    }

    /// Replaces the value of the first field with this name.
    pub fn set_field_named(
        &mut self,
        id: EntryId,
        name: &str,
        value: &str,
    ) -> Result<(), EditError> {
        let field = self
            .entries
            .get(id.0)
            .and_then(|e| e.fields.iter().position(|f| f.name.as_str() == name))
            .ok_or(EditError::Absent)?;
        self.set_field(id, field, value)
    }

    /// Adds a field at the end of an entry.
    pub fn add_field(&mut self, id: EntryId, name: &str, value: &str) -> Result<(), EditError> {
        let current = self.entries.get(id.0).ok_or(EditError::Absent)?;
        if current
            .fields
            .iter()
            .any(|f| f.name.as_str().eq_ignore_ascii_case(name))
        {
            return Err(EditError::Absent);
        }
        if !usable(name, value) {
            return Err(EditError::BadName);
        }
        let mut next = current.clone();
        next.fields.push(new_field(name, value));
        self.commit(id, next)
    }

    /// Renames an entry's citation key, keeping its type and every field.
    pub fn set_key(&mut self, id: EntryId, key: &str) -> Result<(), EditError> {
        let current = self.entries.get(id.0).ok_or(EditError::Absent)?;
        if key.is_empty() || key.bytes().any(is_key_end) {
            return Err(EditError::BadKey);
        }
        let mut next = current.clone();
        next.key = key.into();
        self.commit(id, next)
    }

    /// Puts one entry back the way the file had it.
    pub fn revert(&mut self, id: EntryId) -> Result<(), EditError> {
        self.spans.get(id.0).ok_or(EditError::Absent)?;
        let Some(Some(text)) = self.changed.get(id.0) else {
            return Ok(());
        };
        let restored = parse_str(text, &ParseOptions::default())
            .ok()
            .and_then(|lib| lib.entries().first().cloned())
            .ok_or(EditError::NotInFile)?;
        self.changed[id.0] = None;
        self.entries[id.0] = restored;
        self.edited -= 1;
        Ok(())
    }

    /// Writes `next` in place of the entry's model, and records the bytes a save
    /// has to write. A value that cannot be written is refused before anything
    /// changes, so a refused edit never leaves the file's text and model apart.
    fn commit(&mut self, id: EntryId, next: Entry) -> Result<(), EditError> {
        self.entries.get(id.0).ok_or(EditError::Absent)?;
        let text = match self.span(id) {
            Some(span) => {
                let layout = match self.layouts.get(id.0) {
                    Some(Some(layout)) => layout,
                    _ => return Err(EditError::NotInFile),
                };
                layout
                    .rebuild(&self.source[span.start..span.end], &next)
                    .ok_or(EditError::Unwritable)?
            }
            None => render(&next).ok_or(EditError::Unwritable)?,
        };
        if self.changed[id.0].is_none() {
            self.edited += 1;
        }
        self.changed[id.0] = Some(text.into_boxed_str());
        self.entries[id.0] = next;
        Ok(())
    }

    /// The spans a save replaces, each with the text that goes in its place, in
    /// the order they sit in the source.
    pub(crate) fn replacements(&self) -> Vec<(Span, &str)> {
        self.changed
            .iter()
            .enumerate()
            .filter_map(|(row, text)| {
                let text = text.as_deref()?;
                Some((self.spans.get(row).copied().flatten()?, text))
            })
            .collect()
    }

    /// Whether this entry's text differs from the source.
    fn is_changed(&self, id: EntryId) -> bool {
        self.changed.get(id.0).is_some_and(Option::is_some)
    }
}
