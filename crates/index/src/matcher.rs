//! A compiled query, evaluated against one field at a time.

use crate::fold::fold;
use crate::token::terms;

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

/// One or more terms that all have to match, in any field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Matcher {
    terms: Vec<String>,
    /// Only match when the whole field equals the query, for a key lookup.
    exact: bool,
}

impl Matcher {
    /// Compiles a query, folding and cutting it the way the index did the text, so
    /// a pasted `M{\"u}ller` finds the entry keyed under `m`. A query of only
    /// whitespace has no terms and matches every row, as an empty box should.
    pub fn new(query: &str) -> Self {
        let folded = fold(query);
        let terms = terms(&folded)
            .iter()
            .map(|term| folded[term.start..term.end].to_owned())
            .collect();
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

    /// Whether this matcher accepts only a whole-field match.
    pub fn is_exact(&self) -> bool {
        self.exact
    }

    /// The terms, folded, in the order the caller wrote them.
    pub fn terms(&self) -> &[String] {
        &self.terms
    }

    /// Every term is present in the field, in any order; the caller narrows the
    /// rows with the index first, so this decides the answer per candidate.
    pub fn matches(&self, folded_text: &str) -> bool {
        self.terms.iter().all(|term| {
            if self.exact {
                folded_text == term
            } else {
                folded_text.contains(term.as_str())
            }
        })
    }
}
