//! What a parse produces: the entries, plus every byte it decided not to model.

use crate::model::Entry;

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

/// Every entry in one file, in file order.
#[derive(Debug, Clone, Default)]
pub struct Library {
    source: String,
    entries: Vec<Entry>,
    spans: Vec<Span>,
    skipped: Vec<Span>,
}

impl Library {
    /// Builds a library from the parts a parser collected.
    pub(crate) fn new(
        source: String,
        entries: Vec<Entry>,
        spans: Vec<Span>,
        skipped: Vec<Span>,
    ) -> Self {
        Self {
            source,
            entries,
            spans,
            skipped,
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

    /// How many regular entries the file held.
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
        self.spans.get(id.0).copied()
    }

    /// The source text of one entry, byte for byte.
    pub fn source_of(&self, id: EntryId) -> Option<&str> {
        let span = self.span(id)?;
        self.source.get(span.start..span.end)
    }

    /// Blocks that are not regular entries: `@comment`, `@preamble`, `@string`.
    pub fn skipped_spans(&self) -> &[Span] {
        &self.skipped
    }

    /// The entry whose citation key matches, compared exactly.
    pub fn find_by_key(&self, key: &str) -> Option<EntryId> {
        self.entries
            .iter()
            .position(|e| e.key.as_ref() == key)
            .map(EntryId)
    }
}
