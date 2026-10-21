//! A compiled query, evaluated against one field at a time.

use crate::fold::fold;

/// Which field a hit came from, so the result list can say so.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldRef {
    /// The citation key.
    Key,
    /// The entry type, for example `article`.
    Kind,
    /// A field by its normalised name.
    Named(usize),
}

/// Where a query matched, as a byte range into the folded text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hit {
    /// Which field matched.
    pub field: FieldRef,
    /// First byte of the match.
    pub start: usize,
    /// One past the last byte of the match.
    pub end: usize,
}

/// One or more terms that all have to match, in any field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Matcher {
    terms: Vec<String>,
    /// Only match when the whole field equals the query, for a key lookup.
    exact: bool,
}

impl Matcher {
    /// Compiles a query, folding it the way the index folded the text; a query of
    /// only whitespace matches every row, as an empty search box should.
    pub fn new(query: &str) -> Self {
        let terms = query.split_whitespace().map(fold).collect();
        Self {
            terms,
            exact: false,
        }
    }

    /// A matcher that only accepts an exact, whole-field match.
    pub fn exact(query: &str) -> Self {
        Self {
            terms: vec![fold(query)],
            exact: true,
        }
    }

    /// Whether this query has no terms, and so matches every row.
    pub fn is_empty(&self) -> bool {
        self.terms.is_empty()
    }

    /// The terms, folded, in the order the caller wrote them.
    pub fn terms(&self) -> &[String] {
        &self.terms
    }

    /// Every term is present in the field, in any order.
    pub fn matches(&self, field: FieldRef, folded_text: &str) -> Option<Hit> {
        if self.terms.is_empty() {
            return Some(Hit {
                field,
                start: 0,
                end: 0,
            });
        }
        let mut first: Option<(usize, usize)> = None;
        for term in &self.terms {
            let at = if self.exact {
                (folded_text == term).then_some(0)
            } else {
                memchr::memmem::find(folded_text.as_bytes(), term.as_bytes())
            };
            let at = at?;
            let end = at + term.len();
            first = Some(match first {
                Some((s, e)) => (s.min(at), e.max(end)),
                None => (at, end),
            });
        }
        let (start, end) = first?;
        Some(Hit { field, start, end })
    }
}
