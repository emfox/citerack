//! Adding an entry the file did not hold: what a save writes, where it lands, and
//! what taking it back leaves behind. The file's own bytes are held to the same
//! rule as everywhere else, so every test here compares a save to its input.

use citerack_bib::{
    Delimiter, EditError, Entry, EntryKind, Field, FieldName, ParseOptions, WriteOptions,
    parse_str, write_str,
};

const INPUT: &str = "\
% A library somebody typed by hand.

@Article{smith2024,
  Author   = {Smith, John},
  Title    = {A Study},
  year     = 2024,
}

@comment{and a note at the end}
";

fn parsed(text: &str) -> citerack_bib::Library {
    parse_str(text, &ParseOptions::default()).expect("the input parses")
}

fn saved(lib: &citerack_bib::Library) -> String {
    write_str(lib, &WriteOptions::Preserve)
}

fn field(name: &str, value: &str) -> Field {
    Field {
        name_raw: name.into(),
        name: FieldName::new(name),
        value: value.into(),
        delimiter: Delimiter::Braces,
    }
}

fn imported(key: &str, title: &str) -> Entry {
    Entry {
        kind: EntryKind::new("Article"),
        key: key.into(),
        fields: vec![
            field("title", title),
            field("doi", "10.1000/xyz"),
            field("year", "2021"),
        ],
    }
}

#[test]
fn an_added_entry_is_written_after_what_the_file_held() {
    let mut lib = parsed(INPUT);
    lib.append(imported("doe2021", "Another Study")).unwrap();
    let out = saved(&lib);
    assert!(
        out.starts_with(INPUT),
        "nothing the file held moved:\n{out}"
    );
    assert!(
        out.ends_with(
            "\n@Article{doe2021,\n  title = {Another Study},\n  doi = {10.1000/xyz},\n  year = {2021},\n}\n"
        ),
        "the new entry lands at the end, one field a line:\n{out}"
    );
}

#[test]
fn a_library_with_nothing_added_saves_byte_for_byte() {
    let lib = parsed(INPUT);
    assert_eq!(saved(&lib), INPUT);
}

#[test]
fn the_file_is_still_read_back_after_a_save() {
    let mut lib = parsed(INPUT);
    lib.append(imported("doe2021", "Another Study")).unwrap();
    let again = parsed(&saved(&lib));
    assert_eq!(again.len(), 2);
    assert_eq!(again.entries()[1].key.as_ref(), "doe2021");
    assert_eq!(again.entries()[1].field("doi"), Some("10.1000/xyz"));
}

#[test]
fn an_added_entry_counts_as_unsaved_work() {
    let mut lib = parsed(INPUT);
    assert!(!lib.is_edited());
    lib.append(imported("doe2021", "Another Study")).unwrap();
    assert!(lib.is_edited());
    lib.mark_saved();
    assert!(!lib.is_edited());
}

#[test]
fn taking_an_added_entry_back_leaves_the_file_it_came_from() {
    let mut lib = parsed(INPUT);
    let id = lib.append(imported("doe2021", "Another Study")).unwrap();
    assert!(lib.is_added(id));
    lib.unappend(id).unwrap();
    assert_eq!(lib.len(), 1);
    assert_eq!(saved(&lib), INPUT);
}

#[test]
fn an_entry_that_is_not_the_last_one_cannot_come_back_out() {
    let mut lib = parsed(INPUT);
    let first = lib.append(imported("doe2021", "One")).unwrap();
    lib.append(imported("doe2022", "Two")).unwrap();
    assert_eq!(lib.unappend(first), Err(EditError::Absent));
}

#[test]
fn an_entry_with_a_key_that_cannot_be_written_is_refused() {
    let mut lib = parsed(INPUT);
    let mut entry = imported("doe2021", "A Study");
    entry.key = "has space".into();
    assert_eq!(lib.append(entry), Err(EditError::BadKey));
    assert_eq!(lib.len(), 1);
}

#[test]
fn an_entry_with_a_field_name_that_cannot_be_written_is_refused() {
    let mut lib = parsed(INPUT);
    let mut entry = imported("doe2021", "A Study");
    entry.fields.push(field("no good", "value"));
    assert_eq!(lib.append(entry), Err(EditError::BadName));
    assert_eq!(lib.len(), 1);
}

#[test]
fn a_field_of_an_added_entry_takes_a_new_value() {
    let mut lib = parsed(INPUT);
    let id = lib.append(imported("doe2021", "A Draft Title")).unwrap();
    lib.set_added_value(id, 0, "A Finished Title").unwrap();
    let out = saved(&lib);
    assert!(out.contains("  title = {A Finished Title},"), "{out}");
    assert!(!out.contains("A Draft Title"));
}

#[test]
fn a_field_of_an_entry_the_file_held_is_not_an_added_field() {
    let mut lib = parsed(INPUT);
    assert_eq!(
        lib.set_added_value(citerack_bib::EntryId(0), 0, "x"),
        Err(EditError::Absent)
    );
}
