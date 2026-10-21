//! The searchable text of a library, built once and queried many times.

use citerack_bib::Library;

use crate::fold::fold;
use crate::matcher::{FieldRef, Hit, Matcher};

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

/// The folded text of every entry, arranged so a query touches each byte once.
pub struct Searcher {
    columns: Vec<Column>,
}

/// One field's folded values, one per entry, with `None` where an entry lacks it.
struct Column {
    field: FieldRef,
    name: String,
    values: Vec<Option<String>>,
}

impl Searcher {
    /// Folds every searched field of every entry. This is the only pass that
    /// costs time proportional to the text, so it runs before a window opens.
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
            columns.push(Column {
                field: FieldRef::Key,
                name: String::from("key"),
                values,
            });
            let values = library
                .entries()
                .iter()
                .map(|e| Some(fold(e.kind.as_str())))
                .collect();
            columns.push(Column {
                field: FieldRef::Kind,
                name: String::from("type"),
                values,
            });
        }
        for (idx, name) in names.iter().enumerate() {
            let mut values = vec![None; n];
            for (row, entry) in library.entries().iter().enumerate() {
                if let Some(raw) = entry.field(name) {
                    values[row] = Some(fold(raw));
                }
            }
            columns.push(Column {
                field: FieldRef::Named(idx),
                name: name.clone(),
                values,
            });
        }
        Self { columns }
    }

    /// The field names this searcher covers, excluding key and type.
    pub fn field_names(&self) -> Vec<&str> {
        self.columns
            .iter()
            .filter(|c| matches!(c.field, FieldRef::Named(_)))
            .map(|c| c.name.as_str())
            .collect()
    }

    /// Every entry whose any field matches, with the best hit in that entry.
    pub fn search(&self, matcher: &Matcher, limit: Option<usize>) -> SearchResult {
        let rows = self.columns.first().map_or(0, |c| c.values.len());
        let mut out = Vec::new();
        let mut total = 0usize;
        for row in 0..rows {
            let mut best: Option<Hit> = None;
            for column in &self.columns {
                let Some(Some(text)) = column.values.get(row) else {
                    continue;
                };
                if let Some(hit) = matcher.matches(column.field, text) {
                    best = Some(match best {
                        Some(prev) if prev.end - prev.start >= hit.end - hit.start => prev,
                        _ => hit,
                    });
                }
            }
            if best.is_some() {
                total += 1;
                if limit.is_none_or(|l| out.len() < l) {
                    out.push(row);
                }
            }
        }
        SearchResult { rows: out, total }
    }
}
