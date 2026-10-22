//! Search and sort built once per library: which rows match a query, and which
//! rows a sort would put first. Neither answer needs a window, so both are
//! computed before one exists.

mod duplicate;
mod fold;
mod import;
mod inverted;
mod lookup;
mod matcher;
mod search;
mod sort;
mod token;

pub use duplicate::{Index as Duplicates, Keys, Pair, Reason, find as find_duplicates};
pub use fold::fold;
pub use import::{from_arxiv, from_crossref, keyed};
pub use inverted::{InvertedIndex, WINDOW};
pub use lookup::{Found, Identifier, first_surname, read as read_identifier};
pub use matcher::{FieldRef, Matcher};
pub use search::{SearchOptions, SearchResult, Searcher};
pub use sort::{SortField, SortKey, SortSpec};

/// The order a list of row indices should be put in, given their keys.
pub fn sort_rows(rows: &mut [usize], keys: &[SortKey], spec: &SortSpec) {
    rows.sort_by(|a, b| {
        let ord = keys[*a].compare(&keys[*b], &spec.field);
        if spec.descending { ord.reverse() } else { ord }
    });
}
