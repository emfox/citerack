//! An inverted index over the terms of one column, answering a term by lookup.
//! A query may match inside a word (`ide` is in `indexing`), so the index keeps
//! whole words and every run of four bytes of every word. A term costs postings.

use std::collections::{HashMap, HashSet};

use crate::token::terms;

/// The length of the runs stored in the table; a term this many bytes or more is
/// answered by intersecting its runs.
pub const WINDOW: usize = 4;

/// The rows that hold one run, or one whole word.
type Postings = Vec<u32>;

/// Every row that holds a given four-byte run, or a given whole word. Rows are
/// sorted and deduplicated so two lists intersect in one pass without a second
/// sort.
pub struct InvertedIndex {
    runs: HashMap<u32, Postings>,
    words: HashMap<Box<str>, Postings>,
    /// The distinct words of this column, so a short term can ask which of them
    /// contain it without reading a row.
    vocabulary: Vec<Box<str>>,
    len: usize,
}

/// The key a four-byte run is stored under, independent of byte order so long as
/// writes and reads agree.
fn key(bytes: &[u8]) -> u32 {
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

impl InvertedIndex {
    /// Indexes every row of one column's folded values. A word a row already
    /// contributed is skipped, because a long abstract repeats its vocabulary and
    /// the repeats would only be written here to be dropped by the dedup below.
    pub fn build(values: &[Option<String>]) -> Self {
        let mut runs: HashMap<u32, Postings> = HashMap::new();
        let mut words: HashMap<Box<str>, Postings> = HashMap::new();
        // Reused across rows, so the per-row set costs one clear and, once the
        // widest cell has been seen, no allocation.
        let mut seen: HashSet<&str> = HashSet::new();
        for (row, value) in values.iter().enumerate() {
            let Some(text) = value else { continue };
            let row = row as u32;
            seen.clear();
            for term in terms(text) {
                let word = &text[term.start..term.end];
                if !seen.insert(word) {
                    continue;
                }
                words.entry(word.into()).or_default().push(row);
                // A word shorter than a run is only ever asked about as a whole
                // word, so storing its runs would be memory nobody reads.
                let bytes = word.as_bytes();
                if bytes.len() < WINDOW {
                    continue;
                }
                // Every run of four bytes, at every offset, so a term landing
                // anywhere inside a word names the row that holds it.
                for at in 0..=bytes.len() - WINDOW {
                    runs.entry(key(&bytes[at..at + WINDOW]))
                        .or_default()
                        .push(row);
                }
            }
        }
        for list in runs.values_mut() {
            list.sort_unstable();
            list.dedup();
        }
        for list in words.values_mut() {
            list.sort_unstable();
            list.dedup();
        }
        let vocabulary = words.keys().cloned().collect();
        Self {
            runs,
            words,
            vocabulary,
            len: values.len(),
        }
    }

    /// The rows holding `term` inside a word, in ascending order. A longer term
    /// needs rows holding all of its runs, since every `surnameN` shares `surn`;
    /// a shorter one comes from the words it is in. The term must be folded.
    pub fn rows_with(&self, term: &str) -> Vec<u32> {
        if term.len() >= WINDOW {
            return self.rows_holding_every_run(term);
        }
        let mut rows: Vec<u32> = Vec::new();
        for word in &self.vocabulary {
            if word
                .as_bytes()
                .windows(term.len())
                .any(|part| part == term.as_bytes())
                && let Some(list) = self.words.get(word)
            {
                rows.extend_from_slice(list);
            }
        }
        rows.sort_unstable();
        rows.dedup();
        rows
    }

    /// The rows holding every run of `term`, narrowed by the rarest run first.
    fn rows_holding_every_run(&self, term: &str) -> Vec<u32> {
        let bytes = term.as_bytes();
        let wanted = bytes.len() - WINDOW + 1;
        let mut lists: Vec<&[u32]> = (0..wanted)
            .filter_map(|at| self.runs.get(&key(&bytes[at..at + WINDOW])))
            .map(Vec::as_slice)
            .collect();
        if lists.len() < wanted {
            // A run nobody wrote means no row holds the term.
            return Vec::new();
        }
        lists.sort_unstable_by_key(|list| list.len());
        let mut rows = lists[0].to_vec();
        for other in &lists[1..] {
            rows = intersect(&rows, other);
            if rows.is_empty() {
                return rows;
            }
        }
        rows
    }

    /// How many runs the table holds, for a report.
    pub fn run_count(&self) -> usize {
        self.runs.len()
    }

    /// How many whole words the index holds, for a report.
    pub fn word_count(&self) -> usize {
        self.words.len()
    }

    /// How many rows the posting lists hold, which is what the build spent its
    /// memory on.
    pub fn posting_count(&self) -> usize {
        self.runs
            .values()
            .chain(self.words.values())
            .map(Vec::len)
            .sum()
    }

    /// Whether the column held no rows when this index was built.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

/// The rows both sorted lists hold, each walked once.
fn intersect(a: &[u32], b: &[u32]) -> Vec<u32> {
    let mut out = Vec::with_capacity(a.len().min(b.len()));
    let (mut i, mut j) = (0, 0);
    while i < a.len() && j < b.len() {
        match a[i].cmp(&b[j]) {
            std::cmp::Ordering::Less => i += 1,
            std::cmp::Ordering::Greater => j += 1,
            std::cmp::Ordering::Equal => {
                out.push(a[i]);
                i += 1;
                j += 1;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn index(values: &[&str]) -> InvertedIndex {
        let owned: Vec<Option<String>> = values.iter().map(|v| Some((*v).to_owned())).collect();
        InvertedIndex::build(&owned)
    }

    #[test]
    fn a_long_term_is_found_inside_a_word() {
        let idx = index(&["zhou2024", "wang2023"]);
        assert_eq!(idx.rows_with("zhou"), vec![0]);
        assert_eq!(idx.rows_with("2024"), vec![0]);
        assert_eq!(idx.rows_with("wang"), vec![1]);
    }

    #[test]
    fn a_term_inside_a_word_is_found_at_every_offset() {
        let idx = index(&["zhou2024", "wang2023"]);
        assert_eq!(
            idx.rows_with("ou20"),
            vec![0],
            "an offset past the first run"
        );
        assert_eq!(idx.rows_with("ng20"), vec![1]);
        assert_eq!(idx.rows_with("hou2"), vec![0]);
    }

    #[test]
    fn every_run_of_a_long_term_narrows_it() {
        let idx = index(&["surname19", "surname199", "surname2"]);
        assert_eq!(idx.rows_with("surname"), vec![0, 1, 2]);
        assert_eq!(idx.rows_with("surname19"), vec![0, 1]);
        assert_eq!(idx.rows_with("surname199"), vec![1]);
    }

    #[test]
    fn a_short_term_finds_the_long_words_holding_it() {
        let idx = index(&["zhou2024", "wang2023"]);
        assert_eq!(idx.rows_with("ou"), vec![0], "`ou` inside `zhou`");
        assert_eq!(idx.rows_with("zh"), vec![0]);
        assert!(idx.rows_with("qz").is_empty());
    }

    #[test]
    fn a_short_term_names_its_word_before_the_words_holding_it() {
        let idx = index(&["a study of something"]);
        assert_eq!(idx.rows_with("of"), vec![0]);
        assert_eq!(idx.rows_with("a"), vec![0]);
    }

    #[test]
    fn a_row_holding_a_run_twice_is_listed_once() {
        let idx = index(&["aaab aaab"]);
        assert_eq!(idx.rows_with("aaab"), vec![0]);
    }

    #[test]
    fn a_repeated_word_is_stored_once_per_row() {
        let once = index(&["retrieval"]);
        let often = index(&["retrieval retrieval retrieval retrieval"]);
        assert_eq!(
            often.posting_count(),
            once.posting_count(),
            "a word written four times still names its row once"
        );
    }

    #[test]
    fn a_term_nobody_wrote_has_no_rows() {
        let idx = index(&["smith, john"]);
        assert!(idx.rows_with("zzzz").is_empty());
        assert!(idx.rows_with("q").is_empty());
    }

    #[test]
    fn a_run_nobody_wrote_ends_the_search_early() {
        let idx = index(&["smith, john"]);
        assert!(
            idx.rows_with("smithq").is_empty(),
            "a run in the middle misses"
        );
    }

    #[test]
    fn a_non_ascii_term_is_indexed_whole() {
        let idx = index(&["神经网络导论", "other"]);
        assert_eq!(idx.rows_with("神经网"), vec![0], "a prefix of the run");
        assert_eq!(idx.rows_with("神经网络"), vec![0]);
    }
}
