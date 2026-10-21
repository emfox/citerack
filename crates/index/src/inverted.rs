//! An inverted index over the terms of one column, answering a term by lookup.
//! A query may match inside a word (`ide` is in `indexing`), so the index keeps
//! whole words and every run of four bytes of every word. A term costs postings.

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::ops::Range;

use crate::token::terms;

/// The length of the runs stored in the table; a term this many bytes or more is
/// answered by intersecting its runs.
pub const WINDOW: usize = 4;

/// The rows that hold one run, or one whole word.
type Postings = Vec<u32>;

/// Every row that holds a given four-byte run, or a given whole word. Rows are
/// sorted and deduplicated so two lists intersect in one pass without a second
/// sort.
#[derive(Clone)]
pub struct InvertedIndex {
    runs: HashMap<u32, Postings>,
    words: HashMap<Box<str>, Postings>,
    /// Where each run of one to three bytes starts in `by_gram`, so a term narrower
    /// than a run costs a lookup instead of a walk over the vocabulary.
    grams: HashMap<u64, u32>,
    /// The words those runs name, each held once per gram it belongs to. A gram
    /// naming one word is one entry here, which is what a library of invented words
    /// mostly has.
    by_gram: Vec<WordId>,
    /// Where each word of `by_gram` ends the run it belongs to, so a gram naming
    /// several words is read as a range without asking the table which come next.
    next_gram: Vec<u32>,
    /// Each run of one to three bytes of a word a single row holds, as a pair of run
    /// and row: such a word needs no vocabulary, so a library whose words are all
    /// unique pays a pair per run rather than a word id, a range and a key.
    solo: Vec<(u64, u32)>,
    /// Which entries of `solo` hold each run, so a run is a contiguous range.
    solo_at: HashMap<u64, (u32, u32)>,
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

/// The widths of run to index for a word of this length: every width below a run,
/// and every width of the word itself when the word is shorter than one.
fn widths(len: usize) -> usize {
    WINDOW.min(len + 1)
}

impl InvertedIndex {
    /// Indexes every row of one column's folded values. A word a row already
    /// contributed is skipped, because a long abstract repeats its vocabulary and
    /// the repeats would only be written here to be dropped by the dedup below.
    pub fn build(values: &[Option<String>]) -> Self {
        let mut draft = Draft::new();
        for (row, value) in values.iter().enumerate() {
            if let Some(text) = value {
                draft.add(row as u32, text);
            }
        }
        draft.finish()
    }

    /// This index with one row's text replaced, which is what an edit costs
    /// rather than a rebuild of the column.
    pub fn with_row(&self, row: usize, text: Option<&str>) -> Self {
        let mut draft = Draft::new();
        for at in 0..self.len {
            if at != row {
                draft.push(at as u32, self.rows_of(at));
            }
        }
        if let Some(text) = text {
            draft.add(row as u32, text);
        }
        draft.finish()
    }

    /// Every row holding one word, which is the inverse of what `build` was given.
    fn rows_of(&self, row: usize) -> impl Iterator<Item = (Cow<'_, str>, u32)> + '_ {
        self.words.iter().filter_map(move |(word, rows)| {
            rows.binary_search(&(row as u32))
                .ok()
                .map(|_| (Cow::Borrowed(word.as_ref()), row as u32))
        })
    }
}

/// The columns of one index while they are being collected, before the tables a
/// query reads are laid out from them.
struct Draft {
    runs: HashMap<u32, Postings>,
    words: HashMap<Box<str>, Postings>,
    by_gram: Vec<(u64, WordId)>,
    solo: Vec<(u64, u32)>,
    words_of: Vec<Box<str>>,
    len: usize,
}

impl Draft {
    fn new() -> Self {
        Self {
            runs: HashMap::new(),
            words: HashMap::new(),
            by_gram: Vec::new(),
            solo: Vec::new(),
            words_of: Vec::new(),
            len: 0,
        }
    }

    /// Records one row's postings for words it already contributed. The runs of
    /// a word follow from the word, so they are read back off it rather than
    /// stored twice.
    fn push<'a>(&mut self, row: u32, held: impl Iterator<Item = (Cow<'a, str>, u32)>) {
        for (word, _) in held {
            self.len = self.len.max(row as usize + 1);
            self.word(row, &word);
        }
    }

