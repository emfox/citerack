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

/// Every pair that looks like one work entered twice, ordered by the first entry
/// and then the second so the list is stable. A third copy is reported as two
/// pairs against the first, not every combination of the three.
pub fn find(library: &Library) -> Vec<Pair> {
    let entries = library.entries();
    let mut pairs = Vec::new();
    pairs.extend(by_doi(entries));
    pairs.extend(by_title_and_year(entries));
    pairs.sort_unstable_by_key(|pair| (pair.first, pair.second, pair.reason as u8));
    pairs
}

/// Pairs entries naming the same DOI. The first entry holding an identity is the
/// one the rest are reported against.
fn by_doi(entries: &[Entry]) -> Vec<Pair> {
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut pairs = Vec::new();
    for (row, entry) in entries.iter().enumerate() {
        let Some(doi) = entry.field("doi").and_then(normalise_doi) else {
            continue;
        };
        match seen.entry(doi) {
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
    pairs
}

/// Pairs entries sharing a title and a year. A year that neither entry names is
/// not a year they share, so two untitled-in-year entries are left alone: without
/// both halves there is nothing to be confident about.
fn by_title_and_year(entries: &[Entry]) -> Vec<Pair> {
    let mut seen: HashMap<(String, String), usize> = HashMap::new();
    let mut pairs = Vec::new();
    for (row, entry) in entries.iter().enumerate() {
        let Some(title) = entry.field("title").map(normalise_title) else {
            continue;
        };
        if title.is_empty() {
            continue;
        }
        let Some(year) = entry.field("year").and_then(normalise_year) else {
            continue;
        };
        match seen.entry((title, year)) {
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
