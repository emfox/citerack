//! Reversing and replaying edits: undo an edit, save, and the file is the bytes
//! it started as. A run of typing is one step; a pause starts another.

use std::path::PathBuf;
use std::time::{Duration, Instant};

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

/// A session holding a file at `INPUT`, with nothing edited yet.
fn opened(name: &str) -> (TempFile, Session) {
    let file = TempFile::holding(name, INPUT);
    let session = Session::open(&file.path).expect("an open");
    (file, session)
}

#[test]
fn undoing_an_edit_puts_the_text_back() {
    let (_file, mut session) = opened("undo-text");
    session.set_field(0, 1, "Something Else").unwrap();
    session.settle();
    assert!(session.can_undo());

    assert_eq!(session.undo(), Some("editing a field"));
    assert_eq!(session.value(0, 1), Some("A Study of {Nested} Braces"));
    assert!(!session.can_undo(), "one change, one step back");
    assert_eq!(session.undo(), None, "nothing left to reverse");
}

#[test]
fn a_save_after_an_undo_writes_the_bytes_the_file_had() {
    let (file, mut session) = opened("undo-save");
    session.set_field(0, 1, "Something Else").unwrap();
    session.settle();
    session.undo().expect("an undo");
    session.save().expect("a save");

    assert_eq!(file.read(), INPUT, "the file is as it was, byte for byte");
}

#[test]
fn only_the_undone_edit_gives_way() {
    let (file, mut session) = opened("undo-one-of-two");
    session.set_field(0, 1, "First Rewrite").unwrap();
    session.settle();
    session.set_key(0, "smith2025").unwrap();
    session.settle();

    assert_eq!(session.undo(), Some("renaming an entry"));
    assert_eq!(
        session.value(0, 1),
        Some("First Rewrite"),
        "the first stands"
    );
    assert_eq!(session.library().entries()[0].key.as_ref(), "smith2024");
    assert_eq!(session.undo(), Some("editing a field"));
    session.save().expect("a save");
    assert_eq!(file.read(), INPUT);
}

#[test]
fn a_redo_puts_the_edit_back() {
    let (_file, mut session) = opened("undo-redo");
    session.set_field(0, 1, "Something Else").unwrap();
    session.settle();
    session.undo().expect("an undo");
    assert!(session.can_redo());

    assert_eq!(session.redo(), Some("editing a field"));
    assert_eq!(session.value(0, 1), Some("Something Else"));
    assert!(session.can_undo());
    assert!(!session.can_redo());
}

#[test]
fn an_edit_after_an_undo_drops_what_could_have_been_redone() {
    let (_file, mut session) = opened("undo-branch");
    session.set_field(0, 1, "First Rewrite").unwrap();
    session.settle();
    session.undo().expect("an undo");
    session.set_field(0, 1, "Second Rewrite").unwrap();
    session.settle();

    assert!(!session.can_redo(), "a new edit is a new history");
    assert_eq!(session.redo(), None);
    assert_eq!(session.value(0, 1), Some("Second Rewrite"));
}

#[test]
fn typing_without_a_pause_is_one_step() {
    let (_file, mut session) = opened("undo-run");
    let at = Instant::now();
    let within = Duration::from_millis(800);
    for (index, text) in ["A Stu", "A Stud", "A Study"].iter().enumerate() {
        let now = at + Duration::from_millis(100 * index as u64);
        session.settle_at(now, within);
        session.set_field(0, 1, text).expect("a keystroke's worth");
        session.edited_at(now);
    }
    session.settle_at(at + Duration::from_millis(300), within);
    assert_eq!(session.value(0, 1), Some("A Study"));

    assert_eq!(session.undo(), Some("editing a field"));
    assert_eq!(
        session.value(0, 1),
        Some("A Study of {Nested} Braces"),
        "the whole run goes at once, not the last keystroke"
    );
}

#[test]
fn a_pause_between_keystrokes_starts_a_second_step() {
    let (_file, mut session) = opened("undo-pause");
    let at = Instant::now();
    let within = Duration::from_millis(800);
    session.set_field(0, 1, "A Study, rewritten").unwrap();
    session.edited_at(at);
    session.settle_at(at + Duration::from_millis(100), within);
    assert!(!session.can_undo(), "the run is still open");

    // The user paused for two seconds, then went on typing.
    let later = at + Duration::from_millis(2100);
    session.settle_at(later, within);
    assert!(session.can_undo(), "the pause closed the first step");
    session.set_field(0, 1, "A Study, rewritten again").unwrap();
    session.edited_at(later);

    assert_eq!(session.undo(), Some("editing a field"));
    assert_eq!(session.value(0, 1), Some("A Study, rewritten"));
    assert!(session.can_undo());
    session.undo().expect("a second undo");
    assert_eq!(session.value(0, 1), Some("A Study of {Nested} Braces"));
}

