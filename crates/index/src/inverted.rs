//! An inverted index over the terms of one column, answering a term by lookup.
//! A query may match inside a word (`ide` is in `indexing`), so the index keeps
//! whole words and every run of four bytes of every word. A term costs postings.

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
    words: Vocabulary,
    /// The gram table, out of date after an edit until a short term needs it.
    grams: Option<Grams>,
    /// Each run of one to three bytes of a word a single row holds, as a pair of run
    /// and row, so such a word needs no vocabulary. It names rows, so an edit that
    /// moves one rebuilds it from the vocabulary.
    solo: Solo,
    /// Which entries of `solo` hold each run, so a run is a contiguous range. An
    /// edit drops it, since which rows hold a word is what an edit changes.
    solo_at: Option<HashMap<u64, (u32, u32)>>,
    /// The rows of each word, as a range of `flat`, so answering a short term
    /// never pays a string hash per word.
    per_word: Vec<(u32, u32)>,
    flat: Postings,
    len: usize,
    /// How many words the last walk over the vocabulary left with no row, so the
    /// walk is paid for by many edits rather than by each one.
    dead: usize,
}

/// An index into a column's dictionary.
type WordId = u32;

/// The words one column's table holds, each held by a run of rows.
type Vocabulary = HashMap<Box<str>, Postings>;

/// The gram table of one vocabulary, split into what a query reads.
#[derive(Clone, Debug, Default)]
struct Grams {
    /// Where each four-byte-or-shorter gram starts in `words`.
    at: HashMap<u64, u32>,
    /// The words those grams name, each held once per gram it belongs to.
    words: Vec<WordId>,
    /// Where each entry of `words` ends the gram it belongs to, so a gram naming
    /// several words is read as a range without asking the table which come next.
    next: Vec<u32>,
}

/// The runs of every word no two rows share, as a pair of run and row.
type Solo = Vec<(u64, u32)>;

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

/// One text's distinct words, sorted, which is how a row's postings are held.
fn words_of(text: &str) -> Vec<Box<str>> {
    let mut seen: HashSet<&str> = HashSet::new();
    let mut words: Vec<Box<str>> = Vec::new();
    for term in terms(text) {
        let word = &text[term.start..term.end];
        if seen.insert(word) {
            words.push(word.into());
        }
    }
    words.sort_unstable();
    words
}

/// The runs a row's words put it into, sorted and deduplicated.
fn runs_of(words: &[Box<str>]) -> Vec<u32> {
    let mut runs: Vec<u32> = Vec::new();
    for word in words {
        let bytes = word.as_bytes();
        if bytes.len() < WINDOW {
            continue;
        }
        for at in 0..=bytes.len() - WINDOW {
            runs.push(key(&bytes[at..at + WINDOW]));
        }
    }
    runs.sort_unstable();
    runs.dedup();
    runs
}

/// The widths of run to index for a word of this length: every width below a run,
/// and every width of the word itself when the word is shorter than one.
fn widths(len: usize) -> usize {
    WINDOW.min(len + 1)
}

/// The tables of one index while they are being collected, before the gram and
/// solo tables are laid out from them.
struct Draft {
    runs: HashMap<u32, Postings>,
    words: Vocabulary,
    len: usize,
}

impl Draft {
    fn new() -> Self {
        Self {
            runs: HashMap::new(),
            words: Vocabulary::new(),
            len: 0,
        }
    }

    /// Records one word of one row, with the runs a query finds it by.
    fn word(&mut self, row: u32, word: &str) {
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
        let (grams, solo, order) = lay_out(&words);
        let solo_at = runs_at(&solo);
        // Laid out in the order the words are numbered in, not in the order the rows
        // were read, so a word id finds its own rows.
        let mut per_word: Vec<(u32, u32)> = vec![(0, 0); order.len()];
        let mut flat: Postings = Vec::new();
        for (id, word) in order.iter().enumerate() {
            let Some(list) = words.get(word.as_ref()) else {
                continue;
            };
            per_word[id] = (flat.len() as u32, list.len() as u32);
            flat.extend_from_slice(list);
        }
        // What every row said, so the first edit of a row can take its words back
        // out of the tables rather than reading the whole of them.
        InvertedIndex {
            runs,
            words,
            grams: Some(grams),
            solo,
            solo_at: Some(solo_at),
            per_word,
            flat,
            len,
            dead: 0,
        }
    }
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

