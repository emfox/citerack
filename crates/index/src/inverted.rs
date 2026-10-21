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
    /// Which words contain each run of one to three bytes, so a term narrower than
    /// a run costs a lookup instead of a walk over the vocabulary. Each word is held
    /// once in `by_gram`, and a gram names a range of them.
    grams: HashMap<u64, (u32, u32)>,
    by_gram: Vec<WordId>,
    /// The rows of each word, as a range of `flat`, so answering a short term
    /// never pays a string hash per word.
    per_word: Vec<(u32, u32)>,
    flat: Postings,
    len: usize,
}

/// An index into a column's dictionary.
type WordId = u32;

/// The key a four-byte run is stored under, independent of byte order so long as
/// writes and reads agree.
fn key(bytes: &[u8]) -> u32 {
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

/// The key a run of one to three bytes is stored under: the bytes in the low half
/// and the width in the high half, so `ab` and `ab` padded with zeros stay apart.
fn gram_key(bytes: &[u8]) -> u64 {
    let mut buf = [0u8; WINDOW];
    buf[..bytes.len()].copy_from_slice(bytes);
    u64::from(u32::from_le_bytes(buf)) | ((bytes.len() as u64) << 32)
}

impl InvertedIndex {
    /// Indexes every row of one column's folded values. A word a row already
    /// contributed is skipped, because a long abstract repeats its vocabulary and
    /// the repeats would only be written here to be dropped by the dedup below.
    pub fn build(values: &[Option<String>]) -> Self {
        let mut runs: HashMap<u32, Postings> = HashMap::new();
        let mut words: HashMap<Box<str>, Postings> = HashMap::new();
        let mut by_gram: Vec<(u64, WordId)> = Vec::new();
        let mut words_of: Vec<Box<str>> = Vec::new();
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
                // A word reaches the gram list only the first time the column
                // sees it, which keeps that list the size of the vocabulary
                // rather than the size of the library.
                if !words.contains_key(word) {
                    let id = words_of.len() as WordId;
                    words_of.push(word.into());
                    for width in 1..WINDOW.min(word.len()) + 1 {
                        let bytes = word.as_bytes();
                        for at in 0..=bytes.len() - width {
                            by_gram.push((gram_key(&bytes[at..at + width]), id));
                        }
                    }
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
        // One sorted array of gram and word, so a gram is a contiguous range. Words
        // are ordered rare first, so a query that fills every row stops early.
        // Reach is computed once, since a key function runs per comparison.
        let reach: Vec<u32> = words_of
            .iter()
            .map(|word| words.get(word.as_ref()).map_or(0, |list| list.len()) as u32)
            .collect();
        by_gram.sort_unstable_by_key(|(gram, id)| (*gram, reach[*id as usize], *id));
        by_gram.dedup();
        let mut grams: HashMap<u64, (u32, u32)> = HashMap::new();
        let mut at = 0u32;
        while at < by_gram.len() as u32 {
            let gram = by_gram[at as usize].0;
            let start = at;
            while at < by_gram.len() as u32 && by_gram[at as usize].0 == gram {
                at += 1;
            }
            grams.insert(gram, (start, at - start));
        }
        let mut per_word: Vec<(u32, u32)> = vec![(0, 0); words_of.len()];
        let mut flat: Postings = Vec::new();
        for (id, word) in words_of.iter().enumerate() {
            let Some(list) = words.get(word.as_ref()) else {
                continue;
            };
            per_word[id] = (flat.len() as u32, list.len() as u32);
            flat.extend_from_slice(list);
        }
        Self {
            runs,
            words,
            grams,
            by_gram: by_gram.iter().map(|(_, id)| *id).collect(),
            per_word,
            flat,
            len: values.len(),
        }
    }

    /// The rows holding `term` inside a word, in ascending order. A term of four
    /// bytes or more needs rows holding all of its runs, since every `surnameN`
    /// shares `surn`; a shorter one is answered by the words it appears in.
    pub fn rows_with(&self, term: &str) -> Vec<u32> {
        if term.len() >= WINDOW {
            return self.rows_holding_every_run(term);
        }
        // A bitmap collects the rows of those words, since sorting the repeats
        // across many words costs more than marking them. The walk is rarest
        // first and stops once every row is marked.
        let Some((start, len)) = self.grams.get(&gram_key(term.as_bytes())) else {
            return Vec::new();
        };
        let mut marked = vec![false; self.len];
        let mut unmarked = self.len;
        for id in &self.by_gram[*start as usize..(*start + *len) as usize] {
            let Some(&(at, count)) = self.per_word.get(*id as usize) else {
                continue;
            };
            let rows = &self.flat[at as usize..(at + count) as usize];
            for row in rows {
                if let Some(slot) = marked.get_mut(*row as usize)
                    && !*slot
                {
                    *slot = true;
                    unmarked -= 1;
                }
            }
            if unmarked == 0 {
                break;
            }
        }
        if unmarked == self.len {
            return Vec::new();
        }
        marked
            .iter()
            .enumerate()
            .filter(|(_, hit)| **hit)
            .map(|(row, _)| row as u32)
            .collect()
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
    fn a_run_in_most_rows_reaches_every_row_that_holds_it() {
        // A byte as common as `a` sits in nearly every row, and the rows that lack
        // it must still be excluded on a library wide enough to have many words.
        let values: Vec<Option<String>> = (0..12_000)
            .map(|i| {
                let value = if i % 1_000 == 0 {
                    format!("qqq{i}zzz")
                } else {
                    format!("tar{i}nam")
                };
                Some(value)
            })
            .collect();
        let idx = InvertedIndex::build(&values);
        let hit = idx.rows_with("a");
        assert_eq!(hit.len(), 11_988, "every row but the ones without an a");
        assert!(!hit.contains(&0), "the row without an a is excluded");
        assert!(hit.contains(&1), "a row with an a is kept");
        assert_eq!(idx.rows_with("zzz").len(), 12, "a rare run still answers");
    }

    #[test]
    fn a_short_term_is_answered_without_walking_the_vocabulary() {
        let idx = index(&["bwb1985", "other", "zzzbwb"]);
        assert_eq!(
            idx.rows_with("bwb"),
            vec![0, 2],
            "inside a word, not only as one"
        );
        assert_eq!(idx.rows_with("wb"), vec![0, 2], "the tail of both words");
        assert_eq!(
            idx.rows_with("wb1"),
            vec![0],
            "a two-byte and a one-byte run"
        );
        assert!(
            idx.rows_with("wq").is_empty(),
            "letters a word does not hold"
        );
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
