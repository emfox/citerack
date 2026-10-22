//! A [`Session`] is what a front end holds: the parsed library, the search index
//! built from it, and the row order on display. None of it touches a window, so
//! a benchmark builds one as easily as a screen does.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use citerack_bib::{
    EditError, Entry, EntryId, EntryState, Library, ParseError, ParseOptions, Pattern,
    PatternError, WriteOptions, parse_str, unique, write_str,
};
use citerack_index::{Pair, SearchOptions, SearchResult, Searcher, SortKey, SortSpec};

/// How many changes the history keeps, which is more than a user retraces.
const HISTORY: usize = 100;

/// Which field each facet list narrows the table by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Facet {
    /// The first author's surname, as the Author column shows it.
    Author,
    /// The journal, else the book title, else the publisher, as the Venue column
    /// shows it: a reader asking "where" wants one list, not three.
    Venue,
    /// The year.
    Year,
    /// The entry type.
    Kind,
}

impl Facet {
    /// The lists the window draws, in the order it draws them.
    pub const ALL: [Self; 4] = [Self::Author, Self::Venue, Self::Year, Self::Kind];

    /// The heading over the list.
    pub fn heading(self) -> &'static str {
        match self {
            Self::Author => "Author",
            Self::Venue => "Journal / Book",
            Self::Year => "Year",
            Self::Kind => "Type",
        }
    }

    /// A row's value for this list, or `None` where the entry leaves it empty: an
    /// entry with no year would otherwise be offered as a group called "".
    pub fn value_of(self, library: &Library, row: usize) -> Option<String> {
        Facet::plain(self, library.entries().get(row)?)
    }

    /// The value this list files an entry under, in the spelling the table uses.
    pub fn plain(self, entry: &citerack_bib::Entry) -> Option<String> {
        let raw = match self {
            Self::Author => entry.field("author").map(first_author),
            Self::Venue => entry
                .field("journal")
                .or_else(|| entry.field("booktitle"))
                .or_else(|| entry.field("publisher"))
                .map(str::to_owned),
            Self::Year => entry.field("year").map(str::to_owned),
            Self::Kind => Some(entry.kind.as_str().to_ascii_lowercase()),
        }?;
        let value = raw.trim();
        (!value.is_empty()).then(|| value.to_owned())
    }
}

/// One line of a facet list: the value, and how many rows picking it leaves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FacetValue {
    /// The value as the table shows it, which is also what is compared.
    pub label: String,
    /// How many rows the table would hold with this value picked.
    pub count: usize,
}

/// How many lists there are, which is the width of the picked-value array.
const FACET_COUNT: usize = 4;

/// The value picked in each facet list, held by index into `Facet::ALL`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Facets {
    picked: [Option<String>; FACET_COUNT],
}

impl Facets {
    /// The value picked in one list.
    pub fn of(&self, facet: Facet) -> Option<&str> {
        self.picked[Self::slot(facet)].as_deref()
    }

    /// Picks a value, or lets the list go when it is already picked.
    pub fn toggle(&mut self, facet: Facet, value: &str) {
        let slot = &mut self.picked[Self::slot(facet)];
        if slot.as_deref() == Some(value) {
            *slot = None;
        } else {
            *slot = Some(value.to_owned());
        }
    }

    /// Lets one list go.
    pub fn clear(&mut self, facet: Facet) {
        self.picked[Self::slot(facet)] = None;
    }

    /// How many lists are picked.
    pub fn len(&self) -> usize {
        self.picked.iter().filter(|picked| picked.is_some()).count()
    }

    /// Whether no list is picked.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The same lists, less one, which is how a list counts itself as if it were
    /// not picked.
    pub fn without(&self, facet: Facet) -> Self {
        let mut facets = self.clone();
        facets.clear(facet);
        facets
    }