    /// Replaces one row's text, so an edit costs the row's words rather than the
    /// column's. The gram tables are dropped rather than rebuilt, since they name a
    /// word rather than a row, and laid out again once the keystrokes are over.
    pub fn replace_row(&mut self, row: usize, before: Option<&str>, text: Option<&str>) {
        let row = row as u32;
        self.grams = None;
        let held = before.map(words_of).unwrap_or_default();
        let now: Vec<Box<str>> = text.map(words_of).unwrap_or_default();
        let mut emptied = false;
        for word in &held {
            if now.binary_search(word).is_err()
                && let Some(rows) = self.words.get_mut(word)
            {
                rows.retain(|held| *held != row);
                emptied |= rows.is_empty();
            }
        }
        for word in &now {
            if held.binary_search(word).is_err() {
                self.words.entry(word.clone()).or_default().push(row);
            }
        }
        // A word no row holds only grows the vocabulary, so the walk waits until a
        // tenth of the table is dead rather than running once per edit.
        self.dead += usize::from(emptied);
        if self.dead * 10 > self.words.len() {
            self.words.retain(|_, rows| !rows.is_empty());
            self.dead = 0;
        }
        let was = runs_of(&held);
        let runs = runs_of(&now);
        for run in &was {
            if runs.binary_search(run).is_err()
                && let Some(rows) = self.runs.get_mut(run)
                && let Ok(at) = rows.binary_search(&row)
            {
                rows.remove(at);
            }
        }
        for run in &runs {
            if was.binary_search(run).is_err() {
                self.runs.entry(*run).or_default().push(row);
            }
        }
        self.len = self.len.max(row as usize + 1);
    }

    /// Lays out what an edit dropped: the gram table, and the runs of the solo
    /// table. Both name a word rather than a row, so they are built again rather
    /// than patched, and a run of keystrokes would otherwise pay for each one.
    pub fn relaid(&mut self) {
        let (grams, solo, order) = lay_out(&self.words);
        self.grams = Some(grams);
        self.solo = solo;
        self.solo_at = Some(runs_at(&self.solo));
        // The numbering is this layout's, so the rows are read again under it.
        self.per_word = vec![(0, 0); order.len()];
        self.flat = Postings::new();
        for (id, word) in order.iter().enumerate() {
            let Some(list) = self.words.get(word.as_ref()) else {
                continue;
            };
            self.per_word[id] = (self.flat.len() as u32, list.len() as u32);
            self.flat.extend_from_slice(list);
        }
    }
}

/// The gram table of one vocabulary, and the runs of the words only one row holds:
/// a term narrower than a run costs a lookup either way, and a word nobody else
/// wrote names its row instead of a placeholder entry in the vocabulary.
fn lay_out(words: &Vocabulary) -> (Grams, Solo, Vec<Box<str>>) {
    // Rarest first, which is also the numbering `per_word` is read by. Reach travels
    // in the tuple being sorted, since a comparison that looked a word up would cost
    // a hash probe per comparison rather than per word.
    let mut order: Vec<(u32, &Box<str>)> = words
        .iter()
        .map(|(word, rows)| (rows.len() as u32, word))
        .collect();
    order.sort_unstable_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(b.1)));
    let held: Vec<u32> = order.iter().map(|(reach, _)| *reach).collect();
    let order: Vec<Box<str>> = order.into_iter().map(|(_, word)| word.clone()).collect();
    let mut pairs: Vec<(u64, WordId)> = Vec::new();
    let mut solo: Solo = Vec::new();
    for (id, word) in order.iter().enumerate() {
        let held = held[id];
        // A word one row holds names that row rather than itself, so it is answered
        // from a pair of run and row and needs no entry in the vocabulary. A term of
        // a `WINDOW` or more is answered from `runs`, which every word is in.
        if held <= 1 {
            let Some(&row) = words.get(word).and_then(|rows| rows.first()) else {
                continue;
            };
            let bytes = word.as_bytes();
            for width in 1..widths(word.len()) {
                for at in 0..=bytes.len() - width {
                    solo.push((gram_key(&bytes[at..at + width]), row));
                }
            }
            continue;
        }
        let bytes = word.as_bytes();
        for width in 1..widths(word.len()) {
            for at in 0..=bytes.len() - width {
                pairs.push((gram_key(&bytes[at..at + width]), id as WordId));
            }
        }
    }
    // A run is looked up as a run, so a word is ranked by its rows rather than by
    // which key named it.
    let rank = |id: WordId| held[id as usize];
    pairs.sort_unstable_by(|a, b| {
        a.0.cmp(&b.0)
            .then_with(|| rank(a.1).cmp(&rank(b.1)))
            .then_with(|| a.1.cmp(&b.1))
    });
    // The same word said twice by one row names its gram twice, and only a pair is
    // a repeat: two words sharing a gram are two entries of that gram.
    pairs.dedup();
    // Each gram names a range of `words`, which is `pairs` without the gram keys. A
    // gram naming one word is most of the table on a library of invented words, and
    // its entry is then one word id rather than a start and a length.
    let mut grams = Grams::default();
    let mut at = 0usize;
    while at < pairs.len() {
        let gram = pairs[at].0;
        let mut end = at + 1;
        while end < pairs.len() && pairs[end].0 == gram {
            end += 1;
        }
        // Where this gram's run starts, which is also where its own entry in the
        // table of ends begins.
        grams.at.insert(gram, grams.next.len() as u32);
        grams.words.extend(pairs[at..end].iter().map(|(_, id)| *id));
        // The table of ends is one entry per word and reads as a run: the entry
        // under a word's place is the end of that word's gram, so the last word of a
        // gram carries the end of the run.
        let run_end = grams.words.len() as u32;
        grams.next.resize(run_end as usize, run_end);
        at = end;
    }
    // Sorted by gram and then row, so a gram names a run of rows with no vocabulary
    // behind it.
    solo.sort_unstable();
    solo.dedup();
    (grams, solo, order)
}

