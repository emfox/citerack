//! The table's columns, and how a cell is rendered from a library row.

use citerack_bib::Entry;
use citerack_store::Session;

/// A column the table can show, sized the way a reader scans a bibliography.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Column {
    /// The citation key, which is what a LaTeX author types.
    Key,
    /// The entry type.
    Kind,
    /// The first author's surname.
    Author,
    /// The title.
    Title,
    /// The year.
    Year,
    /// The journal, book or proceedings title.
    Venue,
}

impl Column {
    /// Every column, in the order the table lays them out.
    pub const ALL: [Self; 6] = [
        Self::Key,
        Self::Kind,
        Self::Author,
        Self::Title,
        Self::Year,
        Self::Venue,
    ];

    /// The header text.
    pub fn header(self) -> &'static str {
        match self {
            Self::Key => "Key",
            Self::Kind => "Type",
            Self::Author => "Author",
            Self::Title => "Title",
            Self::Year => "Year",
            Self::Venue => "Journal / Book",
        }
    }

    /// The starting width in points, wide enough for a typical value.
    pub fn initial_width(self) -> f32 {
        match self {
            Self::Key => 150.0,
            Self::Kind => 80.0,
            Self::Author => 150.0,
            Self::Title => 380.0,
            Self::Year => 60.0,
            Self::Venue => 200.0,
        }
    }

    /// The right-aligned columns are the short numeric ones.
    pub fn is_numeric(self) -> bool {
        matches!(self, Self::Year)
    }

    /// Renders one cell, borrowing from the entry rather than copying it.
    pub fn cell(self, entry: Option<&Entry>) -> String {
        let Some(entry) = entry else {
            return String::new();
        };
        match self {
            Self::Key => entry.key.to_string(),
            Self::Kind => entry.kind.as_str().to_owned(),
            Self::Author => entry.field("author").map(first_author).unwrap_or_default(),
            Self::Title => entry.field("title").unwrap_or_default().to_owned(),
            Self::Year => entry.field("year").unwrap_or_default().to_owned(),
            Self::Venue => entry
                .field("journal")
                .or_else(|| entry.field("booktitle"))
                .or_else(|| entry.field("publisher"))
                .map(str::to_owned)
                .unwrap_or_default(),
        }
    }
}

/// The first author as `Last` when the list is `Last, First`, else as written.
fn first_author(authors: &str) -> String {
    let first = authors.split(" and ").next().unwrap_or(authors).trim();
    match first.split_once(',') {
        Some((last, _)) => last.trim().to_owned(),
        None => first.to_owned(),
    }
}

/// The entry a displayed row refers to, if the row is still in range.
pub fn entry_at(session: Option<&Session>, row: usize) -> Option<&Entry> {
    let session = session?;
    let index = *session.order().get(row)?;
    session.library().entries().get(index)
}