    /// Whether a row passes every list, which is the whole of what a facet does.
    pub fn keeps(&self, library: &Library, row: usize) -> bool {
        let entry = library.entries().get(row);
        let lists: &[Facet] = &Facet::ALL;
        lists
            .iter()
            .zip(&self.picked)
            .all(|(facet, picked)| match (picked, entry) {
                (None, _) => true,
                (Some(picked), Some(entry)) => facet.plain(entry).as_deref() == Some(picked),
                (Some(_), None) => false,
            })
    }

    /// Which list position this facet keeps its value in. An exhaustive match,
    /// since a fallback here would quietly erase another list's pick.
    pub fn slot(facet: Facet) -> usize {
        match facet {
            Facet::Author => 0,
            Facet::Venue => 1,
            Facet::Year => 2,
            Facet::Kind => 3,
        }
    }
}

/// The first author as `Last` when the list is `Last, First`, else as written.
fn first_author(authors: &str) -> String {
    let first = authors.split(" and ").next().unwrap_or(authors).trim();
    match first.split_once(',') {
        Some((last, _)) => last.trim().to_owned(),
        None => first.to_owned(),
    }
}

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
    /// The value picked in each facet list; `None` leaves that list narrowed to
    /// nothing rather than setting a value that no row holds.
    facets: Facets,
    /// The lists a window draws, kept between frames: counting them walks the
    /// table, and a frame that walked it every time would cost seconds at 100k.
    facet_lists: Vec<Vec<FacetValue>>,
    /// What `facet_lists` was counted from, since an edit can change a value.
    facet_seen: u64,
    /// Bumped whenever the table or the text behind it changes, which is what
    /// tells the facet lists their counts are out of date.
    view_mark: u64,
    /// What each change replaced, oldest first, and what it left behind.
    past: Vec<Change>,
    future: Vec<Change>,
    /// When the last edit landed, which is what a pause is measured from.
    last_edit: Option<Instant>,
    /// The pattern a generated key follows, kept even while the box holds text the
    /// pattern cannot read, so a typo does not throw the setting away.
    key_pattern: Pattern,
    /// The duplicate scan for the current library, patched by an edit.
    duplicates: Option<Vec<Pair>>,
    /// Which entry first holds each key the scan pairs on, kept beside the scan so
    /// an edit patches the pairs it moves rather than the whole library.
    duplicate_keys: Option<citerack_index::Duplicates>,
}

/// One change: a run of edits with no pause long enough to be a separate step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    /// The entries as they were before the change, which is empty for an import.
    pub before: Vec<EntryState>,
    /// The entries as the change left them.
    pub after: Vec<EntryState>,
}

impl Change {
    /// The row an import added, when the change was one: a change that recorded
    /// nothing to go back to can only have brought an entry in.
    pub fn imported(&self) -> Option<usize> {
        if !self.before.is_empty() {
            return None;
        }
        match self.after.as_slice() {
            [state] => Some(state.entry),
            _ => None,
        }
    }
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
            facets: Facets::default(),
            facet_lists: vec![Vec::new(); FACET_COUNT],
            facet_seen: 0,
            view_mark: 1,
            past: Vec::new(),
            future: Vec::new(),
            last_edit: None,
            key_pattern: Pattern::parse(citerack_bib::DEFAULT_KEY_PATTERN)
                .expect("the default pattern is one"),
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

    /// The values one facet list offers, each with how many rows it would leave
    /// on the table, counted once and then held until the table changes.
    pub fn facet_values(&mut self, facet: Facet) -> &[FacetValue] {
        if self.facet_seen != self.view_mark {
            self.count_facets();
        }
        let slot = Facets::slot(facet);
        &self.facet_lists[slot]
    }

    /// Counts every list at once, which is one walk of the table rather than four.
    fn count_facets(&mut self) {
        for facet in Facet::ALL {
            let counted = self.count_facet(facet);
            self.facet_lists[Facets::slot(facet)] = counted;
        }
        self.facet_seen = self.view_mark;
    }