/// Which entries of a solo table hold each run, so a run is a contiguous range.
fn runs_at(solo: &Solo) -> HashMap<u64, (u32, u32)> {
    let mut at: HashMap<u64, (u32, u32)> = HashMap::new();
    let mut now = 0u32;
    while now < solo.len() as u32 {
        let gram = solo[now as usize].0;
        let start = now;
        while now < solo.len() as u32 && solo[now as usize].0 == gram {
            now += 1;
        }
        at.insert(gram, (start, now - start));
    }
    at
}

impl InvertedIndex {
    /// The words one gram names, as a range of the table. A start past the end names
    /// nothing, which is an empty range rather than a clamped one.
    fn gram_words(grams: &Grams, at: u32) -> Range<usize> {
        let start = at as usize;
        let words = grams.words.len();
        if start >= words {
            return words..words;
        }
        let end = grams.next.get(start).map_or(words, |next| *next as usize);
        start..end.clamp(start, words)
    }

    /// Whether the gram table is in place, which an edit after the last query leaves
    /// undone until a term short enough to need it arrives.
    pub fn is_laid_out(&self) -> bool {
        self.grams.is_some()
    }

    /// Whether a query reads the laid-out tables at all: a term of a `WINDOW` or more
    /// is answered from `runs`, which an edit keeps current, so only a shorter one
    /// waits for the layout.
    pub fn is_read_by(&self, query: &str) -> bool {
        query.split_whitespace().any(|term| term.len() < WINDOW)
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
        let mut marked = vec![false; self.len];
        let mut unmarked = self.len;
        let mut any = false;
        let wanted = gram_key(term.as_bytes());
        if let Some(grams) = &self.grams {
            if let Some(&at) = grams.at.get(&wanted) {
                let ids: Vec<u32> = grams.words[Self::gram_words(grams, at)].to_vec();
                for id in &ids {
                    let Some(&(start, count)) = self.per_word.get(*id as usize) else {
                        continue;
                    };
                    let rows = &self.flat[start as usize..(start + count) as usize];
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
        }
        // A word held by a single row is a pair of run and row, so its rows come
        // straight out of the solo table with no vocabulary to walk.
        if unmarked > 0
            && let Some(at) = self.solo_at.as_ref().and_then(|at| at.get(&wanted))
        {
            let rows = &self.solo[at.0 as usize..(at.0 + at.1) as usize];
            for (_, row) in rows {
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
        self.grams.as_ref().map_or(0, |grams| grams.at.len())
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
        let widest = idx
            .grams
            .as_ref()
            .and_then(|grams| grams.at.keys().map(|gram| gram >> 32).max());
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
