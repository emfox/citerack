//! Search and sort built once per library: which rows match a query, and which
//! rows a sort would put first. Neither answer needs a window, so both are
//! computed before one exists.

mod fold;
mod matcher;
mod search;
mod sort;

pub use fold::fold;
pub use matcher::{FieldRef, Hit, Matcher};
pub use search::{SearchOptions, SearchResult, Searcher};
pub use sort::{SortField, SortKey, SortSpec};

/// The order a list of row indices should be put in, given their keys.
pub fn sort_rows(rows: &mut [usize], keys: &[SortKey], spec: &SortSpec) {
    rows.sort_by(|a, b| {
        let ord = keys[*a].compare(&keys[*b], &spec.field);
        if spec.descending { ord.reverse() } else { ord }
    });
}
