//! Finding the same paper entered twice: two entries are a pair when they share a
//! DOI, or a title and a year. Comparison is folded and stripped, so case, accents
//! and punctuation do not hide a duplicate.

use std::collections::HashMap;
use std::collections::hash_map::Entry as MapEntry;

use citerack_bib::{Entry, Library};

/// Why two entries were called the same work.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    /// Both name the same DOI, which is the work's own identity.
    Doi,
    /// Both name the same title in the same year, which is how a library without
    /// DOIs still catches a second copy.
    TitleAndYear,
}

impl Reason {
    /// The reason as a user reads it.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Doi => "the same DOI",
            Self::TitleAndYear => "the same title and year",
        }
    }
}

/// Two entries that look like the same work.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pair {
    /// The earlier entry, by position in the file.
    pub first: usize,
    /// The later entry.
    pub second: usize,
    /// What they matched on.
    pub reason: Reason,
}

/// The keys one entry is found by, which is what a map of every key answers from.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Keys {
    /// The entry's DOI, normalised.
    pub doi: Option<String>,
    /// The entry's title and year, normalised, when it names both.
    pub title_and_year: Option<(String, String)>,
}

impl Keys {
    /// The keys of one entry, each left absent when the entry does not name
    /// enough to pair on.
    pub fn of(entry: &Entry) -> Self {
        let title = entry.field("title").map(normalise_title);
        let year = entry.field("year").and_then(normalise_year);
        let title_and_year = match (title, year) {
            (Some(title), Some(year)) if !title.is_empty() => Some((title, year)),
            _ => None,
        };
        Self {
            doi: entry.field("doi").and_then(normalise_doi),
            title_and_year,
        }
    }
}

/// The first entry holding each key, which is what a scan reports the rest
/// against. Keeping it beside the library is what an edit patches rather than the
/// whole scan: only the edited entry's own keys can move.
#[derive(Debug, Clone, Default)]
pub struct Index {
    doi: HashMap<String, usize>,
    title: HashMap<(String, String), usize>,
}

impl Index {
    /// The first entry holding each key of every entry, built in one pass.
    pub fn build(library: &Library) -> Self {
        Self::scanned(library).1
    }

    /// The pairs one pass finds, and the map of keys that pass built, so a window
    /// pays for one walk and keeps what an edit patches.
    pub fn scanned(library: &Library) -> (Vec<Pair>, Self) {
        let mut index = Self::default();
        let mut pairs = Vec::new();
        for (row, entry) in library.entries().iter().enumerate() {
            pairs.extend(index.remember(row, &Keys::of(entry)));
        }
        (pairs, index)
    }

    /// Records one entry's keys, the earliest entry winning each of them, and
    /// answers the pairs it makes with the entries already recorded.
    fn remember(&mut self, row: usize, keys: &Keys) -> Vec<Pair> {
        let mut pairs = Vec::new();
        if let Some(doi) = &keys.doi {
            match self.doi.entry(doi.clone()) {
                MapEntry::Occupied(held) => pairs.push(Pair {
                    first: *held.get(),
                    second: row,
                    reason: Reason::Doi,
                }),
                MapEntry::Vacant(slot) => {
                    slot.insert(row);
                }
            }
        }
        if let Some(title) = &keys.title_and_year {
            match self.title.entry(title.clone()) {
                MapEntry::Occupied(held) => pairs.push(Pair {
                    first: *held.get(),
                    second: row,
                    reason: Reason::TitleAndYear,
                }),
                MapEntry::Vacant(slot) => {
                    slot.insert(row);
                }
            }
        }
        pairs
    }

    /// Forgets the keys of one entry, so it is recorded again where it now belongs.
    fn forget(&mut self, row: usize, keys: &Keys) {
        if let Some(doi) = &keys.doi
            && self.doi.get(doi) == Some(&row)
        {
            self.doi.remove(doi);
        }
        if let Some(title) = &keys.title_and_year
            && self.title.get(title) == Some(&row)
        {
            self.title.remove(title);
        }
    }

