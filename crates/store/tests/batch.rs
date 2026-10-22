//! Writing one field across many entries: what it changes, and what it must not
//! touch. The file is compared byte for byte against the input, so a batch edit is
//! held to the same promise as a single one.

use std::path::PathBuf;

use citerack_store::Session;

/// A file of its own per test, removed when the test ends.
struct TempFile {
    path: PathBuf,
}

impl TempFile {
    fn holding(name: &str, text: &str) -> Self {
        let path = std::env::temp_dir().join(format!("citerack-batch-{name}.bib"));
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
  Title    = {A Study of Braces},
  year     = 2024,
}

@Book{doe2019,
  author    = {Doe, Jane},
  title     = {Another Book},
  publisher = {Knot Press},
  keywords  = {old, draft}
}

@Misc{roe2020,
  author = {Roe, Ann},
  title  = {A Note},
}

% a trailing remark
";

fn opened(name: &str) -> (TempFile, Session) {
    let file = TempFile::holding(name, INPUT);
    let session = Session::open(&file.path).expect("an open");
    (file, session)
}

#[test]
fn adding_a_keyword_to_three_entries_changes_only_their_keyword_lines() {
    let (file, mut session) = opened("keywords");
    let (written, refused) = session.set_field_in(&[0, 1, 2], "keywords", "reviewed");
    assert_eq!((written, refused), (3, 0));
    session.save().expect("a save");

    let expected = INPUT
        .replace(
            "  year     = 2024,\n",
            "  year     = 2024,\n  keywords = {reviewed},\n",
        )
        .replace("  keywords  = {old, draft}\n", "  keywords  = {reviewed}\n");
    let expected = expected.replace(
        "  title  = {A Note},\n",
        "  title  = {A Note},\n  keywords = {reviewed},\n",
    );
    assert_eq!(file.read(), expected, "nothing outside the field moved");
}

#[test]
fn the_rest_of_every_entry_keeps_its_own_spacing() {
    let (file, mut session) = opened("spacing");
    session.set_field_in(&[0, 2], "keywords", "todo");
    session.save().expect("a save");

    let text = file.read();
    assert!(
        text.contains("  Author   = {Smith, John},"),
        "the alignment of an entry nobody renamed is the file's own"
    );
    assert!(
        text.contains("@Book{doe2019,\n  author    = {Doe, Jane},"),
        "an entry outside the batch keeps its type's case and its alignment"
    );
    assert!(text.contains("% a trailing remark"), "a comment survives");
}

#[test]
fn one_undo_takes_the_whole_batch_back() {
    let (file, mut session) = opened("undo");
    session.set_field_in(&[0, 1, 2], "keywords", "reviewed");
    assert_eq!(session.undo(), Some("an edit to several entries"));
    assert!(!session.can_undo(), "a batch is one step, not three");
    session.save().expect("a save");
    assert_eq!(file.read(), INPUT, "the bytes are the ones it started with");
}

#[test]
fn an_entry_that_cannot_take_the_value_is_counted_and_left_alone() {
    let (file, mut session) = opened("refused");
    // A bare field holds one word, and "in press" is two: that entry is left as
    // it was, and the one whose field is braced takes the value.
    let (written, refused) = session.set_field_in(&[0, 1], "year", "in press");
    assert_eq!((written, refused), (1, 1));
    assert_eq!(session.value(0, 2), Some("2024"), "the year is as it was");
    session.save().expect("a save");
    assert!(file.read().contains("  year     = 2024,"));
    // The entry that took it added the field at its end, in its own indent. A
    // bare field holds one word, so the two-word value is written braced.
    assert!(
        file.read().contains(
            "  publisher = {Knot Press},\n  keywords  = {old, draft},\n  year = {in press},"
        ),
        "the entry that took it kept everything else and added one field"
    );
}

#[test]
fn a_batch_that_changes_nothing_is_not_an_undo_step() {
    let (_file, mut session) = opened("noop");
    session.set_field_in(&[0], "keywords", "same");
    session.settle();
    session.set_field_in(&[0], "keywords", "same");
    assert_eq!(
        session.undo(),
        Some("adding a field"),
        "writing the value it already holds is not a second change"
    );
}
