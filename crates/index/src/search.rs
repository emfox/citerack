//! The searchable text of a library, built once and queried many times.

use citerack_bib::Library;

use crate::fold::fold;
use crate::inverted::InvertedIndex;
use crate::matcher::{FieldRef, Matcher};

/// Which fields a search looks at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchOptions {
    /// Normalised field names to include. Empty means every field.
    pub fields: Vec<String>,
    /// Whether the citation key and entry type are searched too.
    pub include_key: bool,
    /// Stop after this many hits; `None` means no limit.
    pub limit: Option<usize>,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            fields: Vec::new(),
            include_key: true,
            limit: None,
        }
    }
}

/// The rows a query matched, in the order the library holds them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchResult {
    /// Entry ids, in file order.
    pub rows: Vec<usize>,
    /// How many rows matched before `limit` was applied.
    pub total: usize,
}

/// The folded text of every entry, arranged so a query reads its own matches.
pub struct Searcher {
    columns: Vec<Column>,
    fold_key: bool,
}

/// One field's folded values, one per entry, with `None` where an entry lacks it.
#[derive(Clone)]
struct Column {
    field: FieldRef,
    name: String,
    values: Vec<Option<String>>,
    inverted: InvertedIndex,
}

impl Searcher {
    /// Folds every searched field of every entry and indexes it. This is the only
    /// pass that costs time proportional to the text, so it runs before a window
    /// opens; a query afterwards reads the candidates the index names.
    pub fn build(library: &Library, options: &SearchOptions) -> Self {
        let mut names: Vec<String> = Vec::new();
        for entry in library.entries() {
            for field in &entry.fields {
                let name = field.name.as_str().to_owned();
                let wanted = options.fields.is_empty() || options.fields.contains(&name);
                if wanted && !names.contains(&name) {
                    names.push(name);
                }
            }
        }
        names.sort();

        let n = library.len();
        let mut columns = Vec::new();
        if options.include_key {
            let values = library
                .entries()
                .iter()
                .map(|e| Some(fold(&e.key)))
                .collect();
            columns.push(Column::new(FieldRef::Key, String::from("key"), values));
            let values = library
                .entries()
                .iter()
                .map(|e| Some(fold(e.kind.as_str())))
                .collect();
            columns.push(Column::new(FieldRef::Kind, String::from("type"), values));
        }
        for (idx, name) in names.iter().enumerate() {
            let mut values = vec![None; n];
            for (row, entry) in library.entries().iter().enumerate() {
                if let Some(raw) = entry.field(name) {
                    values[row] = Some(fold(raw));
                }
            }
            columns.push(Column::new(FieldRef::Named(idx), name.clone(), values));
        }
        Self {
            columns,
            fold_key: options.include_key,
        }
    }