    /// The pairs one entry belongs to, which is what an edit to that entry can
    /// change. Every entry it is reported against is read out of the maps, so the
    /// cost is the entry's own keys rather than the library's.
    pub fn patch(&mut self, library: &Library, row: usize) -> Vec<Pair> {
        let Some(entry) = library.entries().get(row) else {
            return Vec::new();
        };
        let keys = Keys::of(entry);
        self.forget(row, &keys);
        let mut pairs = self.remember(row, &keys);
        // A second copy is paired with the first, not with every copy, so the keys
        // now held by an entry after this one are paired against it here.
        let entries = library.entries();
        for (other, candidate) in entries.iter().enumerate().skip(row + 1) {
            let theirs = Keys::of(candidate);
            if let (Some(doi), Some(theirs)) = (&keys.doi, &theirs.doi)
                && doi == theirs
            {
                pairs.push(Pair {
                    first: row,
                    second: other,
                    reason: Reason::Doi,
                });
            }
            if let (Some(title), Some(theirs)) = (&keys.title_and_year, &theirs.title_and_year)
                && title == theirs
            {
                pairs.push(Pair {
                    first: row,
                    second: other,
                    reason: Reason::TitleAndYear,
                });
            }
        }
        pairs.sort_unstable_by_key(|pair| (pair.first, pair.second, pair.reason as u8));
        pairs
    }
}

/// Every pair that looks like one work entered twice, ordered by the first entry
/// and then the second. A third copy is reported as two pairs against the first,
/// not every combination of the three.
pub fn find(library: &Library) -> Vec<Pair> {
    let (mut pairs, _) = Index::scanned(library);
    pairs.sort_unstable_by_key(|pair| (pair.first, pair.second, pair.reason as u8));
    pairs
}

/// The DOI without the spellings that mean the same one: a `doi:` or `https://doi.org/`
/// prefix, surrounding space, and case. A DOI is case-insensitive by its own rules.
fn normalise_doi(raw: &str) -> Option<String> {
    let mut doi = raw.trim();
    for prefix in ["https://doi.org/", "http://doi.org/", "doi.org/", "doi:"] {
        if doi.len() > prefix.len() && doi[..prefix.len()].eq_ignore_ascii_case(prefix) {
            doi = doi[prefix.len()..].trim();
            break;
        }
    }
    // Only a DOI that starts with the registrant prefix is one; anything else in
    // the field is a note somebody typed, not an identity to pair on.
    if !doi.starts_with("10.") || doi.len() < 5 {
        return None;
    }
    Some(doi.to_ascii_lowercase())
}

/// The title folded, braces removed and everything but letters and digits dropped,
/// so `A Study of {Nested} Braces` matches `a study of nested braces`. Braces go
/// before the spacing: `{N}ested` is `Nested`, not `N ested`.
fn normalise_title(raw: &str) -> String {
    let unbraced: String = raw.chars().filter(|c| *c != '{' && *c != '}').collect();
    let mut out = String::with_capacity(unbraced.len());
    let mut spaced = false;
    for c in crate::fold(&unbraced).chars() {
        if c.is_alphanumeric() {
            out.push(c);
            spaced = false;
        } else if !spaced && !out.is_empty() {
            out.push(' ');
            spaced = true;
        }
    }
    // A trailing separator is not part of the title.
    while out.ends_with(' ') {
        out.pop();
    }
    out
}

/// The year as four digits, so `2024`, `{2024}` and ` 2024 ` are one year. A value
/// that is not a year is no year at all rather than a string to compare.
fn normalise_year(raw: &str) -> Option<String> {
    let digits: String = raw.chars().filter(|c| c.is_ascii_digit()).take(4).collect();
    if digits.len() == 4 {
        Some(digits)
    } else {
        None
    }
}