#[test]
fn an_edit_over_two_entries_reverses_as_one_step() {
    let (_file, mut session) = opened("undo-two-entries");
    session.set_field(0, 1, "First Rewrite").unwrap();
    session.set_field(1, 0, "Doe, Jane Q.").unwrap();
    session.settle();

    assert_eq!(
        session.undo(),
        Some("an edit to several entries"),
        "both entries were one visit to the screen"
    );
    assert_eq!(session.value(0, 1), Some("A Study of {Nested} Braces"));
    assert_eq!(session.value(1, 0), Some("Doe, Jane"));
    assert!(!session.can_undo());
}

#[test]
fn an_undo_settles_what_was_still_being_typed() {
    let (_file, mut session) = opened("undo-open");
    session.set_field(0, 1, "A Study, rewritten").unwrap();
    assert!(!session.can_undo(), "no step has closed yet");

    assert_eq!(session.undo(), Some("editing a field"));
    assert_eq!(session.value(0, 1), Some("A Study of {Nested} Braces"));
}

#[test]
fn a_search_after_an_undo_reads_the_text_the_file_had() {
    let (_file, mut session) = opened("undo-search");
    session.set_field(0, 1, "No Braces At All").unwrap();
    session.settle();
    assert_eq!(session.search("Nested").rows.len(), 0);

    session.undo().expect("an undo");
    assert_eq!(
        session.search("Nested").rows,
        vec![0],
        "the word the edit removed is back in the index"
    );
    assert_eq!(session.search("Braces At All").rows.len(), 0);
}

#[test]
fn a_renamed_key_undone_is_the_key_the_file_had() {
    let (file, mut session) = opened("undo-key");
    session.set_key(0, "smith2025").unwrap();
    session.settle();

    session.undo().expect("an undo");
    assert_eq!(session.library().entries()[0].key.as_ref(), "smith2024");
    session.save().expect("a save");
    assert_eq!(file.read(), INPUT, "the key in the file is the old one");
}

#[test]
fn a_field_added_is_taken_away_by_an_undo() {
    let (file, mut session) = opened("undo-add-field");
    session.add_field(1, "note", "kept").unwrap();
    session.settle();

    assert_eq!(session.undo(), Some("adding a field"));
    assert_eq!(session.library().entries()[1].field("note"), None);
    session.save().expect("a save");
    assert_eq!(file.read(), INPUT);
}

#[test]
fn a_reverted_entry_is_its_own_step_to_redo() {
    let (_file, mut session) = opened("undo-revert");
    session.set_field(0, 1, "Something Else").unwrap();
    session.settle();
    session.revert(0).unwrap();
    session.settle();

    assert_eq!(
        session.undo(),
        Some("editing a field"),
        "the revert, undone"
    );
    assert_eq!(session.value(0, 1), Some("Something Else"));
    session.undo().expect("the edit itself, undone");
    assert_eq!(session.value(0, 1), Some("A Study of {Nested} Braces"));
    assert!(!session.can_undo());
}

#[test]
fn a_library_with_nothing_done_has_nothing_to_reverse() {
    let (_file, mut session) = opened("undo-empty");
    assert!(!session.can_undo());
    assert!(!session.can_redo());
    assert_eq!(session.undo(), None);
    assert_eq!(session.redo(), None);
    session.settle_all();
    assert!(
        !session.can_undo(),
        "settling an untouched file adds no step"
    );
}

#[test]
fn undoing_every_step_yields_the_file_the_run_started_with() {
    let (file, mut session) = opened("undo-all");
    session.set_field(0, 1, "First Rewrite").unwrap();
    session.settle();
    session.set_key(1, "doe2020").unwrap();
    session.settle();
    session.add_field(1, "note", "kept").unwrap();
    session.settle();
    session.save().expect("a save");
    assert_ne!(file.read(), INPUT);

    let mut guard = 0;
    while session.undo().is_some() {
        guard += 1;
        assert!(guard < 10, "the history runs out rather than looping");
    }
    assert!(!session.is_edited(), "nothing of the run is left");
    session.save().expect("a save");
    assert_eq!(file.read(), INPUT, "undone all the way is the file it was");
}
