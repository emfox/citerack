//! Importing an entry a file did not hold: what a save writes, what the table
//! shows afterwards, and what an undo leaves behind. A save is compared to the
//! input, so the rule that a change touches only what it names holds here too.

use std::path::PathBuf;

use citerack_bib::{Delimiter, Entry, EntryKind, Field, FieldName};
use citerack_store::Session;

const INPUT: &str = "\
@Article{smith2024,
  Author   = {Smith, John},
  Title    = {A Study},
  year     = 2024,
}
";

fn session(text: &str) -> Session {
    Session::from_text(PathBuf::from("lib.bib"), text, &Default::default()).unwrap()
}

fn field(name: &str, value: &str) -> Field {
    Field {
        name_raw: name.into(),
        name: FieldName::new(name),
        value: value.into(),
        delimiter: Delimiter::Braces,
    }
}

fn record(key: &str, title: &str, author: &str, year: &str) -> Entry {
    Entry {
        kind: EntryKind::new("Article"),
        key: key.into(),
        fields: vec![
            field("author", author),
            field("title", title),
            field("year", year),
            field("doi", "10.1000/xyz"),
        ],
    }
}

fn saved(session: &Session) -> String {
    session.text()
}

#[test]
fn an_imported_entry_is_written_after_the_last_one() {
    let mut session = session(INPUT);
    let row = session
        .import(record("doe2021", "Another Study", "Doe, Jane", "2021"))
        .unwrap();
    assert_eq!(row, 1);
    let out = saved(&session);
    assert!(out.starts_with(INPUT), "the file's own bytes stay:\n{out}");
    assert!(out.contains("@Article{doe2021,"), "{out}");
    assert!(out.contains("  title = {Another Study},"), "{out}");
}

#[test]
fn an_import_is_one_undo_step_that_takes_the_entry_out_again() {
    let mut session = session(INPUT);
    session
        .import(record("doe2021", "Another Study", "Doe, Jane", "2021"))
        .unwrap();
    assert!(session.is_edited());
    assert_eq!(session.undo(), Some("an import"));
    assert_eq!(session.len(), 1);
    assert_eq!(
        saved(&session),
        INPUT,
        "taking it back leaves the file as it was"
    );
    assert!(!session.is_edited());
}

#[test]
fn a_redo_brings_the_entry_back() {
    let mut session = session(INPUT);
    session
        .import(record("doe2021", "Another Study", "Doe, Jane", "2021"))
        .unwrap();
    session.undo();
    assert_eq!(session.redo(), Some("an import"));
    assert_eq!(session.len(), 2);
    assert!(saved(&session).contains("@Article{doe2021,"));
}

#[test]
fn a_key_already_in_the_library_takes_a_letter() {
    let mut session = session(INPUT);
    let row = session
        .import(record("smith2024", "A Second Study", "Smith, John", "2024"))
        .unwrap();
    assert_eq!(
        session.library().entries()[row].key.as_ref(),
        "smith2024a",
        "the entry keeps its own key out of the way of the one already there"
    );
}

#[test]
fn an_imported_entry_shows_up_in_the_table_and_in_a_search() {
    let mut session = session(INPUT);
    session
        .import(record("doe2021", "Another Study", "Doe, Jane", "2021"))
        .unwrap();
    assert_eq!(session.len(), 2);
    assert_eq!(session.order().len(), 2, "the new row is on the table");

    let found = session.search("another");
    assert_eq!(found.rows.len(), 1, "and the index can find it");
    session.clear_search();
    assert_eq!(session.order().len(), 2);

    let found = session.search("smith");
    assert_eq!(found.rows.len(), 1, "the old entry is still found");
}

#[test]
fn an_imported_entry_is_sorted_with_the_others() {
    let mut session = session(INPUT);
    session
        .import(record(
            "adams2020",
            "An Earlier Study",
            "Adams, Amy",
            "2020",
        ))
        .unwrap();
    session.sort(&citerack_index::SortSpec {
        field: citerack_index::SortField::Year,
        descending: false,
    });
    let keys: Vec<String> = session
        .order()
        .iter()
        .map(|row| session.library().entries()[*row].key.to_string())
        .collect();
    assert_eq!(keys, vec!["adams2020", "smith2024"]);
}

#[test]
fn a_field_of_an_imported_entry_can_be_edited_afterwards() {
    let mut session = session(INPUT);
    let row = session
        .import(record("doe2021", "A Draft Title", "Doe, Jane", "2021"))
        .unwrap();
    session.set_field(row, 1, "A Finished Title").unwrap();
    let out = saved(&session);
    assert!(out.contains("title = {A Finished Title}"), "{out}");
}
