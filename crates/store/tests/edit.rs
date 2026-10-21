//! The round trip a library's owner takes: open a file, fix something, save it,
//! and open it again. The test holds a save to the bytes it should have changed,
//! so the untouched parts of the file are compared rather than eyeballed.

use std::path::PathBuf;

use citerack_bib::EditError;
use citerack_store::Session;

/// A file of its own per test, removed when the test ends.
struct TempFile {
    path: PathBuf,
}

impl TempFile {
    fn holding(name: &str, text: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "citerack-{name}-{}-{:?}.bib",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::write(&path, text).expect("the file");
        Self { path }
    }

    fn read(&self) -> String {
        std::fs::read_to_string(&self.path).expect("the file")
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

const INPUT: &str = "\
% Handwritten, and the spacing is deliberate.

@Article{smith2024,
  Author   = {Smith, John},
  Title    = {A Study of {Nested} Braces},
  year     = 2024,
}

@book(doe2019,
  editor = {Doe, Jane},
  publisher = {ACME}
)

% a trailing remark
";

#[test]
fn an_edited_field_survives_a_save_and_a_reopen() {
    let file = TempFile::holding("edit-field", INPUT);
    let mut session = Session::open(&file.path).expect("an open");
    session.set_field(0, 1, "A Study of No Braces").unwrap();
    session.save().expect("a save");

    let again = Session::open(&file.path).expect("a reopen");
    assert_eq!(again.value(0, 1), Some("A Study of No Braces"));
    assert_eq!(again.len(), 2);
}

#[test]
fn only_the_edited_value_moves_in_the_file() {
    let file = TempFile::holding("edit-bytes", INPUT);
    let mut session = Session::open(&file.path).expect("an open");
    session.set_field(0, 1, "A Study of No Braces").unwrap();
    session.save().expect("a save");

    let before: Vec<&str> = INPUT.split("A Study of {Nested} Braces").collect();
    let saved = file.read();
    let after: Vec<&str> = saved.split("A Study of No Braces").collect();
    assert_eq!(before, after, "every other byte is the file's own");
}

#[test]
fn a_save_that_changed_nothing_rewrites_the_same_bytes() {
    let file = TempFile::holding("edit-none", INPUT);
    let mut session = Session::open(&file.path).expect("an open");
    assert!(!session.is_edited());
    session.save().expect("a save");
    assert_eq!(file.read(), INPUT);
}

#[test]
fn a_field_added_and_saved_is_in_the_file_after_a_reopen() {
    let file = TempFile::holding("edit-add", INPUT);
    let mut session = Session::open(&file.path).expect("an open");
    session.add_field(1, "note", "kept").unwrap();
    session.save().expect("a save");

    let again = Session::open(&file.path).expect("a reopen");
    let entry = &again.library().entries()[1];
    assert_eq!(entry.field("note"), Some("kept"));
    assert_eq!(entry.key.as_ref(), "doe2019");
}

#[test]
fn a_renamed_key_is_what_a_search_finds_after_a_reopen() {
    let file = TempFile::holding("edit-key", INPUT);
    let mut session = Session::open(&file.path).expect("an open");
    session.set_key(0, "smith2025").unwrap();
    session.save().expect("a save");

    let mut again = Session::open(&file.path).expect("a reopen");
    let found = again.search("Smith");
    assert_eq!(found.rows.len(), 1);
    assert_eq!(again.library().entries()[0].key.as_ref(), "smith2025");
}

#[test]
fn an_edit_that_cannot_be_written_leaves_the_file_alone() {
    let file = TempFile::holding("edit-refused", INPUT);
    let mut session = Session::open(&file.path).expect("an open");
    let refused = session.set_field(0, 1, "an unbalanced } brace");
    assert_eq!(refused, Err(EditError::Unwritable));
    assert!(!session.is_edited());
    session.save().expect("a save");
    assert_eq!(file.read(), INPUT, "a refused edit writes nothing");
}

#[test]
fn reverting_every_edit_puts_the_file_back_byte_for_byte() {
    let file = TempFile::holding("edit-revert", INPUT);
    let mut session = Session::open(&file.path).expect("an open");
    session.set_field(0, 1, "Something Else").unwrap();
    session.set_key(1, "doe2020").unwrap();
    assert!(session.is_edited());
    session.revert(0).unwrap();
    session.revert(1).unwrap();
    assert!(!session.is_edited());
    session.save().expect("a save");
    assert_eq!(file.read(), INPUT);
}

#[test]
fn a_search_after_an_edit_reads_the_edited_value() {
    let file = TempFile::holding("edit-search", INPUT);
    let mut session = Session::open(&file.path).expect("an open");
    assert_eq!(session.search("Nested").rows.len(), 1);
    session.set_field(0, 1, "A Study of No Braces").unwrap();
    assert_eq!(
        session.search("Nested").rows.len(),
        0,
        "the old word is gone"
    );
    assert_eq!(session.search("Braces").rows.len(), 1);
}

#[test]
fn a_save_leaves_nothing_outstanding() {
    let file = TempFile::holding("save-clears", INPUT);
    let mut session = Session::open(&file.path).expect("an open");
    assert!(
        !session.is_edited(),
        "a file just opened is not unsaved work"
    );
    session.set_field(0, 1, "A Study of No Braces").unwrap();
    assert!(session.is_edited(), "an edit is unsaved work");
    session.save().expect("a save");
    assert!(
        !session.is_edited(),
        "the bytes on disk are the model's now"
    );
}

#[test]
fn a_second_save_after_a_first_one_is_still_the_same_file() {
    let file = TempFile::holding("save-twice", INPUT);
    let mut session = Session::open(&file.path).expect("an open");
    session.set_field(0, 1, "A Study of No Braces").unwrap();
    session.save().expect("a save");
    let once = file.read();
    session.save().expect("a second save");
    assert_eq!(file.read(), once, "saving twice wrote nothing new");
}

#[test]
fn an_edit_after_a_save_is_unsaved_again() {
    let file = TempFile::holding("edit-after-save", INPUT);
    let mut session = Session::open(&file.path).expect("an open");
    session.set_field(0, 1, "First").unwrap();
    session.save().expect("a save");
    session.set_field(0, 1, "Second").unwrap();
    assert!(session.is_edited(), "a later edit is new work");
    session.save().expect("a second save");
    assert!(file.read().contains("Second"), "the later edit was written");
}
