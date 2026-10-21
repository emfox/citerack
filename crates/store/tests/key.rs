//! Keys written from the entry rather than by hand: a pattern over the fields,
//! and the letter that keeps two entries apart when the pattern makes one key.

use std::path::PathBuf;

use citerack_bib::PatternError;
use citerack_store::Session;

const INPUT: &str = "\
@Article{byhand,
  Author   = {Smith, John},
  Title    = {A Study of Braces},
  Year     = 2024,
}

@Article{Erdos1959,
  Author   = {Erd{\\H o}s, Paul},
  Title    = {On Graphs},
  Year     = 1959,
}
";

fn session(text: &str) -> Session {
    Session::from_text(
        PathBuf::from("keys.bib"),
        text,
        &citerack_bib::ParseOptions { recover: true },
    )
    .expect("the input parses")
}

#[test]
fn the_default_pattern_names_an_entry_by_its_first_author_and_year() {
    let session = session(INPUT);
    assert_eq!(session.key_pattern(), "{author}{year}");
    assert_eq!(session.proposed_key(0), "Smith2024");
    assert_eq!(
        session.proposed_key(1),
        "Erdos1959",
        "an accented surname is written without its accent"
    );
}

#[test]
fn a_pattern_can_be_changed_and_read_back() {
    let mut session = session(INPUT);
    session
        .set_key_pattern("{author:l}:{year}")
        .expect("a pattern");
    assert_eq!(session.key_pattern(), "{author:l}:{year}");
    assert_eq!(session.proposed_key(0), "smith:2024");
}

#[test]
fn a_pattern_that_cannot_name_anything_is_refused_and_the_old_one_kept() {
    let mut session = session(INPUT);
    assert_eq!(session.set_key_pattern("smith"), Err(PatternError::NoField));
    assert_eq!(
        session.key_pattern(),
        "{author}{year}",
        "a refused pattern leaves the setting as it was"
    );
}

#[test]
fn a_key_that_is_taken_gets_a_letter_rather_than_overwriting() {
    let session = session(
        "\
@Article{Smith2024,
  Author = {Smith, John},
  Year   = 2024,
}

@Article{other,
  Author = {Smith, John},
  Year   = 2024,
}
",
    );
    assert_eq!(
        session.proposed_key(1),
        "Smith2024a",
        "the plain key belongs to the entry that holds it, so this one is lettered"
    );
}

#[test]
fn two_generated_keys_in_one_library_do_not_collide() {
    let session = session(INPUT);
    let keys = session.proposed_keys();
    assert_eq!(keys.len(), 2);
    assert_ne!(keys[0], keys[1], "the column never shows one key twice");
}

#[test]
fn generating_a_key_changes_the_key_and_nothing_else() {
    let mut session = session(INPUT);
    session.generate_key(0).expect("a key");
    assert_eq!(
        session.library().entries()[0].key.as_ref(),
        "Smith2024",
        "the entry is called by the pattern"
    );
    assert_eq!(session.library().entries()[0].fields.len(), 3);
    assert_eq!(
        session.value(0, 1),
        Some("A Study of Braces"),
        "the fields stand"
    );
    assert!(session.is_edited(), "the rename is work to save");
}

#[test]
fn a_pattern_change_brings_a_key_up_to_date_and_writes_the_others() {
    let mut session = session(INPUT);
    // The keys the file holds were written by its author, and a press replaces
    // them: what the column shows is what a press writes.
    let written = session.generate_keys().expect("the keys");
    assert_eq!(
        written, 1,
        "the entry whose key was not the pattern's is renamed"
    );
    assert_eq!(session.library().entries()[0].key.as_ref(), "Smith2024");
    assert_eq!(
        session.library().entries()[1].key.as_ref(),
        "Erdos1959",
        "a key that already says what the pattern says stands"
    );

    // A new pattern renames both again, and the second press of it is a no-op.
    session
        .set_key_pattern("{author:u}_{year}")
        .expect("a pattern");
    session.settle_all();
    let written = session.generate_keys().expect("the keys");
    assert_eq!(written, 2, "a new pattern reaches both entries");
    assert_eq!(session.library().entries()[0].key.as_ref(), "SMITH_2024");
    assert_eq!(session.library().entries()[1].key.as_ref(), "ERDOS_1959");
    session.settle_all();
    assert_eq!(session.generate_keys().expect("the keys"), 0);
}

#[test]
fn a_second_press_of_generate_all_changes_nothing() {
    let mut session = session(INPUT);
    let first = session.generate_keys().expect("the keys");
    assert_eq!(
        first, 1,
        "only the key the pattern did not make is left as it was"
    );
    assert_eq!(
        session.library().entries()[0].key.as_ref(),
        "Smith2024",
        "the first entry is named by its own fields"
    );
    session.settle_all();
    let second = session.generate_keys().expect("the keys");
    assert_eq!(
        second, 0,
        "every key already says what the pattern says, so a second press writes nothing"
    );
}
