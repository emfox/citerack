//! A [`Session`] is what a front end holds: the parsed library, the search index
//! built from it, and the row order on display. None of it touches a window, so
//! a benchmark builds one as easily as a screen does.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use citerack_bib::{
    EditError, EntryId, EntryState, Library, ParseError, ParseOptions, WriteOptions, parse_str,
    write_str,
};
use citerack_index::{Pair, SearchOptions, SearchResult, Searcher, SortKey, SortSpec};

/// How many changes the history keeps, which is more than a user retraces.
const HISTORY: usize = 100;

/// A library open in memory, with its search index ready.
pub struct Session {
    path: PathBuf,
    library: Library,
    searcher: Searcher,
    sort_keys: Vec<SortKey>,
    /// The rows a query matched, or every row in file order, in display order.
    rows: Vec<usize>,
    /// What the table shows: `rows`, or just the entries in a duplicate pair when
    /// the window asked for those. Kept apart from `rows` so turning the filter on
    /// and off does not have to remember which rows the search had picked.
    view: Vec<usize>,
    /// Whether the table is showing only the entries in a duplicate pair.
    only_duplicates: bool,
    /// Whether an edit left the index describing the file rather than the screen.
    stale: bool,
    /// What each change replaced, oldest first, and what it left behind.
    past: Vec<Change>,
    future: Vec<Change>,
    /// When the last edit landed, which is what a pause is measured from.
    last_edit: Option<Instant>,
    /// The duplicate scan for the current library, patched by an edit.
    duplicates: Option<Vec<Pair>>,
    /// Which entry first holds each key the scan pairs on, kept beside the scan so
    /// an edit patches the pairs it moves rather than the whole library.
    duplicate_keys: Option<citerack_index::Duplicates>,
}

