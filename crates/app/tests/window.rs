//! The window driven without a display: a session goes in, clicks and typed text
//! go through the paths a mouse and a keyboard take, and what a user would read
//! comes back out of the accessibility tree.

use std::path::PathBuf;

use accesskit::Role;
use citerack::App;
use citerack_store::Session;
use egui_kittest::Harness;
use egui_kittest::kittest::{AccessKitNode, Queryable};

const INPUT: &str = "\
@Article{smith2024,
  Author   = {Smith, John},
  Title    = {A Study of Braces},
  Year     = 2024,
}

@Book{jones1999,
  Author   = {Jones, Ann},
  Title    = {Another Book},
  Year     = 1999,
}
";

fn session() -> Session {
    Session::from_text(
        PathBuf::from("window.bib"),
        INPUT,
        &citerack_bib::ParseOptions { recover: true },
    )
    .expect("the input parses")
}

fn harness(app: App) -> Harness<'static, App> {
    let mut harness = Harness::builder()
        .with_size(egui::vec2(1000.0, 700.0))
        .build_state(|ctx, app: &mut App| app.draw(ctx), app);
    harness.run();
    harness
}

/// One entry open with the detail pane already showing it.
fn opened() -> Harness<'static, App> {
    let mut harness = harness(App::with_session(session()));
    harness.state_mut().select(Some(0));
    harness.run();
    harness
}

/// The value of every field box, in the order the pane draws them. The filter
/// box is a text input too and comes first, hence the skip.
fn boxes(harness: &Harness<'_, App>) -> Vec<String> {
    harness
        .query_all_by_role(Role::TextInput)
        .filter_map(|node| node.value())
        .skip(1)
        .collect()
}

/// The box holding `value`, so a test names a field without counting fields.
fn field<'a>(harness: &'a Harness<'_, App>, value: &'a str) -> egui_kittest::Node<'a> {
    harness.get_by(|node: &AccessKitNode| {
        node.role() == Role::TextInput && node.value().as_deref() == Some(value)
    })
}

/// Selects the year box, empties it, types `text` and leaves it, which is the
/// sequence a user goes through to correct a value.
fn retype_year(harness: &mut Harness<'_, App>, text: &str) {
    field(harness, "2024").click();
    harness.run();
    // One event per frame, since a frame with a key in it ends the harness pass.
    harness.key_press(egui::Key::End);
    harness.run();
    for _ in 0..4 {
        harness.key_press(egui::Key::Backspace);
        harness.run();
    }
    debug_assert_eq!(
        boxes(harness)[2],
        "",
        "four presses empty a four-digit year"
    );
    harness
        .input_mut()
        .events
        .push(egui::Event::Text(text.to_owned()));
    harness.run();
    harness.key_press(egui::Key::Tab);
    harness.run();
}

#[test]
fn the_detail_pane_shows_the_selected_entry_field_by_field() {
    let harness = opened();
    assert_eq!(
        boxes(&harness),
        ["Smith, John", "A Study of Braces", "2024"],
        "one box per field, in file order"
    );
    assert_eq!(
        harness.query_all_by_label("smith2024").count(),
        2,
        "the table row and the pane heading both name the key"
    );
}

#[test]
fn the_pane_asks_for_a_selection_before_one_is_made() {
    let harness = harness(App::with_session(session()));
    harness.get_by_label("Select an entry to see its fields.");
    assert!(
        boxes(&harness).is_empty(),
        "no field is drawn without a row: {:?}",
        boxes(&harness)
    );
}

#[test]
fn typing_into_a_field_and_leaving_it_edits_the_entry() {
    let mut harness = opened();
    retype_year(&mut harness, "2025");

    assert_eq!(harness.state().status(), "Edited, not saved yet");
    assert!(harness.state().session().unwrap().is_edited());
    assert_eq!(boxes(&harness)[2], "2025", "the pane shows the new value");
}

#[test]
fn a_save_after_editing_is_offered_with_a_marker() {
    let mut harness = opened();
    harness.get_by_label("Save");
    retype_year(&mut harness, "2025");
    harness.get_by_label("Save *");
}

#[test]
fn a_value_that_cannot_be_written_stays_in_its_box() {
    let mut harness = opened();
    // A bare value holds no space, so this one is refused rather than written.
    retype_year(&mut harness, " 2024-05");

    assert_eq!(
        harness.state().status(),
        "2 entries",
        "a refused edit is not reported as one"
    );
    assert_eq!(
        boxes(&harness)[2],
        " 2024-05",
        "the typing is still in the box to be corrected"
    );
    assert!(!harness.state().session().unwrap().is_edited());
}

#[test]
fn ctrl_z_puts_back_the_value_that_was_there_before() {
    let mut harness = opened();
    retype_year(&mut harness, "2025");
    assert_eq!(boxes(&harness)[2], "2025");

    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
    harness.run();

    assert_eq!(boxes(&harness)[2], "2024");
    assert_eq!(harness.state().status(), "Undid editing a field");
    assert!(!harness.state().session().unwrap().is_edited());
}

#[test]
fn a_save_after_ctrl_z_writes_the_text_the_edit_replaced() {
    let mut harness = opened();
    retype_year(&mut harness, "2025");
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
    harness.run();

    let session = harness.state().session().unwrap();
    let written = citerack_bib::write_str(session.library(), &citerack_bib::WriteOptions::Preserve);
    assert_eq!(written, INPUT, "a save now writes the file as it was");
}

#[test]
fn ctrl_shift_z_puts_the_edit_back_after_a_ctrl_z() {
    let mut harness = opened();
    retype_year(&mut harness, "2025");
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
    harness.run();
    assert_eq!(boxes(&harness)[2], "2024");

    harness.key_press_modifiers(
        egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
        egui::Key::Z,
    );
    harness.run();

    assert_eq!(boxes(&harness)[2], "2025", "the redo shows the edit again");
    assert_eq!(harness.state().status(), "Redid editing a field");
    assert!(harness.state().session().unwrap().is_edited());
}

#[test]
fn an_undo_with_nothing_to_undo_leaves_the_file_alone() {
    let mut harness = opened();
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
    harness.run();

    assert_eq!(harness.state().status(), "Nothing to undo");
    assert!(!harness.state().session().unwrap().is_edited());
}

#[test]
fn a_search_after_ctrl_z_finds_the_value_that_came_back() {
    let mut harness = opened();
    retype_year(&mut harness, "2025");
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
    harness.run();

    // The pane reads the library, so the year it shows is the indexed one.
    let session = harness.state().session().unwrap();
    let mut again = Session::from_text(
        PathBuf::from("window.bib"),
        &citerack_bib::write_str(session.library(), &citerack_bib::WriteOptions::Preserve),
        &citerack_bib::ParseOptions { recover: true },
    )
    .expect("what a save would write parses");
    assert_eq!(again.search("2024").rows, vec![0], "the old year is back");
    assert_eq!(again.search("2025").rows.len(), 0, "the new one is gone");
}
