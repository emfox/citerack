//! What counts as the same work entered twice, and what does not. The spellings
//! here are the ones a real library holds: a DOI with a resolver in front of it, a
//! title with braces kept for capitalisation, a year in quotes.

use citerack_bib::{ParseOptions, parse_str};
use citerack_index::{Reason, find_duplicates};

/// The duplicate scan of a library written as text.
fn pairs(text: &str) -> Vec<(usize, usize, Reason)> {
    let library = parse_str(text, &ParseOptions::default()).expect("the input parses");
    find_duplicates(&library)
        .into_iter()
        .map(|pair| (pair.first, pair.second, pair.reason))
        .collect()
}

#[test]
fn two_entries_with_the_same_doi_are_a_pair() {
    let found = pairs(
        "\
@article{a, doi = {10.1000/xyz}, title = {First}}
@article{b, doi = {10.1000/xyz}, title = {Second}}
",
    );
    assert_eq!(found, [(0, 1, Reason::Doi)]);
}

#[test]
fn a_doi_spelled_with_a_resolver_is_the_same_doi() {
    let found = pairs(
        "\
@article{a, doi = {10.1000/xyz}}
@article{b, doi = {https://doi.org/10.1000/XYZ}}
@article{c, doi = {doi:10.1000/xyz}}
",
    );
    assert_eq!(
        found,
        [(0, 1, Reason::Doi), (0, 2, Reason::Doi)],
        "every spelling of one DOI pairs with the first entry that named it"
    );
}

#[test]
fn a_field_that_is_not_a_doi_pairs_with_nothing() {
    let found = pairs(
        "\
@article{a, doi = {to be added}}
@article{b, doi = {to be added}}
",
    );
    assert!(
        found.is_empty(),
        "a note in the doi field is not an identity: {found:?}"
    );
}

#[test]
fn the_same_title_and_year_is_a_pair() {
    let found = pairs(
        "\
@article{a, title = {A Study of Braces}, year = 2024}
@article{b, title = {A Study of Braces}, year = 2024}
",
    );
    assert_eq!(found, [(0, 1, Reason::TitleAndYear)]);
}

#[test]
fn a_title_pairs_through_case_accents_and_braces() {
    let found = pairs(
        "\
@article{a, title = {The Art of {N}ested Braces}, year = {2024}}
@article{b, title = {the art of nested braces}, year = {2024}}
@article{c, title = {Th\u{e9} Art of Nested Braces}, year = {2024}}
",
    );
    assert_eq!(
        found,
        [(0, 1, Reason::TitleAndYear), (0, 2, Reason::TitleAndYear)],
        "punctuation, case, braces and an accent all fold away"
    );
}

#[test]
fn the_same_title_in_another_year_is_not_a_pair() {
    let found = pairs(
        "\
@article{a, title = {A Study of Braces}, year = 2024}
@article{b, title = {A Study of Braces}, year = 2023}
",
    );
    assert!(found.is_empty(), "a reprint is a different year: {found:?}");
}

#[test]
fn two_entries_with_no_year_are_not_paired_on_a_title_alone() {
    let found = pairs(
        "\
@article{a, title = {A Study of Braces}}
@article{b, title = {A Study of Braces}}
",
    );
    assert!(
        found.is_empty(),
        "without a year there is only half of the identity: {found:?}"
    );
}

#[test]
fn two_entries_with_no_title_are_not_paired_on_a_year_alone() {
    let found = pairs(
        "\
@article{a, year = 2024}
@article{b, year = 2024}
",
    );
    assert!(
        found.is_empty(),
        "a shared year is not a shared work: {found:?}"
    );
}

#[test]
fn a_doi_outranks_a_title_and_both_are_reported() {
    let found = pairs(
        "\
@article{a, doi = {10.1000/xyz}, title = {Same}, year = 2024}
@article{b, doi = {10.1000/xyz}, title = {Same}, year = 2024}
",
    );
    assert_eq!(
        found,
        [(0, 1, Reason::Doi), (0, 1, Reason::TitleAndYear)],
        "the pair is listed once per reason, since each is a separate thing to fix"
    );
}

#[test]
fn three_copies_are_two_pairs_against_the_first() {
    let found = pairs(
        "\
@article{a, doi = {10.1000/xyz}}
@article{b, doi = {10.1000/xyz}}
@article{c, doi = {10.1000/xyz}}
",
    );
    assert_eq!(
        found,
        [(0, 1, Reason::Doi), (0, 2, Reason::Doi)],
        "a third copy is reported against the first, not against both"
    );
}

#[test]
fn a_library_with_no_duplicates_has_no_pairs() {
    let found = pairs(
        "\
@article{a, doi = {10.1000/one}, title = {One}, year = 2020}
@article{b, doi = {10.1000/two}, title = {Two}, year = 2021}
",
    );
    assert!(found.is_empty());
}

#[test]
fn the_pairs_come_out_in_file_order() {
    let found = pairs(
        "\
@article{c, doi = {10.1000/late}}
@article{a, doi = {10.1000/early}}
@article{d, doi = {10.1000/late}}
@article{b, doi = {10.1000/early}}
",
    );
    assert_eq!(
        found,
        [(0, 2, Reason::Doi), (1, 3, Reason::Doi)],
        "sorted by the first entry, so the list does not move between runs"
    );
}