    /// The values of one list, as the other facets and this one's own pick leave
    /// it: a held value is kept in the list so that pressing it again lets it go.
    fn count_facet(&self, facet: Facet) -> Vec<FacetValue> {
        let others = self.facets.without(facet);
        let library = &self.library;
        let picked = self.facets.of(facet);
        let mut tally: Vec<(String, usize)> = Vec::new();
        for row in 0..library.len() {
            if !others.keeps(library, row) {
                continue;
            }
            let Some(value) = facet.value_of(library, row) else {
                continue;
            };
            // The table as the other lists leave it, plus the rows this list's own
            // pick holds back: without the second, the pressed value would vanish.
            let listed = self.view.contains(&row);
            if listed || picked == Some(value.as_str()) {
                match tally.iter_mut().find(|(held, _)| *held == value) {
                    Some((_, count)) => *count += 1,
                    None => tally.push((value, 1)),
                }
            }
        }
        // Biggest first, since the group a user wants is usually the one the
        // library holds most of, and a count is read before a name is.
        tally.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        tally
            .into_iter()
            .map(|(label, count)| FacetValue { label, count })
            .collect()
    }

    /// The value picked in one facet list.
    pub fn picked(&self, facet: Facet) -> Option<&str> {
        self.facets.of(facet)
    }

    /// Picks a value in a facet list, or lets it go when it is already picked,
    /// and narrows the table to what the lists now allow.
    pub fn toggle_facet(&mut self, facet: Facet, value: &str) {
        self.facets.toggle(facet, value);
        self.refresh_view();
    }

    /// Lets one list go, which puts back the rows it was hiding.
    pub fn clear_facet(&mut self, facet: Facet) {
        self.facets.clear(facet);
        self.refresh_view();
    }

    /// Lets every list go at once.
    pub fn clear_facets(&mut self) {
        self.facets = Facets::default();
        self.refresh_view();
    }

    /// How many lists are narrowing the table.
    pub fn facet_count(&self) -> usize {
        self.facets.len()
    }

    /// Rebuilds what the table shows from the search result and the filter.
    fn refresh_view(&mut self) {
        self.view.clone_from(&self.rows);
        if self.only_duplicates {
            self.view = self.duplicates_view();
        }
        if !self.facets.is_empty() {
            let facets = self.facets.clone();
            let library = &self.library;
            self.view.retain(|row| facets.keeps(library, *row));
        }
        self.view_mark += 1;
    }

