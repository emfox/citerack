//! An edit re-indexes the field it touched rather than the whole column, so the
//! answer a query gets afterwards has to match a library that was indexed in one
//! pass. These tests compare the two directly.

use citerack_bib::{Library, parse_str};
use citerack_index::{Matcher, SearchOptions, Searcher};

const SAMPLE: &str = "\
@article{smith2024,
  author = {Smith, John},
  title  = {A Study of Braces},
  year   = {2024},
}

@book{jones1999,
  author = {Jones, Ann},
  title  = {Another Book},
  year   = {1999},
}

@misc{lee2001,
  author = {Lee, Kaya},
  title  = {Quiet Machines},
  year   = {2001},
}
";

fn library() -> Library {
    parse_str(SAMPLE, &Default::default()).expect("the sample parses")
}

/// Every row a query matches, which is what a filter box would show.
fn hits(searcher: &Searcher, query: &str) -> Vec<usize> {
    searcher.search(&Matcher::new(query), None).rows
}

fn queries() -> [&'static str; 8] {
    [
        "smith", "braces", "book", "machines", "anon", "sm", "es", "kai",
    ]
}

#[test]
fn a_short_term_after_an_edit_answers_like_a_library_indexed_again() {
    let mut edited = Searcher::build(&library(), &SearchOptions::default());
    edited.replace_field(1, "title", "The Other Book");
    let mut rebuilt_text = String::from(SAMPLE);
    rebuilt_text = rebuilt_text.replace("{Another Book}", "{The Other Book}");
    let rebuilt = Searcher::build(
        &parse_str(&rebuilt_text, &Default::default()).expect("the edited sample parses"),
        &SearchOptions::default(),
    );

    for query in ["ano", "oth", "boo", "k", "sm", "ann"] {
        edited.settle(query);
        assert_eq!(
            hits(&edited, query),
            hits(&rebuilt, query),
            "a short term disagrees on {query:?}"
        );
    }
}

#[test]
fn a_short_term_the_edit_removed_stops_matching() {
    let mut edited = Searcher::build(&library(), &SearchOptions::default());
    assert_eq!(hits(&edited, "ano"), [1], "only the book said it");

    edited.replace_field(1, "title", "The Other Book");
    edited.settle("ano");

    assert_eq!(hits(&edited, "ano"), [] as [usize; 0]);
    assert_eq!(hits(&edited, "oth"), [1], "the word that replaced it");
}

#[test]
fn a_field_replaced_in_the_index_answers_like_a_library_indexed_again() {
    let before = library();
    let indexed = Searcher::build(&before, &SearchOptions::default());

    let mut text = String::from(SAMPLE);
    text = text.replace("{Another Book}", "{The Other Book}");
    text = text.replace("{Jones, Ann}", "{Jones, Anna}");
    let after = parse_str(&text, &Default::default()).expect("the edited sample parses");
    let rebuilt = Searcher::build(&after, &SearchOptions::default());

    let mut edited = indexed;
    edited.replace_field(1, "title", "The Other Book");
    edited.replace_field(1, "author", "Jones, Anna");
    // A short term is answered from tables an edit drops until a query asks for
    // them, so the comparison only means anything once they are laid out.
    edited.settle("sm");

    for query in queries() {
        assert_eq!(
            hits(&edited, query),
            hits(&rebuilt, query),
            "the incremental index and the rebuilt one disagree on {query:?}"
        );
    }
}

#[test]
fn a_word_the_edit_removed_stops_matching() {
    let indexed = Searcher::build(&library(), &SearchOptions::default());
    assert_eq!(
        hits(&indexed, "another"),
        [1],
        "only the book had that word"
    );

    let mut edited = indexed;
    edited.replace_field(1, "title", "The Other Book");
    assert_eq!(hits(&edited, "another"), [] as [usize; 0]);
    assert_eq!(hits(&edited, "other"), [1]);
}

#[test]
fn a_field_read_back_is_the_folded_text_the_index_holds() {
    let indexed = Searcher::build(&library(), &SearchOptions::default());
    assert_eq!(indexed.value(0, "author"), Some(Some("smith, john")));
    assert_eq!(indexed.value(2, "year"), Some(Some("2001")));
    assert_eq!(
        indexed.value(0, "note"),
        None,
        "a field no entry had is not indexed"
    );
}

#[test]
fn the_covered_field_names_are_each_listed_once() {
    let indexed = Searcher::build(&library(), &SearchOptions::default());
    let names = indexed.field_names();
    assert_eq!(names, ["author", "title", "year"]);
}

#[test]
fn the_field_names_are_listed_once_even_after_a_run_of_edits() {
    // Replacing a field rebuilds the column list, so the names are asked for by
    // themselves rather than by the column beside them.
    let mut indexed = Searcher::build(&library(), &SearchOptions::default());
    for name in ["title", "author", "year", "title", "year"] {
        indexed.replace_field(0, name, "Smith, John");
    }
    let names = indexed.field_names();
    assert_eq!(names, ["author", "title", "year"], "one entry per field");
}

#[test]
fn replacing_a_field_the_index_does_not_cover_changes_nothing() {
    let mut indexed = Searcher::build(&library(), &SearchOptions::default());
    let before: Vec<Vec<usize>> = queries().iter().map(|q| hits(&indexed, q)).collect();
    // A field no entry had, and a row the library does not hold, each leave the
    // index answering as it did.
    indexed.replace_field(0, "note", "something");
    indexed.replace_field(9, "title", "nowhere");
    let after: Vec<Vec<usize>> = queries().iter().map(|q| hits(&indexed, q)).collect();
    assert_eq!(before, after, "an unknown field or row changes nothing");
}
