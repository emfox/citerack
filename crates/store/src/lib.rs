//! A [`Session`] is what a front end holds: the parsed library, the search index
//! built from it, and the row order on display. None of it touches a window, so
//! a benchmark builds one as easily as a screen does.

use std::path::{Path, PathBuf};

use citerack_bib::{Library, ParseError, ParseOptions, WriteOptions, parse_str, write_str};
use citerack_index::{SearchOptions, SearchResult, Searcher, SortKey, SortSpec};

/// A library open in memory, with its search index ready.
pub struct Session {
    path: PathBuf,
    library: Library,
    searcher: Searcher,
    sort_keys: Vec<SortKey>,
    order: Vec<usize>,
}

/// What went wrong while opening or saving.
#[derive(Debug)]
pub enum StoreError {
    /// The file could not be read.
    Io(std::io::Error),
    /// The file could not be parsed.
    Parse(ParseError),
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "cannot read the file: {err}"),
            Self::Parse(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for StoreError {}

impl From<std::io::Error> for StoreError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

impl From<ParseError> for StoreError {
    fn from(err: ParseError) -> Self {
        Self::Parse(err)
    }
}

impl Session {
    /// Reads, parses and indexes one file, in that order.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let path = path.as_ref().to_path_buf();
        let text = std::fs::read_to_string(&path)?;
        Self::from_text(path, &text, &ParseOptions { recover: true })
    }

    /// Builds a session from text already in memory, which a test can supply.
    pub fn from_text(
        path: PathBuf,
        text: &str,
        options: &ParseOptions,
    ) -> Result<Self, StoreError> {
        let library = parse_str(text, options)?;
        let searcher = Searcher::build(&library, &SearchOptions::default());
        let sort_keys = (0..library.len())
            .map(|i| SortKey::of(&library, i))
            .collect();
        let order = (0..library.len()).collect();
        Ok(Self {
            path,
            library,
            searcher,
            sort_keys,
            order,
        })
    }

    /// The file this session was opened from.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The parsed library.
    pub fn library(&self) -> &Library {
        &self.library
    }

    /// How many entries the file holds.
    pub fn len(&self) -> usize {
        self.library.len()
    }

    /// Whether the file held no entries.
    pub fn is_empty(&self) -> bool {
        self.library.is_empty()
    }

    /// The rows currently displayed, in display order.
    pub fn order(&self) -> &[usize] {
        &self.order
    }

    /// Filters to a query, keeping the current sort.
    pub fn search(&mut self, query: &str) -> SearchResult {
        let matcher = citerack_index::Matcher::new(query);
        let result = self.searcher.search(&matcher, None);
        self.order.clone_from(&result.rows);
        result
    }

    /// Restores every row and the file order.
    pub fn clear_search(&mut self) {
        self.order = (0..self.library.len()).collect();
    }

    /// Reorders the displayed rows, without touching the library.
    pub fn sort(&mut self, spec: &SortSpec) {
        citerack_index::sort_rows(&mut self.order, &self.sort_keys, spec);
    }

    /// Writes the library back to the path it came from.
    pub fn save(&self) -> Result<(), StoreError> {
        self.save_as(&self.path)
    }

    /// Writes to another path, leaving the original untouched; the default
    /// `Preserve` mode makes a save that changed nothing a no-op.
    pub fn save_as(&self, path: impl AsRef<Path>) -> Result<(), StoreError> {
        let text = write_str(&self.library, &WriteOptions::Preserve);
        std::fs::write(path, text)?;
        Ok(())
    }
}
