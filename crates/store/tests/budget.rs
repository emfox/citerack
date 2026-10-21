//! The opening budget, measured on a generated library: a stored fixture would
//! measure the fixture, and generating the text keeps its shape beside the limit
//! it is held to.

use std::time::Instant;

use citerack_bib::ParseOptions;
use citerack_bib::parse_str;
use citerack_index::{Matcher, SearchOptions, Searcher};

/// The number of entries the budget is stated for.
const ENTRIES: usize = 50_000;

/// Parsing this many entries may not exceed this; a debug build gets half again.
const PARSE_BUDGET_S: f64 = 2.0;

/// A full-text search over the parsed library may not exceed this.
const SEARCH_BUDGET_MS: f64 = 50.0;

/// A debug build runs the same code roughly ten times slower, so the limits are
/// scaled rather than skipped: a budget that is skipped where it is checked least
/// is a budget nobody holds.
fn budget(limit: f64) -> f64 {
    if cfg!(debug_assertions) {
        limit * 10.0
    } else {
        limit
    }
}

/// Builds a library with `ENTRIES` entries of the shape a real file has.
fn generate(entries: usize) -> String {
    let mut out = String::with_capacity(entries * 1200);
    for i in 0..entries {
        out.push_str(&format!(
            "@article{{author{i:06}2024,\n  author = {{Surname{i}, Given and Other{i}, Second}},\n  \
             title = {{A Study of Something Number {i} in Distributed Systems}},\n  \
             journal = {{Journal of Things}},\n  year = {{2024}},\n  volume = {{{}}},\n  \
             pages = {{1--10}},\n  doi = {{10.1000/x{i}}},\n  abstract = {{{}}}\n}}\n\n",
            i % 40,
            "This abstract repeats a sentence long enough to look like one. ".repeat(2)
        ));
    }
    out
}

#[test]
fn parsing_fifty_thousand_entries_fits_the_budget() {
    let text = generate(ENTRIES);
    let started = Instant::now();
    let library = parse_str(&text, &ParseOptions::default()).unwrap();
    let elapsed = started.elapsed().as_secs_f64();
    assert_eq!(library.len(), ENTRIES);
    println!(
        "parse: {ENTRIES} entries, {:.2} MiB, {elapsed:.3} s",
        text.len() as f64 / (1024.0 * 1024.0)
    );
    assert!(
        elapsed < budget(PARSE_BUDGET_S),
        "parsing took {elapsed:.3} s, budget is {:.1} s",
        budget(PARSE_BUDGET_S)
    );
}

#[test]
fn a_search_over_fifty_thousand_entries_fits_the_budget() {
    let text = generate(ENTRIES);
    let library = parse_str(&text, &ParseOptions::default()).unwrap();
    let index_started = Instant::now();
    let searcher = Searcher::build(&library, &SearchOptions::default());
    let index_s = index_started.elapsed().as_secs_f64();

    let started = Instant::now();
    let result = searcher.search(&Matcher::new("Surname42, Given and Other42"), None);
    let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
    println!(
        "index: {index_s:.3} s   search: {elapsed_ms:.3} ms, {} hits",
        result.total
    );
    assert!(result.total > 0, "the query found nothing to time");
    assert!(
        elapsed_ms < budget(SEARCH_BUDGET_MS),
        "search took {elapsed_ms:.3} ms, budget is {:.0} ms",
        budget(SEARCH_BUDGET_MS)
    );
}