    /// Records one word of one row, with the runs a query finds it by.
    fn word(&mut self, row: u32, word: &str) {
        if !self.words.contains_key(word) {
            self.words_of.push(word.into());
        }
        self.words.entry(word.into()).or_default().push(row);
        // A word shorter than a run is only ever asked about as a whole word, so
        // storing its runs would be memory nobody reads.
        let bytes = word.as_bytes();
        if bytes.len() < WINDOW {
            return;
        }
        for at in 0..=bytes.len() - WINDOW {
            self.runs
                .entry(key(&bytes[at..at + WINDOW]))
                .or_default()
                .push(row);
        }
    }

    /// Records every word of one row's text, each once.
    fn add(&mut self, row: u32, text: &str) {
        self.len = self.len.max(row as usize + 1);
        // Reused across rows, so the per-row set costs one clear and, once the
        // widest cell has been seen, no allocation.
        let mut seen: HashSet<&str> = HashSet::new();
        for term in terms(text) {
            let word = &text[term.start..term.end];
            if seen.insert(word) {
                self.word(row, word);
            }
        }
    }

    fn finish(self) -> InvertedIndex {
        let Self {
            mut runs,
            mut words,
            mut by_gram,
            mut solo,
            words_of,
            len,
        } = self;
        for list in runs.values_mut() {
            list.sort_unstable();
            list.dedup();
        }
        for list in words.values_mut() {
            list.sort_unstable();
            list.dedup();
        }
        // One sorted array of gram and word, so a gram is a contiguous run. Words are
        // ordered rare first, so a query that fills every row stops early. Reach is
        // computed once, since a key function runs per comparison.
        let reach: Vec<u32> = words_of
            .iter()
            .map(|word| words.get(word.as_ref()).map_or(0, |list| list.len()) as u32)
            .collect();
        // A word held by one row needs no word-to-rows indirection: its row is the
        // answer, and it goes in `solo` as a plain pair. The rest are ordered and
        // indexed as they were.
        for (id, word) in words_of.iter().enumerate() {
            let held = reach[id];
            if held <= 1 {
                let row = words.get(word.as_ref()).and_then(|l| l.first()).copied();
                if let Some(row) = row {
                    // Widths below a run only, since a four-byte gram would never
                    // be read; a word shorter than a run gives every width it has.
                    for width in 1..widths(word.len()) {
                        let bytes = word.as_bytes();
                        for at in 0..=bytes.len() - width {
                            solo.push((gram_key(&bytes[at..at + width]), row));
                        }
                    }
                }
                continue;
            }
            for width in 1..widths(word.len()) {
                let bytes = word.as_bytes();
                for at in 0..=bytes.len() - width {
                    by_gram.push((gram_key(&bytes[at..at + width]), id as WordId));
                }
            }
        }
        // A reach of zero is a word no row holds, which the dedup above cannot
        // take away since a live word shares its bytes. Nothing may name it.
        by_gram.retain(|(_, id)| reach[*id as usize] > 0);
        by_gram.sort_unstable_by_key(|(gram, id)| (*gram, reach[*id as usize], *id));
        by_gram.dedup();
        // Each gram names a range of `words_at`, which is `by_gram` without the gram
        // keys. A gram naming one word is most of the table on a library of invented
        // words, and its entry is then one word id rather than a start and a length.
        let mut grams: HashMap<u64, u32> = HashMap::new();
        let mut words_at: Vec<WordId> = Vec::new();
        let mut next_gram: Vec<u32> = Vec::new();
        let mut at = 0usize;
        while at < by_gram.len() {
            let gram = by_gram[at].0;
            let mut end = at + 1;
            while end < by_gram.len() && by_gram[end].0 == gram {
                end += 1;
            }
            grams.insert(gram, words_at.len() as u32);
            words_at.extend(by_gram[at..end].iter().map(|(_, id)| *id));
            next_gram.resize(words_at.len(), words_at.len() as u32);
            at = end;
        }
        // The solo table is sorted by gram and then row, so a gram names a run of
        // rows with no vocabulary behind it.
        solo.sort_unstable();
        solo.dedup();
        let mut solo_at: HashMap<u64, (u32, u32)> = HashMap::new();
        let mut at = 0u32;
        while at < solo.len() as u32 {
            let gram = solo[at as usize].0;
            let start = at;
            while at < solo.len() as u32 && solo[at as usize].0 == gram {
                at += 1;
            }
            solo_at.insert(gram, (start, at - start));
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
        InvertedIndex {
            runs,
            words,
            grams,
            by_gram: words_at,
            next_gram,
            solo,
            solo_at,
            per_word,
            flat,
            len,
        }
    }
}

impl InvertedIndex {
    /// The words a gram entry names, as a range of `by_gram`. A start past the end
    /// names nothing, which is an empty range rather than a clamped one.
    fn gram_words(&self, held: u32) -> Range<usize> {
        let start = held as usize;
        let words = self.by_gram.len();
        if start >= words {
            return words..words;
        }
        let end = self
            .next_gram
            .get(start)
            .map_or(words, |next| *next as usize);
        start..end.clamp(start, words)
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
        let key_of_gram = gram_key(term.as_bytes());
        let mut marked = vec![false; self.len];
        let mut unmarked = self.len;
        let mut any = false;
        if let Some(held) = self.grams.get(&key_of_gram) {
            // A gram names its words where it says to, and `next_gram` says where the
            // run ends, so a gram naming one word costs one word id and no length.
            for id in &self.by_gram[self.gram_words(*held)] {
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
                        any = true;
                    }
                }
                if unmarked == 0 {
                    break;
                }
            }
        }
        // A word held by a single row is a pair of run and row, so its rows come
        // straight out of `solo` with no vocabulary to walk.
        if unmarked > 0
            && let Some((start, len)) = self.solo_at.get(&key_of_gram)
        {
            for (_, row) in &self.solo[*start as usize..(*start + *len) as usize] {
                if let Some(slot) = marked.get_mut(*row as usize)
                    && !*slot
                {
                    *slot = true;
                    any = true;
                }
            }
        }
        if !any {
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

    /// How many distinct runs the gram table names, for a report.
    pub fn gram_count(&self) -> usize {
        self.grams.len()
    }

    /// How many gram entries the table of words held by a single row has, for a
    /// report.
    pub fn solo_count(&self) -> usize {
        self.solo.len()
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
    fn a_word_only_one_row_holds_costs_no_gram_of_its_own() {
        // Every word here is invented, so no word is held by two rows and the whole
        // vocabulary belongs in the solo table rather than the gram table.
        let values: Vec<Option<String>> = (0..2_000)
            .map(|i| Some(format!("w{i:04}x{}", (i * 7_919) % 10_003)))
            .collect();
        let idx = InvertedIndex::build(&values);
        assert_eq!(
            idx.gram_count(),
            0,
            "no gram names a word that more than one row holds"
        );
        assert!(
            idx.solo_count() > 0,
            "the one-row words are all in the solo table"
        );
        for term in ["w0001", "x1", "w1999"] {
            let mut want: Vec<u32> = (0..values.len() as u32)
                .filter(|row| {
                    values[*row as usize]
                        .as_deref()
                        .is_some_and(|v| v.contains(term))
                })
                .collect();
            want.dedup();
            assert_eq!(idx.rows_with(term), want, "term {term:?}");
        }
    }

    #[test]
    fn a_word_many_rows_hold_is_answered_from_the_gram_table() {
        let idx = index(&["retrieval", "retrieval again", "retrieval again and more"]);
        assert!(idx.gram_count() > 0, "a repeated word is indexed as a gram");
        assert_eq!(idx.rows_with("rie"), vec![0, 1, 2]);
    }

    #[test]
    fn a_four_byte_gram_is_not_stored_because_it_is_never_read() {
        // A term of four bytes or more is answered from the run table, so a gram of
        // four bytes would be written and never asked for.
        let idx = index(&["retrieval", "retrieval and more", "retrieval again"]);
        let widest = idx.grams.keys().map(|gram| gram >> 32).max();
        assert_eq!(widest, Some(3), "the widest gram is three bytes");
    }

    #[test]
    fn a_gram_shared_by_many_words_names_all_of_them() {
        // `ind` sits in four words held by different sets of rows, so the gram names
        // more than one word and its run is read to the end of the group.
        let idx = index(&["index", "index indexing", "reindex", "ind"]);
        assert_eq!(idx.rows_with("ind"), vec![0, 1, 2, 3]);
        assert_eq!(idx.rows_with("nde"), vec![0, 1, 2], "and a narrower one");
    }

    #[test]
    fn a_non_ascii_term_is_indexed_whole() {
        let idx = index(&["神经网络导论", "other"]);
        assert_eq!(idx.rows_with("神经网"), vec![0], "a prefix of the run");
        assert_eq!(idx.rows_with("神经网络"), vec![0]);
    }
}
