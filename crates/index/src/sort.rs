//! Sort keys computed once, so re-sorting a view never re-reads an entry.

use std::cmp::Ordering;

use citerack_bib::Library;

/// What the row order is keyed on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SortField {
    /// The citation key.
    Key,
    /// The entry type.
    Kind,
    /// The first author, in the `Last, First` order BibTeX usually uses.
    FirstAuthor,
    /// The four-digit year, as a number.
    Year,
    /// The title.
    Title,
    /// Any other field, by normalised name.
    Named(String),
}

/// One row's sort key, precomputed for every column a view offers.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SortKey {
    /// The author's surname, folded.
    pub author: String,
    /// The year as a number, so `999` sorts before `2001`.
    pub year: u32,
    /// The title, folded, with a leading article removed.
    pub title: String,
    /// The citation key, folded.
    pub key: String,
    /// The entry type.
    pub kind: String,
}

/// A sort instruction: a field, and a direction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SortSpec {
    /// The field to order by.
    pub field: SortField,
    /// Whether to reverse the ascending order.
    pub descending: bool,
}

impl SortKey {
    /// Precomputes one row's keys.
    pub fn of(library: &Library, row: usize) -> Self {
        let Some(entry) = library.entries().get(row) else {
            return Self::default();
        };
        let author = entry
            .field("author")
            .or_else(|| entry.field("editor"))
            .map(first_surname)
            .unwrap_or_default();
        Self {
            author,
            year: entry.field("year").and_then(parse_year).unwrap_or(0),
            title: entry.field("title").map(strip_article).unwrap_or_default(),
            key: crate::fold(&entry.key),
            kind: crate::fold(entry.kind.as_str()),
        }
    }

    /// Compares two keys for one field, ascending.
    pub fn compare(&self, other: &Self, field: &SortField) -> Ordering {
        match field {
            SortField::Key => self.key.cmp(&other.key),
            SortField::Kind => self.kind.cmp(&other.kind),
            SortField::FirstAuthor => self.author.cmp(&other.author),
            SortField::Year => self.year.cmp(&other.year),
            SortField::Title => self.title.cmp(&other.title),
            SortField::Named(name) => match name.as_str() {
                "author" | "editor" => self.author.cmp(&other.author),
                "year" => self.year.cmp(&other.year),
                "title" => self.title.cmp(&other.title),
                _ => self.key.cmp(&other.key),
            },
        }
    }
}

/// The surname of the first author in a `Last, First and Other, Person` list.
fn first_surname(author: &str) -> String {
    let first = author.split(" and ").next().unwrap_or(author).trim();
    let surname = match first.split_once(',') {
        Some((last, _)) => last,
        None => first.rsplit_once(' ').map_or(first, |(_, last)| last),
    };
    crate::fold(surname.trim())
}

/// The first four-digit run in a year field, which survives `{2024}` and `2024a`.
fn parse_year(raw: &str) -> Option<u32> {
    let digits: String = raw
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(char::is_ascii_digit)
        .collect();
    digits.parse().ok()
}

/// Removes an English leading article, so `The Art of X` sorts under `Art`.
fn strip_article(title: &str) -> String {
    let folded = crate::fold(title);
    for article in ["the ", "a ", "an "] {
        if let Some(rest) = folded.strip_prefix(article) {
            if !rest.trim().is_empty() {
                return rest.to_owned();
            }
        }
    }
    folded
}
