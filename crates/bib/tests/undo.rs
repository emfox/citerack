//! A change is a set of entries captured before the first of its edits, and
//! putting one back is what an undo holds. These tests read the two states back
//! through the writer, so a restore is compared in bytes rather than in intent.

use citerack_bib::{EditError, EntryId, Library, ParseOptions, WriteOptions, parse_str, write_str};

const INPUT: &str = "\
% A hand-formatted library.

@Article{smith2024,
  Author   = {Smith, John},
  Title    = {A Study of Braces},
  year     = 2024,
}

@Book{jones1999,
  Author   = {Jones, Ann},
  Title    = {Another Book},
}

@Misc{lee2001,
  note = {kept},
}
";

fn library() -> Library {
    parse_str(INPUT, &ParseOptions::default()).expect("the input parses")
}

fn written(library: &Library) -> String {
    write_str(library, &WriteOptions::Preserve)
}

#[test]
fn the_state_before_a_change_is_the_file_itself() {
    let mut lib = library();
    assert!(lib.pending_before().is_empty(), "nothing has been edited");

    lib.set_field_named(EntryId(0), "year", "2025")
        .expect("a bare year takes four digits");
    let before = lib.pending_before();
    assert_eq!(before.len(), 1, "one entry was touched");
    assert_eq!(before[0].entry, 0);
    assert_eq!(before[0].text, None, "the entry was the file's own");
    assert_eq!(before[0].model.key.as_ref(), "smith2024");
}

#[test]
fn a_second_edit_in_the_same_change_is_not_recorded_twice() {
    let mut lib = library();
    lib.set_field_named(EntryId(0), "year", "2025").unwrap();
    lib.set_field_named(EntryId(0), "title", "A Study").unwrap();
    let before = lib.pending_before();
    assert_eq!(
        before.len(),
        1,
        "the state wanted is the first one, not the middle"
    );
    assert_eq!(before[0].model.field("year"), Some("2024"));
}

#[test]
fn restoring_what_was_before_writes_the_file_again() {
    let mut lib = library();
    lib.set_field_named(EntryId(0), "year", "2025").unwrap();
    let before = lib.pending_before();
    let edited = lib.current_states();
    assert_ne!(written(&lib), INPUT, "the edit reached the file");

    lib.restore(&before).expect("the file's own bytes fit");
    assert_eq!(written(&lib), INPUT, "every byte is back");
    assert!(!lib.is_edited());

    lib.restore(&edited).expect("the edited bytes still fit");
    assert!(lib.is_edited());
    assert_eq!(lib.values(EntryId(0))[2], "2025");
}

#[test]
fn a_change_that_touches_two_entries_undoes_as_one() {
    let mut lib = library();
    lib.set_field_named(EntryId(0), "year", "2025").unwrap();
    lib.set_key(EntryId(1), "jones1999b").unwrap();
    let before = lib.pending_before();
    assert_eq!(before.len(), 2);

    lib.restore(&before).unwrap();
    assert_eq!(written(&lib), INPUT, "both entries went back together");
}

#[test]
fn a_value_typed_back_the_way_it_was_is_not_a_change() {
    let mut lib = library();
    assert_eq!(
        lib.values(EntryId(0))[2],
        "2024",
        "the year is the third field"
    );
    assert!(
        !lib.replace_field(EntryId(0), 2, "2024").unwrap(),
        "the same text is not a change"
    );
    assert!(lib.pending_before().is_empty(), "nothing to undo");
    assert!(!lib.is_edited());
}

#[test]
fn seeding_a_change_leaves_the_state_sealed() {
    let mut lib = library();
    lib.set_field_named(EntryId(0), "year", "2025").unwrap();
    lib.seal();
    assert!(lib.pending_before().is_empty(), "the change was committed");
    lib.set_field_named(EntryId(0), "year", "2026").unwrap();
    assert_eq!(lib.pending_before().len(), 1, "the next change starts anew");
}

#[test]
fn a_restore_that_cannot_be_written_changes_nothing() {
    let mut lib = library();
    lib.set_field_named(EntryId(0), "year", "2025").unwrap();
    let mut broken = lib.current_states();
    broken.truncate(1);
    assert_eq!(broken.len(), 1);
    broken[0].text = Some("{ unbalanced".into());
    assert!(matches!(
        lib.restore(&broken),
        Err(EditError::NotInFile) | Err(EditError::Unwritable)
    ));
    assert_eq!(lib.values(EntryId(0))[2], "2025", "still the edit");
}

#[test]
fn a_reverted_entry_has_nothing_left_to_keep() {
    let mut lib = library();
    lib.set_field_named(EntryId(0), "year", "2025").unwrap();
    lib.seal();
    lib.revert(EntryId(0)).unwrap();
    assert_eq!(
        written(&lib),
        INPUT,
        "the entry is the file's own bytes again"
    );
}
