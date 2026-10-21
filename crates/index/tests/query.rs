//! What the index answers, and that the answer does not depend on a window.

use citerack_bib::parse_str;
use citerack_index::{
    Matcher, SearchOptions, Searcher, SortField, SortKey, SortSpec, fold, sort_rows,
};

const SAMPLE: &str = "\
@article{muller2021,
  author = {M{\\\"u}ller, Hans and Doe, Jane},
  title  = {The Art of Indexing},
  year   = {2021},
}

@book{adams1979,
  author = {Adams, Douglas},
  title  = {A Guide to the Galaxy},
  year   = {1979},
}

@inproceedings{zhao2024,
  author = {Zhao, Wei},
  title  = {Fast Filters},
  year   = {2024},
}
";

fn library() -> citerack_bib::Library {
    parse_str(SAMPLE, &Default::default()).unwrap()
}

fn rows(query: &str) -> Vec<usize> {
    let lib = library();
    let searcher = Searcher::build(&lib, &SearchOptions::default());
    searcher.search(&Matcher::new(query), None).rows
}

#[test]
fn an_empty_query_matches_every_row() {
    assert_eq!(rows(""), vec![0, 1, 2]);
}

#[test]
fn a_term_matches_inside_a_field() {
    assert_eq!(rows("indexing"), vec![0]);
}

#[test]
fn matching_ignores_case() {
    assert_eq!(rows("GUIDE"), vec![1]);
}

#[test]
fn several_terms_must_all_match_the_same_entry() {
    assert_eq!(rows("guide galaxy"), vec![1]);
    assert!(rows("guide zhao").is_empty());
}

#[test]
fn the_citation_key_is_searchable() {
    assert_eq!(rows("zhao2024"), vec![2]);
}

#[test]
fn an_accented_letter_folds_to_its_base() {
    assert_eq!(fold("Müller"), "muller");
}

#[test]
fn a_braced_accent_sequence_still_matches_the_plain_spelling() {
    // The sample spells the umlaut as a TeX escape, which fold() cannot undo.
    let lib = library();
    let searcher = Searcher::build(&lib, &SearchOptions::default());
    let plain = parse_str("@misc{a, author = {Muller, Hans}}\n", &Default::default()).unwrap();
    let plain_index = Searcher::build(&plain, &SearchOptions::default());
    assert_eq!(
        searcher.search(&Matcher::new("hans"), None).rows,
        plain_index.search(&Matcher::new("hans"), None).rows
    );
}

#[test]
fn the_order_of_terms_does_not_matter() {
    assert_eq!(rows("galaxy guide"), rows("guide galaxy"));
}

#[test]
fn a_limit_stops_the_row_list_but_not_the_count() {
    let lib = library();
    let searcher = Searcher::build(&lib, &SearchOptions::default());
    let result = searcher.search(&Matcher::new("a"), Some(1));
    assert_eq!(result.rows.len(), 1);
    assert!(result.total >= 2);
}

#[test]
fn restricting_fields_excludes_the_rest() {
    let lib = library();
    let options = SearchOptions {
        fields: vec![String::from("title")],
        include_key: false,
        limit: None,
    };
    let searcher = Searcher::build(&lib, &options);
    assert_eq!(
        searcher.search(&Matcher::new("zhao"), None).rows,
        Vec::<usize>::new()
    );
    assert_eq!(searcher.search(&Matcher::new("fast"), None).rows, vec![2]);
}

#[test]
fn an_exact_matcher_rejects_a_prefix() {
    let lib = library();
    let searcher = Searcher::build(&lib, &SearchOptions::default());
    assert_eq!(searcher.search(&Matcher::exact("zhao"), None).total, 0);
    assert_eq!(
        searcher.search(&Matcher::exact("zhao2024"), None).rows,
        vec![2]
    );
}

#[test]
fn sorting_by_year_puts_1979_first() {
    let lib = library();
    let keys: Vec<SortKey> = (0..lib.len()).map(|i| SortKey::of(&lib, i)).collect();
    let mut order: Vec<usize> = (0..lib.len()).collect();
    sort_rows(
        &mut order,
        &keys,
        &SortSpec {
            field: SortField::Year,
            descending: false,
        },
    );
    assert_eq!(order, vec![1, 0, 2]);
}

#[test]
fn sorting_descending_reverses_the_year_order() {
    let lib = library();
    let keys: Vec<SortKey> = (0..lib.len()).map(|i| SortKey::of(&lib, i)).collect();
    let mut order: Vec<usize> = (0..lib.len()).collect();
    sort_rows(
        &mut order,
        &keys,
        &SortSpec {
            field: SortField::Year,
            descending: true,
        },
    );
    assert_eq!(order, vec![2, 0, 1]);
}

#[test]
fn sorting_by_title_ignores_a_leading_article() {
    let lib = library();
    let keys: Vec<SortKey> = (0..lib.len()).map(|i| SortKey::of(&lib, i)).collect();
    assert_eq!(keys[0].title, "art of indexing");
    assert_eq!(keys[1].title, "guide to the galaxy");
}

#[test]
fn sorting_by_first_author_uses_the_surname() {
    let lib = library();
    let keys: Vec<SortKey> = (0..lib.len()).map(|i| SortKey::of(&lib, i)).collect();
    let mut order: Vec<usize> = (0..lib.len()).collect();
    sort_rows(
        &mut order,
        &keys,
        &SortSpec {
            field: SortField::FirstAuthor,
            descending: false,
        },
    );
    assert_eq!(order, vec![1, 0, 2]);
}

#[test]
fn an_entry_without_a_year_sorts_before_one_that_has_it() {
    let lib = parse_str(
        "@misc{a, title = {x}}\n@misc{b, year = {2020}}\n",
        &Default::default(),
    )
    .unwrap();
    assert_eq!(SortKey::of(&lib, 0).year, 0);
    assert_eq!(SortKey::of(&lib, 1).year, 2020);
}