    /// The rows on the table that are in a duplicate pair, in display order.
    fn duplicates_view(&mut self) -> Vec<usize> {
        self.duplicates();
        let mut involved = vec![false; self.library.len()];
        for pair in self.duplicates.iter().flatten() {
            involved[pair.first] = true;
            involved[pair.second] = true;
        }
        self.view
            .iter()
            .copied()
            .filter(|row| involved.get(*row).copied().unwrap_or(false))
            .collect()
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

    /// Files an entry the file did not hold, or says why it cannot, and keys it so
    /// that it names nothing already in the library. The whole import is one undo
    /// step, and a refused entry leaves the library untouched.
    pub fn import(&mut self, entry: Entry) -> Result<usize, EditError> {
        self.settle_all();
        let key = self.key_for(&entry.key);
        let mut entry = entry;
        entry.key = key.into();
        let id = self.library.append(entry)?;
        let row = id.0;
        self.after_append(row);
        Ok(row)
    }

    /// A key that names nothing yet: the one asked for, or that one and a letter,
    /// which is how a second paper by the same author in the same year is filed.
    pub fn key_for(&self, wanted: &str) -> String {
        if self.library.keyed(wanted) == 0 {
            return wanted.to_owned();
        }
        for suffix in b'a'..=b'z' {
            let candidate = format!("{wanted}{}", suffix as char);
            if self.library.keyed(&candidate) == 0 {
                return candidate;
            }
        }
        // A library that already holds every letter of this key takes a number,
        // which a reader will never see unless they imported 27 of one year.
        for suffix in 27..=999 {
            let candidate = format!("{wanted}{suffix}");
            if self.library.keyed(&candidate) == 0 {
                return candidate;
            }
        }
        wanted.to_owned()
    }

    /// Puts the sort key, the index and the table back after a row arrived. An
    /// import is rare, so the whole index is rebuilt rather than patched.
    fn after_append(&mut self, row: usize) {
        self.duplicates = None;
        self.duplicate_keys = None;
        self.stale = true;
        self.sort_keys.push(SortKey::of(&self.library, row));
        self.rows.push(row);
        self.refresh_view();
    }

    /// Files an entry an undo took back, so a redo can bring it in again.
    fn restored_import(&mut self, state: &EntryState) -> Option<usize> {
        let id = self.library.append(state.model.clone()).ok()?;
        let row = id.0;
        self.after_append(row);
        Some(row)
    }

    /// Takes back the entry the last undo reversed, which is only ever the last
    /// row since nothing follows an import.
    fn unimport(&mut self, row: usize) {
        if self.library.unappend(EntryId(row)).is_err() {
            return;
        }
        self.sort_keys.truncate(row);
        self.rows.retain(|held| *held != row);
        self.duplicates = None;
        self.duplicate_keys = None;
        self.stale = true;
        self.refresh_view();
    }

    /// Adds a field at the end of an entry.
    pub fn add_field(&mut self, entry: usize, name: &str, value: &str) -> Result<(), EditError> {
        if self.library.append_field(EntryId(entry), name, value)? {
            self.touched(entry);
        }
        Ok(())
    }

    /// Writes one field across several entries, returning how many changed and how
    /// many were refused. The whole batch is one undo step; a field the entry does
    /// not hold is added, and an entry that refuses the value is left as it was.
    pub fn set_field_in(&mut self, entries: &[usize], name: &str, value: &str) -> (usize, usize) {
        let (mut written, mut refused) = (0, 0);
        for &entry in entries {
            let done = match self.field_index(entry, name) {
                Some(field) => self.set_field(entry, field, value),
                None => self.add_field(entry, name, value),
            };
            match done {
                Ok(()) => written += 1,
                Err(_) => refused += 1,
            }
        }
        if written > 0 {
            self.settle();
        }
        (written, refused)
    }

    /// Where a field sits in an entry, compared as BibTeX compares names.
    fn field_index(&self, entry: usize, name: &str) -> Option<usize> {
        self.library
            .entries()
            .get(entry)?
            .fields
            .iter()
            .position(|field| field.name.as_str().eq_ignore_ascii_case(name))
    }

    /// The pattern a generated key follows.
    pub fn key_pattern(&self) -> String {
        self.key_pattern.as_str()
    }

    /// Sets the pattern, refusing one that names no field or cannot be written.
    pub fn set_key_pattern(&mut self, pattern: &str) -> Result<(), PatternError> {
        self.key_pattern = Pattern::parse(pattern)?;
        Ok(())
    }

    /// The key the pattern makes for one entry as the library stands now.
    pub fn proposed_key(&self, entry: usize) -> String {
        self.free_key(
            self.key_pattern.key_of(&self.library, EntryId(entry)),
            entry,
        )
    }

    /// The key the pattern makes for the entry at a displayed row, letter and
    /// all, read through the table's own order so a column and the control
    /// beside it agree about what a press would write.
    pub fn proposed_key_at(&self, row: usize) -> Option<String> {
        let entry = *self.view.get(row)?;
        Some(self.proposed_key_of(entry))
    }

    /// The keys the pattern makes for the whole table, as the Generated column
    /// shows them. A key another entry already holds, or an earlier row claimed,
    /// keeps its letter, so the column is what a press would write.
    pub fn proposed_keys(&self) -> Vec<String> {
        let mut claimed: Vec<String> = Vec::with_capacity(self.view.len());
        self.view
            .iter()
            .map(|&entry| {
                let base = self.key_pattern.key_of(&self.library, EntryId(entry));
                let free = self.free_among(&base, entry, &claimed);
                claimed.push(free.clone());
                free
            })
            .collect()
    }

    /// The key the pattern makes for one entry, given the keys every earlier
    /// entry on the table already claims.
    fn proposed_key_of(&self, entry: usize) -> String {
        let mut claimed: Vec<String> = Vec::new();
        for &earlier in self.view.iter() {
            if earlier == entry {
                return self.free_among(
                    &self.key_pattern.key_of(&self.library, EntryId(entry)),
                    entry,
                    &claimed,
                );
            }
            let base = self.key_pattern.key_of(&self.library, EntryId(earlier));
            claimed.push(self.free_among(&base, earlier, &claimed));
        }
        self.proposed_key(entry)
    }

    /// Calls an entry something else, keeping its type and every field. This is
    /// the one edit that reaches every row, since a key is how a citation names
    /// an entry and two entries cannot share one.
    pub fn generate_key(&mut self, entry: usize) -> Result<(), EditError> {
        let key = self.proposed_key(entry);
        self.set_key(entry, &key)
    }

    /// Gives every entry the key the pattern makes for its own fields, and the
    /// letter that keeps it apart from a key an earlier entry claimed, so the
    /// column and a press agree and no key is ever pointing at two entries.
    pub fn generate_keys(&mut self) -> Result<usize, EditError> {
        let mut written = 0;
        let mut claimed: Vec<String> = Vec::new();
        for row in self.rows.clone() {
            let base = self.key_pattern.key_of(&self.library, EntryId(row));
            let free = self.free_among(&base, row, &claimed);
            claimed.push(free.clone());
            let current = self.library.entries().get(row).map(|e| e.key.to_string());
            if current.as_deref() != Some(free.as_str()) {
                self.set_key(row, &free)?;
                written += 1;
            }
        }
        Ok(written)
    }

    /// A key from `base` that no other entry holds and no earlier generated key
    /// claimed.
    fn free_among(&self, base: &str, entry: usize, claimed: &[String]) -> String {
        let library = &self.library;
        unique(base, |candidate| {
            claimed.iter().any(|taken| taken == candidate)
                || library
                    .find_by_key(candidate)
                    .is_some_and(|id| id.0 != entry)
        })
    }

    /// A key from `base` that no other entry holds.
    fn free_key(&self, base: String, entry: usize) -> String {
        let library = &self.library;
        unique(&base, |candidate| {
            library
                .find_by_key(candidate)
                .is_some_and(|id| id.0 != entry)
        })
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
        if let Some(row) = change.imported() {
            // An import left no state to put back, since the row was not there.
            self.unimport(row);
        } else if self.library.restore(&change.before).is_err() {
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
        if change.imported().is_some() {
            // An import that was taken back is put back by filing the entry again,
            // which is the only way a row that never had bytes can return.
            let row = self.restored_import(&change.after[0]);
            match row {
                Some(row) => {
                    let state = self.library.state_of(row);
                    self.past.push(Change {
                        before: Vec::new(),
                        after: state.into_iter().collect(),
                    });
                    Some(what)
                }
                None => {
                    self.future.push(change);
                    None
                }
            }
        } else if self.library.restore(&change.after).is_err() {
            self.future.push(change);
            None
        } else {
            self.library.seal();
            self.refreshed(&change.after);
            self.past.push(change);
            Some(what)
        }
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
            // An entry that arrived has no state to go back to, so it is recorded
            // as an empty before, which is what makes an import undoable.
            if let Some(state) = self
                .library
                .newest_added()
                .and_then(|row| self.library.state_of(row))
            {
                self.push(Change {
                    before: Vec::new(),
                    after: vec![state],
                });
                return;
            }
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
            // Nothing here came from the file, so the change was an entry arriving.
            return "an import";
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
        // The rows are back where they were, so the count did not move, but the
        // text a list is spelled from did.
        self.view_mark += 1;
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

    /// The text a save would write, without writing it, which is what a test and a
    /// preview both want.
    pub fn text(&self) -> String {
        write_str(&self.library, &WriteOptions::Preserve)
    }

    /// Writes to another path, leaving the original untouched; the default
    /// `Preserve` mode makes a save that changed nothing a no-op.
    pub fn save_as(&self, path: impl AsRef<Path>) -> Result<(), StoreError> {
        let text = write_str(&self.library, &WriteOptions::Preserve);
        std::fs::write(path, text)?;
        Ok(())
    }
}