    /// The field names this searcher covers, excluding key and type.
    pub fn field_names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self
            .columns
            .iter()
            .filter_map(|c| match c.field {
                FieldRef::Named(_) => Some(c.name.as_str()),
                FieldRef::Key | FieldRef::Kind => None,
            })
            .collect();
        names.dedup();
        names
    }

    /// One field's folded text for one row, whether the row holds it or not. A
    /// name the index does not cover is a field no entry had when it was built.
    pub fn value(&self, entry: usize, name: &str) -> Option<Option<&str>> {
        let column = self
            .columns
            .iter()
            .find(|c| matches!(c.field, FieldRef::Named(_)) && c.name == name)?;
        Some(column.values.get(entry)?.as_deref())
    }

    /// This searcher with one field of one entry replaced, so an edit costs the
    /// text of the entry rather than the text of the library.
    pub fn replace_field(&self, entry: usize, name: &str, text: &str) -> Option<Self> {
        let at = self
            .columns
            .iter()
            .position(|c| matches!(c.field, FieldRef::Named(_)) && c.name == name)?;
        let folded = fold(text);
        let mut columns = self.columns[..at].to_vec();
        let mut column = self.columns[at].clone();
        *column.values.get_mut(entry)? = Some(folded);
        column.reindex(entry);
        columns.push(column);
        columns.extend_from_slice(&self.columns[at + 1..]);
        Some(Self {
            columns,
            fold_key: self.fold_key,
        })
    }

    /// Every entry that holds all of the query's terms; an exact matcher keeps
    /// only the entry whose key is the query, and a key lookup ignores the
    /// configured fields, as a citation key is always searchable.
    pub fn search(&self, matcher: &Matcher, limit: Option<usize>) -> SearchResult {
        let rows = if matcher.is_exact() {
            self.exact_rows(matcher)
        } else if matcher.is_empty() {
            (0..self.row_count()).map(|row| row as u32).collect()
        } else {
            self.matching_rows(matcher).unwrap_or_default()
        };
        finish(rows, limit)
    }

    /// The rows holding every term, narrowed by the rarest term first. A query costs
    /// the lists it names, never a walk over the library, and a term nobody wrote
    /// ends the search before a single row is read.
    fn matching_rows(&self, matcher: &Matcher) -> Option<Vec<u32>> {
        let mut lists: Vec<Vec<u32>> = matcher
            .terms()
            .iter()
            .map(|term| self.rows_holding(term))
            .collect();
        if lists.iter().any(Vec::is_empty) {
            return None;
        }
        // The rarest term decides who is examined, and the rest are asked about
        // those rows only, so nothing outside the shortest list is ever read.
        lists.sort_unstable_by_key(Vec::len);
        let mut rows = lists.remove(0);
        for other in &lists {
            rows = intersect(&rows, other);
            if rows.is_empty() {
                return None;
            }
        }
        Some(rows)
    }

    /// The rows holding `term`, in ascending order and each once, merged through
    /// one bitmap: a term may name the same row in several fields, and marking it
    /// costs less than sorting the repeats.
    fn rows_holding(&self, term: &str) -> Vec<u32> {
        let n = self.row_count();
        let mut marked = vec![false; n];
        let mut any = false;
        for column in &self.columns {
            for row in column.inverted.rows_with(term) {
                if let Some(slot) = marked.get_mut(row as usize) {
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

    /// The rows whose citation key is exactly the query.
    fn exact_rows(&self, matcher: &Matcher) -> Vec<u32> {
        if !self.fold_key {
            return Vec::new();
        }
        let Some(key) = self.columns.first() else {
            return Vec::new();
        };
        let Some(term) = matcher.terms().first() else {
            return Vec::new();
        };
        key.inverted
            .rows_with(term)
            .into_iter()
            .filter(|row| {
                key.values
                    .get(*row as usize)
                    .and_then(Option::as_deref)
                    .is_some_and(|text| text == term)
            })
            .collect()
    }

    /// Prints what every column holds for each term of a query, so a search that
    /// returns too much or too little is traced to a column. The query is folded
    /// and cut first, as a search does it.
    pub fn explain(&self, query: &str) {
        for term in Matcher::new(query).terms() {
            println!("term {term:?}");
            for column in &self.columns {
                println!(
                    "  {:<14} {:>7} rows  {:>8} runs  {:>7} words",
                    column.name,
                    column.inverted.rows_with(term).len(),
                    column.inverted.run_count(),
                    column.inverted.word_count()
                );
            }
        }
    }

    /// Prints how many postings each column holds, which is where the build's
    /// memory goes and which column a saving would have to come from.
    pub fn report(&self) {
        // Column, postings, runs, words, grams, and the gram entries a one-row word
        // costs. The last two are where a wide vocabulary puts its memory.
        let mut rows: Vec<(&str, [usize; 5])> = self
            .columns
            .iter()
            .map(|c| {
                (
                    c.name.as_str(),
                    [
                        c.inverted.posting_count(),
                        c.inverted.run_count(),
                        c.inverted.word_count(),
                        c.inverted.gram_count(),
                        c.inverted.solo_count(),
                    ],
                )
            })
            .collect();
        rows.sort_unstable_by_key(|(_, counts)| std::cmp::Reverse(counts[0]));
        let total: usize = rows.iter().map(|(_, counts)| counts[0]).sum();
        println!(
            "{:<16} {:>10} {:>8} {:>8} {:>8} {:>10}",
            "", "postings", "runs", "words", "grams", "solo"
        );
        for (name, [postings, runs, words, grams, solo]) in rows {
            println!(
                "{name:<16} {postings:>10} {runs:>8} {words:>8} {grams:>8} {solo:>10}  {:>5.1}%",
                postings as f64 * 100.0 / total as f64
            );
        }
        println!("{:<16} {total:>10}", "total");
    }

    /// How many postings the whole index holds, for a memory report.
    pub fn posting_total(&self) -> usize {
        self.columns
            .iter()
            .map(|c| c.inverted.posting_count())
            .sum()
    }

    /// How many distinct runs the whole index holds, for a memory report.
    pub fn run_total(&self) -> usize {
        self.columns.iter().map(|c| c.inverted.run_count()).sum()
    }

    /// The rows a full scan of every cell would return, so a test can hold the
    /// index to the answer it has to reproduce.
    pub fn scan_rows(&self, matcher: &Matcher) -> Vec<usize> {
        (0..self.row_count())
            .filter(|row| {
                self.columns.iter().any(|column| {
                    column
                        .values
                        .get(*row)
                        .and_then(Option::as_deref)
                        .is_some_and(|text| matcher.matches(text))
                })
            })
            .collect()
    }

    /// How many rows the index covers.
    fn row_count(&self) -> usize {
        self.columns.first().map_or(0, |c| c.values.len())
    }
}

impl Column {
    fn new(field: FieldRef, name: String, values: Vec<Option<String>>) -> Self {
        let inverted = InvertedIndex::build(&values);
        Self {
            field,
            name,
            values,
            inverted,
        }
    }

    /// Re-indexes one row, since the rest of the column did not change.
    fn reindex(&mut self, entry: usize) {
        self.inverted = self.inverted.with_row(entry, self.values[entry].as_deref());
    }
}

/// The rows both lists hold, each walked once.
fn intersect(a: &[u32], b: &[u32]) -> Vec<u32> {
    let mut out = Vec::new();
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

/// Counts every match, then applies the limit to the row list alone.
fn finish(rows: Vec<u32>, limit: Option<usize>) -> SearchResult {
    let total = rows.len();
    let keep = limit.map_or(total, |limit| total.min(limit));
    SearchResult {
        rows: rows[..keep].iter().map(|row| *row as usize).collect(),
        total,
    }
}
