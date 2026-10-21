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
    let result = searcher.search(&Matcher::new("galaxy"), Some(1));
    assert_eq!(result.rows, vec![1]);
    assert_eq!(result.total, 1);
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

/// A query answers exactly what a scan of every cell would, whatever shape the
/// term has: a whole word, a word prefix, a fragment of a word, or nothing.
#[test]
fn the_index_agrees_with_a_full_scan() {
    let lib = library();
    let searcher = Searcher::build(&lib, &SearchOptions::default());
    for query in [
        "",
        "a",
        "an",
        "art",
        "indexing",
        "ide",
        "dex",
        "zhao",
        "2024",
        "guide galaxy",
        "o",
        "muller",
        "M{\\\"u}ller",
        "fast filters",
        "nothing",
    ] {
        let matcher = Matcher::new(query);
        assert_eq!(
            searcher.search(&matcher, None).rows,
            searcher.scan_rows(&matcher),
            "query {query:?} answered differently by the index"
        );
    }
}

/// A generated library the size of a real one, written the way one is.
fn large(entries: usize) -> citerack_bib::Library {
    let mut text = String::new();
    for i in 0..entries {
        text.push_str(&format!(
            "@article{{key{i:05},\n  author = {{Surname{i}, Given and Other}},\n  \
             title = {{A Study of Something Number {i} in Distributed Systems}},\n  \
             journal = {{Journal of Things}},\n  year = {{2024}},\n  abstract = {{The quick brown \
             fox jumps over the lazy dog while indexing happens.}}\n}}\n\n"
        ));
    }
    parse_str(&text, &Default::default()).unwrap()
}

/// A library whose every word is unique, the shape a real catalogue has and the
/// generated one above does not: nothing repeats, so no list shrinks by luck.
fn high_entropy(entries: usize) -> citerack_bib::Library {
    let mut text = String::new();
    for i in 0..entries {
        let word = format!("w{i:04}x{}", (i * 7919) % 100_003);
        let other = format!("t{}q", (i * 104_729) % 99_991);
        text.push_str(&format!(
            "@misc{{k{i:05},\n  author = {{{word}, Given}},\n  title = {{The {word} of {other}}},\n  \
             year = {{2000}},\n}}\n\n"
        ));
    }
    parse_str(&text, &Default::default()).unwrap()
}

/// A short term is answered by the words it sits inside, on a library where no
/// word is repeated and the vocabulary is as wide as the entry count.
#[test]
fn a_short_term_agrees_with_a_scan_on_a_library_of_unique_words() {
    let lib = high_entropy(3_000);
    let searcher = Searcher::build(&lib, &SearchOptions::default());
    for query in ["w", "x", "w0", "x1", "0x", "q", "of", "the", "999", "zzz"] {
        let matcher = Matcher::new(query);
        assert_eq!(
            searcher.search(&matcher, None).rows,
            searcher.scan_rows(&matcher),
            "query {query:?} answered differently by the index"
        );
    }
}

/// A short term in most rows is answered by the rows that lack it, which is a
/// separate table from the words that hold it. The two must agree with a scan,
/// including on the rows that the table has to exclude.
#[test]
fn a_term_most_rows_hold_agrees_with_a_scan() {
    let lib = high_entropy(12_000);
    let searcher = Searcher::build(&lib, &SearchOptions::default());
    for query in ["a", "e", "t", "w", "0", "1", "of", "zzz", "x"] {
        let matcher = Matcher::new(query);
        assert_eq!(
            searcher.search(&matcher, None).rows,
            searcher.scan_rows(&matcher),
            "query {query:?} answered differently by the index"
        );
    }
}

/// The index answers what a scan answers on a library far larger than a test's
/// sample, including a term that only a part of the rows hold.
#[test]
fn the_index_agrees_with_a_full_scan_on_a_large_library() {
    let lib = large(20_000);
    let searcher = Searcher::build(&lib, &SearchOptions::default());
    for query in [
        "surname199",
        "fox",
        "study something",
        "2024",
        "article",
        "xyz",
        "othing 199",
    ] {
        let matcher = Matcher::new(query);
        assert_eq!(
            searcher.search(&matcher, None).rows,
            searcher.scan_rows(&matcher),
            "query {query:?} answered differently by the index"
        );
    }
}
