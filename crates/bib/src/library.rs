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
    /// Whether each entry was edited since the last save, indexed like `entries`.
    unsaved: Vec<bool>,
    /// The model each entry had when the file was last written, indexed like
    /// `entries`, so an undo can say whether it landed back on the saved text.
    saved: Vec<Option<Entry>>,
    /// The state of every entry the pending change touches, captured before the
    /// first of its edits. An asker reads this, or `changed` when it is empty.
    before: Vec<(usize, Option<Box<str>>, Entry)>,
}

/// What one entry looked like before the pending change, for an undo record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryState {
    /// Which entry.
    pub entry: usize,
    /// The bytes a save would have written for it, or `None` if it was untouched.
    pub text: Option<Box<str>>,
    /// The model it had.
    pub model: Entry,
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
        // A file just opened is what its bytes say, so its entries start out as the
        // saved ones and an edit back to them leaves nothing to write.
        let saved = entries.iter().cloned().map(Some).collect();
        let len = entries.len();
        Self {
            source,
            entries,
            spans: spans.into_iter().map(Some).collect(),
            skipped,
            layouts,
            changed: vec![None; len],
            unsaved: vec![false; len],
            saved,
            before: Vec::new(),
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

    /// Whether any entry has been edited since the last save, which is what the
    /// window calls unsaved work.
    pub fn is_edited(&self) -> bool {
        self.unsaved.iter().any(|&unsaved| unsaved)
    }

    /// How many entries are unsaved.
    pub fn edited_count(&self) -> usize {
        self.unsaved.iter().filter(|&&unsaved| unsaved).count()
    }

    /// Forgets that anything is unsaved, which a save does once the bytes are on
    /// disk: the model becomes what the file holds and what a later undo is measured
    /// against, and the replacements stay since they say how each model differs.
    pub fn mark_saved(&mut self) {
        self.unsaved.fill(false);
        for (at, entry) in self.entries.iter().enumerate() {
            self.saved[at] = Some(entry.clone());
        }
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
        self.replace_field(id, field, value).map(drop)
    }

    /// As `set_field`, and says whether the file changed, since typing a value
    /// back the way it was is not an edit an undo has to hold.
    pub fn replace_field(
        &mut self,
        id: EntryId,
        field: usize,
        value: &str,
    ) -> Result<bool, EditError> {
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
        self.append_field(id, name, value).map(drop)
    }

    /// As `add_field`, and says whether the file changed.
    pub fn append_field(
        &mut self,
        id: EntryId,
        name: &str,
        value: &str,
    ) -> Result<bool, EditError> {
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

    /// The entries whose citation key is `key`, compared exactly, so a caller can
    /// tell whether a key it is about to write would name a second entry too.
    pub fn keyed(&self, key: &str) -> usize {
        self.entries
            .iter()
            .filter(|e| e.key.as_ref() == key)
            .count()
    }

    /// Renames an entry's citation key, keeping its type and every field.
    pub fn set_key(&mut self, id: EntryId, key: &str) -> Result<(), EditError> {
        self.rename_key(id, key).map(drop)
    }

    /// As `set_key`, and says whether the file changed.
    pub fn rename_key(&mut self, id: EntryId, key: &str) -> Result<bool, EditError> {
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
        // What was before is the entry as the change left it, not as the file had
        // it, since a revert is one of the edits a change can hold.
        if !self.before.iter().any(|(row, ..)| *row == id.0) {
            self.before
                .push((id.0, Some(text.clone()), self.entries[id.0].clone()));
        }
        self.changed[id.0] = None;
        self.entries[id.0] = restored;
        self.unsaved[id.0] = false;
        Ok(())
    }

    /// Writes `next` in place of the entry's model, and records the bytes a save
    /// has to write. A value that cannot be written is refused before anything
    /// changes, so a refused edit never leaves the file's text and model apart.
    fn commit(&mut self, id: EntryId, next: Entry) -> Result<bool, EditError> {
        let current = self.entries.get(id.0).cloned().ok_or(EditError::Absent)?;
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
        if self.entries[id.0] == next {
            // The entry already says this, so there is nothing to undo either.
            return Ok(false);
        }
        if !self.before.iter().any(|(row, ..)| *row == id.0) {
            let was = self.changed[id.0].clone();
            self.before.push((id.0, was, current));
        }
        // Typing a value back to what the file holds leaves nothing to save, so the
        // entry is marked as the file has it rather than as edited.
        let back = self.saved[id.0].as_ref() == Some(&next);
        self.changed[id.0] = (!back).then(|| text.into_boxed_str());
        self.unsaved[id.0] = !back;
        self.entries[id.0] = next;
        Ok(true)
    }

    /// The state of each entry the pending change touched, as each was before the
    /// change. Empty when the previous edit was already asked about or committed.
    pub fn pending_before(&self) -> Vec<EntryState> {
        self.before
            .iter()
            .map(|(entry, text, model)| EntryState {
                entry: *entry,
                text: text.clone(),
                model: model.clone(),
            })
            .collect()
    }

    /// The state of each entry that differs from the file, which is what a record
    /// of an undo or a redo needs to hold.
    pub fn current_states(&self) -> Vec<EntryState> {
        self.changed
            .iter()
            .enumerate()
            .filter(|(_, text)| text.is_some())
            .filter_map(|(entry, text)| {
                Some(EntryState {
                    entry,
                    text: text.clone(),
                    model: self.entries.get(entry)?.clone(),
                })
            })
            .collect()
    }

    /// The state of one entry now, changed or not, so a change can record how it
    /// left the rows it touched rather than every row the file has seen.
    pub fn state_of(&self, entry: usize) -> Option<EntryState> {
        Some(EntryState {
            entry,
            text: self.changed.get(entry).cloned().flatten(),
            model: self.entries.get(entry)?.clone(),
        })
    }

    /// Ends one change, so the next edit starts a record of its own, and hands
    /// back what the change recorded.
    pub fn seal(&mut self) -> Vec<EntryState> {
        let recorded = self.pending_before();
        self.before.clear();
        recorded
    }

    /// Puts entries back to the states recorded, which is what undo and redo do.
    /// Every entry is checked first, so a restore is never left half done.
    pub fn restore(&mut self, states: &[EntryState]) -> Result<(), EditError> {
        for state in states {
            self.spans.get(state.entry).ok_or(EditError::Absent)?;
            if let Some(text) = &state.text {
                self.fits(state.entry, text)?;
            }
        }
        for state in states {
            if !self.before.iter().any(|(row, ..)| *row == state.entry) {
                let was = self.changed[state.entry].clone();
                self.before
                    .push((state.entry, was, self.entries[state.entry].clone()));
            }
            self.entries[state.entry] = state.model.clone();
            // What matters is whether this left the entry as the file has it, not
            // whether a step was recorded for it: an undo back over a save lands on
            // text the file does hold, and an undo of a later step does not.
            let back = self.saved[state.entry].as_ref() == Some(&state.model);
            if back {
                self.changed[state.entry] = None;
                self.unsaved[state.entry] = false;
            } else {
                let text = match &state.text {
                    Some(text) => Some(text.clone()),
                    None => self.rewritten(state.entry),
                };
                self.changed[state.entry] = text;
                self.unsaved[state.entry] = true;
            }
        }
        Ok(())
    }

    /// The bytes a save would write for one entry, rendered from its model, for a
    /// state that was recorded before the entry had any bytes of its own.
    fn rewritten(&self, entry: usize) -> Option<Box<str>> {
        let model = self.entries.get(entry)?;
        let span = self.spans.get(entry).copied().flatten();
        match span.and_then(|span| {
            let layout = self.layouts.get(entry)?.as_ref()?;
            layout.rebuild(&self.source[span.start..span.end], model)
        }) {
            Some(text) => Some(text.into_boxed_str()),
            None => render(model).map(String::into_boxed_str),
        }
    }

    /// Refuses bytes that would not read back as the entry they stand for.
    fn fits(&self, entry: usize, text: &str) -> Result<(), EditError> {
        let span = self.spans.get(entry).copied().flatten();
        let Some(Some(layout)) = self.layouts.get(entry) else {
            return Ok(());
        };
        let Some(span) = span else { return Ok(()) };
        layout
            .rebuild(&self.source[span.start..span.end], &layout_model(text)?)
            .map(drop)
            .ok_or(EditError::Unwritable)
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

    /// Whether this entry has been written since the file was read, whether or
    /// not the writing changed anything.
    fn is_changed(&self, id: EntryId) -> bool {
        self.changed.get(id.0).is_some_and(Option::is_some)
    }
}

/// The entry a replacement text stands for, read back so a restore can be checked
/// against the delimiters the file already has.
fn layout_model(text: &str) -> Result<Entry, EditError> {
    parse_str(text, &ParseOptions::default())
        .ok()
        .and_then(|library| library.entries().first().cloned())
        .ok_or(EditError::NotInFile)
}
