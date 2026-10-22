//! The table's columns, and how a cell is rendered from a library row.

use crate::catalogue::Text;
use citerack_bib::Entry;
use citerack_store::{Facet, Session};

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
    /// The key the pattern would give the entry, which is what a citation written
    /// after the reader fixes the keys will say.
    Generated,
}

impl Column {
    /// Every column, in the order the table lays them out.
    pub const ALL: [Self; 7] = [
        Self::Key,
        Self::Kind,
        Self::Author,
        Self::Title,
        Self::Year,
        Self::Venue,
        Self::Generated,
    ];

    /// The key of the text the header shows.
    pub fn header(self) -> Text {
        match self {
            Self::Key => Text::ColumnKey,
            Self::Kind => Text::ColumnType,
            Self::Author => Text::ColumnAuthor,
            Self::Title => Text::ColumnTitle,
            Self::Year => Text::ColumnYear,
            Self::Venue => Text::ColumnVenue,
            Self::Generated => Text::ColumnGenerated,
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
            Self::Generated => 130.0,
        }
    }

    /// The right-aligned columns are the short numeric ones.
    pub fn is_numeric(self) -> bool {
        matches!(self, Self::Year)
    }

    /// Renders one cell. The column and the facet list above it read their
    /// values from the same place, so a group is always spelled as its rows are.
    pub fn cell(self, entry: Option<&Entry>) -> String {
        let Some(entry) = entry else {
            return String::new();
        };
        let facet = match self {
            Self::Author => Facet::Author,
            Self::Venue => Facet::Venue,
            Self::Year => Facet::Year,
            Self::Kind => Facet::Kind,
            Self::Generated => return String::new(),
            Self::Key | Self::Title => {
                return match self {
                    Self::Key => entry.key.to_string(),
                    _ => entry.field("title").unwrap_or_default().to_owned(),
                };
            }
        };
        Facet::plain(facet, entry).unwrap_or_default()
    }
}

/// The entry a displayed row refers to, if the row is still in range.
pub fn entry_at(session: Option<&Session>, row: usize) -> Option<&Entry> {
    let session = session?;
    let index = *session.order().get(row)?;
    session.library().entries().get(index)
}
