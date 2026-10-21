//! The duplicate scan a window asks for every frame: it must follow the library
//! rather than a library that existed once.

use std::path::PathBuf;

use citerack_bib::ParseOptions;
use citerack_store::Session;

const INPUT: &str = "\
@article{smith2024,
  Author = {Smith, John},
  Title  = {A Study of Braces},
  Year   = 2024,
  Doi    = {10.1000/braces},
}

@article{smith2024b,
  Author = {Smith, J.},
  Title  = {A Study of Braces},
  Year   = 2024,
}

@book{jones1999,
  Author = {Jones, Ann},
  Title  = {Another Book},
  Year   = 1999,
}
";

fn session() -> Session {
    Session::from_text(
        PathBuf::from("duplicate.bib"),
        INPUT,
        &ParseOptions { recover: true },
    )
    .expect("the input parses")
}

#[test]
fn a_title_and_year_shared_twice_is_found() {
    let mut session = session();
    let found: Vec<(usize, usize)> = session
        .duplicates()
        .iter()
        .map(|pair| (pair.first, pair.second))
        .collect();
    assert_eq!(found, [(0, 1)], "the two braces entries are one work");
}

#[test]
fn giving_a_duplicate_its_own_title_ends_the_pair() {
    let mut session = session();
    assert_eq!(session.duplicate_count(), 1);
    // Fixing the second entry is what a user does about it.
    session.set_field(1, 1, "A Different Study").unwrap();
    assert_eq!(
        session.duplicate_count(),
        0,
        "the scan follows the edit rather than the library it first saw"
    );
}

#[test]
fn undoing_the_fix_brings_the_pair_back() {
    let mut session = session();
    session.set_field(1, 1, "A Different Study").unwrap();
    assert_eq!(session.duplicate_count(), 0);
    session.undo().expect("a step to undo");
    assert_eq!(
        session.duplicate_count(),
        1,
        "the scan is dropped when a restore changes a row"
    );
}

#[test]
fn a_doi_given_to_a_second_entry_pairs_it() {
    let mut session = session();
    assert_eq!(session.duplicate_count(), 1);
    // The third entry has no doi field, so the DOI is added rather than replaced,
    // which is how a user makes two entries name the same work.
    session.add_field(2, "doi", "10.1000/braces").unwrap();
    let reasons: Vec<&str> = session
        .duplicates()
        .iter()
        .filter(|pair| pair.second == 2)
        .map(|pair| pair.reason.as_str())
        .collect();
    assert_eq!(
        reasons,
        ["the same DOI"],
        "the added DOI pairs the third entry with the first"
    );
}

#[test]
fn asking_twice_does_not_rescan() {
    let mut session = session();
    let first = session.duplicates().to_vec();
    let second = session.duplicates().to_vec();
    assert_eq!(first, second, "the second answer is the one already held");
}
