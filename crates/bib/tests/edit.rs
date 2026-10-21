//! The editing contract: what an edit changes, and what it is not allowed to
//! change. A save is compared byte for byte against the input, so a field nobody
//! touched is held to the file's own spacing rather than to a formatter's.

use citerack_bib::{
    Delimiter, EditError, EntryId, ParseOptions, WriteOptions, parse_str, write_str,
};

const INPUT: &str = "\
% This file was written by hand, and the spacing is deliberate.

@comment{jabref-meta: databaseType:bibtex;}

@Article{smith2024,
  Author   = {Smith, John},
  Title    = {A {Nested} Brace},
  year     = 2024,
  note     = \"a note with {braces}\"
}

@book(doe2019,
  editor = {Doe, Jane},
  publisher = {ACME}
)

% a trailing remark
";

fn parse(text: &str) -> citerack_bib::Library {
    parse_str(text, &ParseOptions::default()).unwrap()
}

fn saved(lib: &citerack_bib::Library) -> String {
    write_str(lib, &WriteOptions::Preserve)
}

#[test]
fn an_untouched_library_saves_byte_for_byte() {
    let lib = parse(INPUT);
    assert!(!lib.is_edited());
    assert_eq!(saved(&lib), INPUT);
}

#[test]
fn an_edited_field_saves_and_reopens_as_edited() {
    let mut lib = parse(INPUT);
    lib.set_field(EntryId(0), 1, "A Shallow Brace").unwrap();
    let out = saved(&lib);
    let again = parse(&out);
    assert_eq!(again.entries()[0].field("title"), Some("A Shallow Brace"));
}

#[test]
fn every_byte_outside_the_edited_value_is_unchanged() {
    let mut lib = parse(INPUT);
    lib.set_field(EntryId(0), 1, "A Shallow Brace").unwrap();
    let out = saved(&lib);
    let before: Vec<&str> = INPUT.split("A {Nested} Brace").collect();
    let after: Vec<&str> = out.split("A Shallow Brace").collect();
    assert_eq!(before, after, "only the value's own bytes moved");
}

#[test]
fn an_edit_keeps_the_delimiter_and_the_spacing_around_it() {
    let mut lib = parse(INPUT);
    lib.set_field_named(EntryId(0), "author", "Smith, Jane")
        .unwrap();
    let out = saved(&lib);
    assert!(out.contains("  Author   = {Smith, Jane},\n"));
}

#[test]
fn an_edit_leaves_a_sibling_entry_untouched() {
    let mut lib = parse(INPUT);
    lib.set_field(EntryId(1), 0, "Roe, Jane").unwrap();
    let out = saved(&lib);
    assert!(out.contains("@Article{smith2024,\n  Author   = {Smith, John},"));
    assert!(out.contains("  editor = {Roe, Jane},"));
    assert!(out.contains("@comment{jabref-meta"));
    assert!(out.contains("% a trailing remark"));
}

#[test]
fn a_parenthesised_entry_keeps_its_parentheses() {
    let mut lib = parse(INPUT);
    lib.set_field(EntryId(1), 1, "Elsewhere").unwrap();
    let out = saved(&lib);
    assert!(out.contains("@book(doe2019,"));
    assert!(out.contains("  publisher = {Elsewhere}\n)"));
}

#[test]
fn a_value_that_would_close_its_entry_is_refused() {
    let mut lib = parse(INPUT);
    assert_eq!(
        lib.set_field(EntryId(0), 1, "Unbalanced } brace"),
        Err(EditError::Unwritable)
    );
    assert_eq!(saved(&lib), INPUT, "a refused edit leaves no trace");
}

#[test]
fn a_number_written_into_a_bare_field_stays_bare() {
    let mut lib = parse(INPUT);
    lib.set_field_named(EntryId(0), "year", "2025").unwrap();
    let out = saved(&lib);
    assert!(out.contains("  year     = 2025,"));
    let again = parse(&out);
    assert_eq!(again.entries()[0].fields[2].delimiter, Delimiter::Bare);
}

#[test]
fn a_word_written_into_a_bare_field_is_refused() {
    let mut lib = parse(INPUT);
    assert_eq!(
        lib.set_field_named(EntryId(0), "year", "twenty four"),
        Err(EditError::Unwritable),
        "a bare value cannot hold a space or a comma"
    );
    assert_eq!(
        lib.set_field_named(EntryId(0), "year", "2024,"),
        Err(EditError::Unwritable),
        "a comma would end the field early"
    );
}