/// One change: a run of edits with no pause long enough to be a separate step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    /// The entries as they were before the change.
    pub before: Vec<EntryState>,
    /// The entries as the change left them.
    pub after: Vec<EntryState>,
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
        let rows: Vec<usize> = (0..library.len()).collect();
        Ok(Self {
            path,
            library,
            searcher,
            sort_keys,
            view: rows.clone(),
            rows,
            only_duplicates: false,
            stale: false,
            past: Vec::new(),
            future: Vec::new(),
            last_edit: None,
            duplicates: None,
            duplicate_keys: None,
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
        &self.view
    }

    /// Shows only the entries that are in a duplicate pair, or stops doing so.
    /// The search and the sort stay as they were, so the filter can be turned off
    /// without the table losing its place in the library.
    pub fn show_only_duplicates(&mut self, only: bool) {
        self.only_duplicates = only;
        self.refresh_view();
    }

    /// Whether the table is showing only the duplicate rows.
    pub fn showing_only_duplicates(&self) -> bool {
        self.only_duplicates
    }

    /// Rebuilds what the table shows from the search result and the filter.
    fn refresh_view(&mut self) {
        if !self.only_duplicates {
            self.view.clone_from(&self.rows);
            return;
        }
        self.duplicates();
        let mut involved = vec![false; self.library.len()];
        for pair in self.duplicates.iter().flatten() {
            involved[pair.first] = true;
            involved[pair.second] = true;
        }
        self.view = self
            .rows
            .iter()
            .copied()
            .filter(|row| involved.get(*row).copied().unwrap_or(false))
            .collect();
    }

    /// Filters to a query, keeping the current sort.
    pub fn search(&mut self, query: &str) -> SearchResult {
        self.catch_up(query);
        let matcher = citerack_index::Matcher::new(query);
        let result = self.searcher.search(&matcher, None);
        self.rows.clone_from(&result.rows);
        self.refresh_view();
        result
    }

    /// Prints what the index holds for each term of a query, for a report.
    pub fn explain(&self, query: &str) {
        self.searcher.explain(query);
    }

    /// Prints how many postings each column holds, which is where the memory goes.
    pub fn report(&self) {
        self.searcher.report();
    }

    /// Restores every row and the file order.
    pub fn clear_search(&mut self) {
        self.rows = (0..self.library.len()).collect();
        self.refresh_view();
    }

    /// Reorders the displayed rows, without touching the library.
    pub fn sort(&mut self, spec: &SortSpec) {
        citerack_index::sort_rows(&mut self.rows, &self.sort_keys, spec);
        self.refresh_view();
    }

    /// The pairs of entries that look like the same work. The scan runs the first
    /// time it is asked for, and an edit patches it rather than dropping it, so a
    /// window may ask every frame without paying for a scan.
    pub fn duplicates(&mut self) -> &[Pair] {
        if self.duplicates.is_none() {
            let (mut pairs, keys) = citerack_index::Duplicates::scanned(&self.library);
            pairs.sort_unstable_by_key(|pair| (pair.first, pair.second, pair.reason as u8));
            self.duplicates = Some(pairs);
            self.duplicate_keys = Some(keys);
        }
        self.duplicates.as_deref().unwrap_or_default()
    }

    /// How many pairs of entries look like the same work.
    pub fn duplicate_count(&mut self) -> usize {
        self.duplicates().len()
    }

    /// The pairs one entry belongs to, with the key of the entry it paired with
    /// and what the two matched on, which is what a pane showing that entry needs.
    pub fn duplicates_of(&mut self, row: usize) -> Vec<(String, String)> {
        self.duplicates();
        self.duplicates
            .iter()
            .flatten()
            .filter(|pair| pair.first == row || pair.second == row)
            .map(|pair| {
                let other = if pair.first == row {
                    pair.second
                } else {
                    pair.first
                };
                let key = self
                    .library
                    .entries()
                    .get(other)
                    .map(|entry| entry.key.to_string())
                    .unwrap_or_default();
                (key, pair.reason.as_str().to_owned())
            })
            .collect()
    }

    /// Writes the library back to the path it came from.
    pub fn save(&mut self) -> Result<(), StoreError> {
        let path = self.path.clone();
        self.save_as(&path)?;
        // The bytes on disk are the model's now, so the window stops calling the
        // work unsaved and a second save has nothing new to write.
        self.settle_all();
        self.library.mark_saved();
        Ok(())
    }

    /// Whether anything has been edited since the file was opened.
    pub fn is_edited(&self) -> bool {
        self.library.is_edited()
    }

    /// The value of one field, as a save would write it.
    pub fn value(&self, entry: usize, field: usize) -> Option<&str> {
        self.library.field(EntryId(entry), field)
    }

    /// Replaces one field's value. The sort key for that row is refreshed here,
    /// since it is cheap; the search index is rebuilt before the next query, so a
    /// long edit session costs one build rather than one per keystroke.
    pub fn set_field(&mut self, entry: usize, field: usize, value: &str) -> Result<(), EditError> {
        if self.library.replace_field(EntryId(entry), field, value)? {
            self.touched(entry);
        }
        Ok(())
    }

    /// Adds a field at the end of an entry.
    pub fn add_field(&mut self, entry: usize, name: &str, value: &str) -> Result<(), EditError> {
        if self.library.append_field(EntryId(entry), name, value)? {
            self.touched(entry);
        }
        Ok(())
    }

    /// Renames an entry's citation key.
    pub fn set_key(&mut self, entry: usize, key: &str) -> Result<(), EditError> {
        if self.library.rename_key(EntryId(entry), key)? {
            self.touched(entry);
        }
        Ok(())
    }

    /// Puts one entry back the way the file had it.
    pub fn revert(&mut self, entry: usize) -> Result<(), EditError> {
        self.library.revert(EntryId(entry))?;
        self.touched(entry);
        Ok(())
    }

    /// Reverses the last change, and says what it was called; `None` when there
    /// is nothing left to reverse. Whatever is still being typed becomes a step
    /// first, so the first undo takes back what the screen just showed.
    pub fn undo(&mut self) -> Option<&'static str> {
        self.settle_all();
        let change = self.past.pop()?;
        let what = self.describe(&change);
        if self.library.restore(&change.before).is_err() {
            self.past.push(change);
            return None;
        }
        // The undo is a step of its own, so what it left is not folded into the
        // change the user has not typed yet.
        self.library.seal();
        self.refreshed(&change.before);
        self.future.push(change);
        Some(what)
    }

    /// Puts back the change the last undo reversed, after closing whatever the
    /// user has typed since.
    pub fn redo(&mut self) -> Option<&'static str> {
        self.settle_all();
        let change = self.future.pop()?;
        let what = self.describe(&change);
        if self.library.restore(&change.after).is_err() {
            self.future.push(change);
            return None;
        }
        self.library.seal();
        self.refreshed(&change.after);
        self.past.push(change);
        Some(what)
    }

    /// Whether there is a change to reverse.
    pub fn can_undo(&self) -> bool {
        !self.past.is_empty()
    }

    /// Whether there is a change to put back.
    pub fn can_redo(&self) -> bool {
        !self.future.is_empty()
    }

    /// Ends the change in progress, so the next edit becomes a step of its own.
    /// Only the rows this change touched are recorded, and a change that left
    /// them as it found them becomes no step at all.
    pub fn settle(&mut self) {
        let before = self.library.pending_before();
        if before.is_empty() {
            return;
        }
        let after: Vec<EntryState> = before
            .iter()
            .filter_map(|was| self.library.state_of(was.entry))
            .collect();
        self.library.seal();
        let untouched = after
            .iter()
            .zip(&before)
            .all(|(now, was)| now.text == was.text && now.model == was.model);
        if untouched {
            return;
        }
        self.push(Change { before, after });
    }

    /// Ends the change in progress after a clock reading, so a run of typing into
    /// one field is one step and a pause or a moved cursor starts another.
    pub fn settle_at(&mut self, at: Instant, within: Duration) {
        let Some(last) = self.last_edit else {
            self.settle();
            return;
        };
        if at.duration_since(last) > within {
            self.settle();
        }
    }

    /// Notes that an edit just happened, which is what the pause is measured from.
    pub fn edited_at(&mut self, at: Instant) {
        self.last_edit = Some(at);
    }

    /// Whatever change is still open becomes a step, which a save does before it
    /// writes.
    pub fn settle_all(&mut self) {
        self.settle();
        self.last_edit = None;
    }

    /// A name for a change, which is the status line's word for it.
    fn describe(&self, change: &Change) -> &'static str {
        let rows: Vec<usize> = change
            .after
            .iter()
            .map(|state| state.entry)
            .filter(|entry| self.library.span(EntryId(*entry)).is_some())
            .collect();
        if rows.len() > 1 {
            return "an edit to several entries";
        }
        let Some(row) = rows.first() else {
            return "an edit";
        };
        let before = change
            .before
            .iter()
            .find(|state| state.entry == *row)
            .map(|state| &state.model);
        let after = change
            .after
            .iter()
            .find(|state| state.entry == *row)
            .map(|state| &state.model);
        match (before, after) {
            (Some(before), Some(after)) if before.key != after.key => "renaming an entry",
            (Some(before), Some(after)) if before.fields.len() != after.fields.len() => {
                "adding a field"
            }
            _ => "editing a field",
        }
    }

    /// Records a finished change and drops any redo that no longer applies.
    fn push(&mut self, change: Change) {
        self.past.push(change);
        self.future.clear();
        // Oldest first, so the oldest goes when the history is longer than a user
        // could still be thinking about.
        if self.past.len() > HISTORY {
            self.past.remove(0);
        }
    }

    /// Brings the keys and the index back in line with entries a restore changed.
    fn refreshed(&mut self, states: &[EntryState]) {
        self.duplicates = None;
        self.duplicate_keys = None;
        self.stale = false;
        for state in states {
            self.reindexed(state.entry);
        }
        self.refresh_view();
    }

    /// Records that one row changed: its key, and the postings of every field of it
    /// in the index. Only the changed row can start or end a pair, so a scan already
    /// held is patched rather than dropped, which a user typing would feel.
    fn touched(&mut self, entry: usize) {
        if let Some(pairs) = &mut self.duplicates
            && let Some(keys) = &mut self.duplicate_keys
        {
            pairs.retain(|pair| pair.first != entry && pair.second != entry);
            pairs.extend(keys.patch(&self.library, entry));
            pairs.sort_unstable_by_key(|pair| (pair.first, pair.second, pair.reason as u8));
        }
        self.reindexed(entry);
        self.refresh_view();
    }

    /// Recomputes one row's sort key and replaces its words in each column, which
    /// costs the row rather than the library: an edit is a keystroke, so a rebuild
    /// here would be felt on a large library.
    fn reindexed(&mut self, entry: usize) {
        if let Some(key) = self.sort_keys.get_mut(entry) {
            *key = SortKey::of(&self.library, entry);
        }
        // The gram and solo tables name rows as well as words, so an edit drops them
        // and a search lays out the ones it needs, rather than every keystroke.
        let names: Vec<String> = self
            .searcher
            .field_names()
            .iter()
            .map(|name| (*name).to_owned())
            .collect();
        for name in &names {
            self.searcher
                .replace_field(entry, name, &self.text_of(entry, name));
        }
    }

    /// One field's text as the index should hold it, so a row that lost a field
    /// is indexed with an empty one rather than the value it no longer has.
    fn text_of(&self, entry: usize, name: &str) -> String {
        self.library
            .entries()
            .get(entry)
            .and_then(|e| e.field(name))
            .unwrap_or_default()
            .to_owned()
    }

    /// Brings the index up to the screen: the whole index after a restore, and the
    /// gram and solo tables after a run of edits. A search asks this first, so it
    /// answers from the text on display rather than the text the file had.
    fn catch_up(&mut self, query: &str) {
        if self.stale {
            self.searcher = Searcher::build(&self.library, &SearchOptions::default());
            self.stale = false;
            return;
        }
        self.searcher.settle(query);
    }

    /// Writes to another path, leaving the original untouched; the default
    /// `Preserve` mode makes a save that changed nothing a no-op.
    pub fn save_as(&self, path: impl AsRef<Path>) -> Result<(), StoreError> {
        let text = write_str(&self.library, &WriteOptions::Preserve);
        std::fs::write(path, text)?;
        Ok(())
    }
}