#[test]
fn a_quoted_value_keeps_its_quotes() {
    let mut lib = parse(INPUT);
    lib.set_field_named(EntryId(0), "note", "plain").unwrap();
    let out = saved(&lib);
    assert!(out.contains("  note     = \"plain\"\n"));
    let again = parse(&out);
    assert_eq!(again.entries()[0].fields[3].delimiter, Delimiter::Quotes);
}

#[test]
fn a_quote_written_into_a_quoted_value_is_refused() {
    let mut lib = parse(INPUT);
    assert_eq!(
        lib.set_field_named(EntryId(0), "note", "a \" bare quote"),
        Err(EditError::Unwritable),
        "the quote would end the value early"
    );
}

#[test]
fn renaming_a_key_keeps_the_type_and_every_field() {
    let mut lib = parse(INPUT);
    lib.set_key(EntryId(0), "smith2025").unwrap();
    let out = saved(&lib);
    assert!(out.contains("@Article{smith2025,\n  Author   = {Smith, John},"));
    let again = parse(&out);
    assert_eq!(again.entries()[0].key.as_ref(), "smith2025");
    assert_eq!(again.entries()[0].kind.as_str(), "Article");
}

#[test]
fn a_key_holding_a_brace_or_space_is_refused() {
    let mut lib = parse(INPUT);
    assert_eq!(lib.set_key(EntryId(0), "a b"), Err(EditError::BadKey));
    assert_eq!(lib.set_key(EntryId(0), "a{b"), Err(EditError::BadKey));
    assert_eq!(lib.set_key(EntryId(0), ""), Err(EditError::BadKey));
    assert_eq!(saved(&lib), INPUT);
}

#[test]
fn an_added_field_lands_after_the_last_one() {
    let mut lib = parse(INPUT);
    lib.add_field(EntryId(0), "doi", "10.1000/x").unwrap();
    let out = saved(&lib);
    assert!(
        out.contains("  doi = {10.1000/x},\n}"),
        "kept inside the entry"
    );
    let again = parse(&out);
    assert_eq!(again.entries()[0].field("doi"), Some("10.1000/x"));
    assert_eq!(again.len(), 2);
}

#[test]
fn an_added_field_with_a_name_that_cannot_be_written_is_refused() {
    let mut lib = parse(INPUT);
    assert_eq!(
        lib.add_field(EntryId(0), "a b", "x"),
        Err(EditError::BadName)
    );
    assert_eq!(saved(&lib), INPUT);
}

#[test]
fn reverting_an_entry_saves_the_file_as_it_was() {
    let mut lib = parse(INPUT);
    lib.set_field(EntryId(0), 1, "Something Else").unwrap();
    lib.set_key(EntryId(1), "doe2020").unwrap();
    assert_eq!(lib.edited_count(), 2);
    assert_ne!(saved(&lib), INPUT);
    lib.revert(EntryId(0)).unwrap();
    lib.revert(EntryId(1)).unwrap();
    assert!(!lib.is_edited());
    assert_eq!(saved(&lib), INPUT, "the file comes back exactly");
}

#[test]
fn two_entries_edited_apart_save_both() {
    let mut lib = parse(INPUT);
    lib.set_field(EntryId(0), 1, "First").unwrap();
    lib.set_field(EntryId(1), 0, "Second").unwrap();
    let out = saved(&lib);
    assert!(out.contains("{First}"));
    assert!(out.contains("{Second}"));
    let again = parse(&out);
    assert_eq!(again.len(), 2);
}

#[test]
fn saving_twice_writes_the_same_bytes() {
    let mut lib = parse(INPUT);
    lib.set_field(EntryId(0), 1, "Stable").unwrap();
    assert_eq!(saved(&lib), saved(&lib));
}

#[test]
fn a_refused_edit_leaves_the_value_as_it_was() {
    let mut lib = parse(INPUT);
    lib.set_field(EntryId(0), 1, "Accepted").unwrap();
    let _ = lib.set_field(EntryId(0), 1, "Broken }");
    assert_eq!(
        lib.entry(EntryId(0)).unwrap().field("title"),
        Some("Accepted")
    );
    assert_eq!(lib.field(EntryId(0), 1), Some("Accepted"));
}

#[test]
fn a_field_index_past_the_end_is_refused() {
    let mut lib = parse(INPUT);
    assert_eq!(lib.set_field(EntryId(0), 99, "x"), Err(EditError::Absent));
    assert_eq!(lib.set_field(EntryId(99), 0, "x"), Err(EditError::Absent));
}
