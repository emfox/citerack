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
  Journal  = {Journal of Braces},
}

@Book{jones1999,
  Author    = {Jones, Ann},
  Title     = {Another Book},
  Year      = 1999,
  Publisher = {Knot Press},
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

/// The value of every field box, in the order the pane draws them. The filter and
/// the key pattern are text inputs too, and a field's box is the only one drawn
/// inside the right-hand pane.
fn boxes(harness: &Harness<'_, App>) -> Vec<String> {
    harness
        .query_all_by_role(Role::TextInput)
        .filter(|node| node.rect().min.x > 600.0)
        .filter_map(|node| node.value())
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
    // Escape leaves the box, which commits what is in it. Tab would walk to the
    // next field instead: the pane owns Tab while a box holds the cursor.
    harness.key_press(egui::Key::Escape);
    harness.run();
}

#[test]
fn the_detail_pane_shows_the_selected_entry_field_by_field() {
    let harness = opened();
    assert_eq!(
        boxes(&harness),
        [
            "Smith, John",
            "A Study of Braces",
            "2024",
            "Journal of Braces"
        ],
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

#[test]
fn ctrl_f_puts_the_cursor_in_the_filter_box() {
    let mut harness = opened();
    harness.state_mut().select(Some(0));
    harness.run();

    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::F);
    harness.run();

    assert!(
        harness.state().filter_focused(&harness.ctx),
        "the filter box has the cursor after Ctrl+F"
    );
}

#[test]
fn ctrl_s_writes_the_library_to_the_file_it_came_from() {
    let dir = std::env::temp_dir().join(format!("citerack-ctrl-s-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a directory");
    let path = dir.join("library.bib");
    std::fs::write(&path, INPUT).expect("the file");
    let session = Session::open(&path).expect("an open");

    let mut harness = harness(App::with_session(session));
    harness.state_mut().select(Some(0));
    harness.run();
    retype_year(&mut harness, "2025");
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::S);
    harness.run();

    assert_eq!(harness.state().status(), "Saved", "the key saved the file");
    let written = std::fs::read_to_string(&path).expect("the file");
    assert!(written.contains("2025"), "the edit reached the disk");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn ctrl_f_reaches_the_filter_from_a_field_box() {
    let mut harness = opened();
    // A field box holds the cursor, which is where a user is after an edit.
    field(&harness, "Smith, John").click();
    harness.run();

    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::F);
    harness.run();

    assert!(
        harness.state().filter_focused(&harness.ctx),
        "Ctrl+F moves the cursor out of the field and into the filter"
    );
}

#[test]
fn typing_after_ctrl_f_narrows_the_table() {
    let mut harness = opened();
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::F);
    harness.run();
    assert!(
        harness.state().filter_focused(&harness.ctx),
        "the typing below needs the filter to have the cursor"
    );

    harness
        .input_mut()
        .events
        .push(egui::Event::Text(String::from("jones")));
    // The filter waits for the typing to stop, and each step reads the real
    // clock, so the wait has to pass in real time before a frame searches.
    for _ in 0..8 {
        std::thread::sleep(std::time::Duration::from_millis(40));
        harness.step();
    }

    assert_eq!(
        harness.state().session().unwrap().order(),
        [1],
        "only the entry holding the typed word is shown"
    );
}

#[test]
fn ctrl_o_asks_for_a_file_without_disturbing_what_is_open() {
    let mut harness = opened();
    let before = harness.state().status().to_owned();

    // No display means no dialog, so the key finds no file. What matters here is
    // that the window survives the question and keeps its library.
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::O);
    harness.run();

    assert!(harness.state().opening().is_none(), "no file was chosen");
    assert_eq!(harness.state().status(), before, "the status is unchanged");
    assert!(
        harness.state().session().is_some(),
        "the library is still open"
    );
}

#[test]
fn the_down_key_walks_the_table_and_stops_at_the_last_row() {
    let mut harness = opened();
    assert_eq!(harness.state().selected(), Some(0));

    harness.key_press(egui::Key::ArrowDown);
    harness.run();
    assert_eq!(harness.state().selected(), Some(1), "the second row");

    harness.key_press(egui::Key::ArrowDown);
    harness.run();
    assert_eq!(harness.state().selected(), Some(1), "the last row holds");
}

#[test]
fn the_up_key_walks_back_and_stops_at_the_first_row() {
    let mut harness = opened();
    harness.state_mut().select(Some(1));
    harness.run();

    harness.key_press(egui::Key::ArrowUp);
    harness.run();
    assert_eq!(harness.state().selected(), Some(0), "the first row");

    harness.key_press(egui::Key::ArrowUp);
    harness.run();
    assert_eq!(harness.state().selected(), Some(0), "the first row holds");
}

#[test]
fn a_key_move_shows_the_row_it_landed_on() {
    let mut harness = opened();
    harness.key_press(egui::Key::ArrowDown);
    harness.run();

    assert_eq!(
        boxes(&harness),
        ["Jones, Ann", "Another Book", "1999", "Knot Press"],
        "the pane follows the selection"
    );
}

#[test]
fn the_first_arrow_picks_a_row_when_none_is_picked() {
    let mut harness = harness(App::with_session(session()));
    assert_eq!(harness.state().selected(), None);

    harness.key_press(egui::Key::ArrowDown);
    harness.run();
    assert_eq!(harness.state().selected(), Some(0), "down takes the first");

    harness.state_mut().select(None);
    harness.run();
    harness.key_press(egui::Key::ArrowUp);
    harness.run();
    assert_eq!(harness.state().selected(), Some(1), "up takes the last");
}

#[test]
fn an_arrow_in_a_field_box_moves_the_caret_not_the_selection() {
    let mut harness = opened();
    field(&harness, "Smith, John").click();
    harness.run();

    harness.key_press(egui::Key::ArrowDown);
    harness.run();

    assert_eq!(
        harness.state().selected(),
        Some(0),
        "the table did not move while a box held the cursor"
    );
}

#[test]
fn ctrl_d_puts_the_cursor_in_the_first_field() {
    let mut harness = opened();
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::D);
    harness.run();
    assert_eq!(
        harness.state().focused_field(),
        Some(0),
        "Ctrl+D reaches the pane from the table"
    );
}

#[test]
fn tab_walks_the_fields_and_wraps_at_the_end() {
    let mut harness = opened();
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::D);
    harness.run();
    let mut walked = Vec::new();
    for _ in 0..4 {
        walked.push(harness.state().focused_field());
        harness.key_press(egui::Key::Tab);
        harness.run();
    }
    // Four boxes and four Tabs, the fourth being the journal an article needs
    // though the file was made without one: the walk visits each box once and
    // comes back to the start rather than leaving the entry.
    assert_eq!(walked, [Some(0), Some(1), Some(2), Some(3)]);
    assert_eq!(harness.state().focused_field(), Some(0), "the walk wraps");
}

#[test]
fn shift_tab_walks_the_fields_backwards() {
    let mut harness = opened();
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::D);
    harness.run();
    harness.key_press_modifiers(egui::Modifiers::SHIFT, egui::Key::Tab);
    harness.run();
    assert_eq!(
        harness.state().focused_field(),
        Some(3),
        "backwards from the first field is the last one, the journal it lacks"
    );
}

#[test]
fn escape_leaves_the_field_box_without_undoing_the_typing() {
    let mut harness = opened();
    // Ctrl+D lands in the first field, which is the author's.
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::D);
    harness.run();
    harness.key_press(egui::Key::End);
    harness.run();
    for _ in 0..4 {
        harness.key_press(egui::Key::Backspace);
        harness.run();
    }
    harness
        .input_mut()
        .events
        .push(egui::Event::Text(String::from("Ann")));
    harness.run();
    harness.key_press(egui::Key::Escape);
    harness.run();
    assert!(
        !harness.state().field_focused(),
        "Escape hands the cursor back to the window"
    );
    assert_eq!(
        harness.state().status(),
        "Edited, not saved yet",
        "the typing was kept"
    );
    assert_eq!(
        harness.state().session().unwrap().value(0, 0).unwrap(),
        "Smith, Ann",
        "the box committed before it let go"
    );
}

#[test]
fn the_arrows_move_the_table_again_once_the_field_box_lets_go() {
    let mut harness = opened();
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::D);
    harness.run();
    harness.key_press(egui::Key::Escape);
    harness.run();
    harness.key_press(egui::Key::ArrowDown);
    harness.run();
    assert_eq!(
        harness.state().selected(),
        Some(1),
        "with no box holding the cursor the arrows are the table's again"
    );
}

#[test]
fn a_save_clears_the_unsaved_marker_from_the_toolbar() {
    let dir = std::env::temp_dir().join(format!("citerack-marker-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a directory");
    let path = dir.join("library.bib");
    std::fs::write(&path, INPUT).expect("the file");
    let session = Session::open(&path).expect("an open");

    let mut harness = harness(App::with_session(session));
    harness.state_mut().select(Some(0));
    harness.run();
    retype_year(&mut harness, "2025");
    assert_eq!(
        harness.query_all_by_label("Save *").count(),
        1,
        "an edit marks the save as owed"
    );
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::S);
    harness.run();
    assert!(
        !harness.state().session().unwrap().is_edited(),
        "the file holds the edit now"
    );
    assert_eq!(
        harness.query_all_by_label("Save *").count(),
        0,
        "the star goes once the work is written"
    );
    assert_eq!(
        harness.query_all_by_label("Save").count(),
        1,
        "the button stays"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// A library holding the same paper twice, so the duplicate scan has a pair.
const TWICE: &str = "\
@Article{smith2024,
  Author   = {Smith, John},
  Title    = {A Study of Braces},
  Year     = 2024,
  Doi      = {10.1000/braces},
}

@Article{smith2024copy,
  Author   = {Smith, J.},
  Title    = {A Study of Braces},
  Year     = 2024,
  Doi      = {https://doi.org/10.1000/BRACES},
}

@Book{jones1999,
  Author   = {Jones, Ann},
  Title    = {Another Book},
  Year     = 1999,
}
";

fn twice() -> Harness<'static, App> {
    let session = Session::from_text(
        PathBuf::from("twice.bib"),
        TWICE,
        &citerack_bib::ParseOptions { recover: true },
    )
    .expect("the input parses");
    harness(App::with_session(session))
}

#[test]
fn the_toolbar_says_how_many_pairs_there_are() {
    let harness = twice();
    harness.get_by_label("Duplicates 2");
}

#[test]
fn the_details_of_a_duplicate_name_the_other_entry_and_the_reason() {
    let mut harness = twice();
    harness.state_mut().select(Some(0));
    harness.run();
    harness.get_by_label("Also entered as smith2024copy: the same DOI");
    harness.get_by_label("Also entered as smith2024copy: the same title and year");
}

#[test]
fn the_duplicate_button_narrows_the_table_to_the_pairs() {
    let mut harness = twice();
    assert_eq!(harness.state().session().unwrap().order().len(), 3);
    harness.get_by_label("Duplicates 2").click();
    harness.run();
    assert_eq!(
        harness.state().session().unwrap().order().len(),
        2,
        "the third entry is not in a pair"
    );
    assert!(
        harness.query_all_by_label("jones1999").count() <= 1,
        "the un-paired entry is off the table, leaving only its detail heading at most"
    );
}

#[test]
fn the_duplicate_button_turns_the_filter_off_again() {
    let mut harness = twice();
    harness.get_by_label("Duplicates 2").click();
    harness.run();
    harness.get_by_label("Duplicates 2 (shown)").click();
    harness.run();
    assert_eq!(
        harness.state().session().unwrap().order().len(),
        3,
        "the whole library is back"
    );
}

#[test]
fn a_library_with_no_pairs_offers_no_button_to_press() {
    let mut harness = harness(App::with_session(session()));
    harness.get_by_label("Duplicates 0").click();
    harness.run();
    assert_eq!(
        harness.state().session().unwrap().order().len(),
        2,
        "pressing it changes nothing, since the filter would show the same rows"
    );
    assert!(
        !harness.state().session().unwrap().showing_only_duplicates(),
        "the table did not narrow to an empty set"
    );
}

/// The error a machine reports when its GL driver will not open a window, which
/// is the one the second renderer exists to answer.
fn a_refused_context() -> eframe::Error {
    use glutin::display::{Display, DisplayApiPreference, GlDisplay as _};
    use raw_window_handle::{RawDisplayHandle, XlibDisplayHandle};

    let template = glutin::config::ConfigTemplateBuilder::new().build();
    // SAFETY: no window is passed, and the handle asks for the default display.
    let handle = RawDisplayHandle::Xlib(XlibDisplayHandle::new(None, 0));
    let Ok(display) = (unsafe { Display::new(handle, DisplayApiPreference::Egl) }) else {
        return eframe::Error::NoGlutinConfigs(template, "no display".into());
    };
    match unsafe { display.find_configs(template.clone()) } {
        Err(err) => eframe::Error::Glutin(err),
        Ok(_) => eframe::Error::NoGlutinConfigs(template, "no config".into()),
    }
}

#[test]
fn a_refused_renderer_is_answered_by_the_other_one() {
    assert!(
        citerack::falls_back(&a_refused_context()),
        "a window the GL driver will not open is tried again with wgpu"
    );
}

#[test]
fn a_window_the_user_closed_is_not_reopened_elsewhere() {
    // Nothing failed: the event loop ended, and a second window opened on top
    // would be one the user had already dismissed.
    assert!(
        !citerack::falls_back(&eframe::Error::WinitEventLoop(
            winit::error::EventLoopError::ExitFailure(0)
        )),
        "a closed window is left closed"
    );
}

/// A library whose entries differ in author, venue and year, so every facet has
/// something to narrow by.
const SPREAD: &str = "\
@article{smith2024,
  Author  = {Smith, John},
  Title   = {A Study of Braces},
  Year    = 2024,
  Journal = {Journal of Braces},
}

@article{smith2023,
  Author  = {Smith, John},
  Title   = {Another Study},
  Year    = 2023,
  Journal = {Journal of Braces},
}

@book{jones1999,
  Author    = {Jones, Ann},
  Title     = {Another Book},
  Year      = 1999,
  Publisher = {Knot Press},
}
";

fn spread() -> Harness<'static, App> {
    let session = Session::from_text(
        PathBuf::from("spread.bib"),
        SPREAD,
        &citerack_bib::ParseOptions { recover: true },
    )
    .expect("the input parses");
    harness(App::with_session(session))
}

#[test]
fn a_facet_list_offers_a_value_with_the_rows_it_leaves() {
    let harness = spread();
    harness.get_by_label("Smith (2)");
    harness.get_by_label("Jones (1)");
}

#[test]
fn pressing_a_value_narrows_the_table_to_it() {
    let mut harness = spread();
    assert_eq!(harness.state().session().unwrap().order().len(), 3);

    harness.get_by_label("Jones (1)").click();
    harness.run();

    assert_eq!(
        harness.state().session().unwrap().order(),
        [2],
        "the table holds Jones's entry alone"
    );
    assert_eq!(
        harness.state().status(),
        "1 entries  |  narrowed by 1 facet list"
    );
}

#[test]
fn the_list_of_one_facet_counts_what_the_other_facets_allow() {
    let mut harness = spread();
    harness.get_by_label("Smith (2)").click();
    harness.run();

    harness.get_by_label("2024 (1)");
    harness.get_by_label("2023 (1)");
    assert!(
        harness.query_all_by_label("1999 (1)").count() == 0,
        "a year Smith has no entry in is not offered"
    );
}

#[test]
fn pressing_the_held_value_again_lets_the_list_go() {
    let mut harness = spread();
    harness.get_by_label("Jones (1)").click();
    harness.run();
    assert_eq!(harness.state().session().unwrap().order().len(), 1);

    // The held value is drawn apart from the rest, and pressing it is the way back.
    harness.get_by_label("Jones  (let go)").click();
    harness.run();
    assert_eq!(
        harness.state().session().unwrap().order().len(),
        3,
        "the whole library is back"
    );
    assert_eq!(harness.state().status(), "3 entries");
}

#[test]
fn the_clear_button_lets_every_list_go_at_once() {
    let mut harness = spread();
    harness.get_by_label("Smith (2)").click();
    harness.run();
    harness.get_by_label("2023 (1)").click();
    harness.run();
    assert_eq!(harness.state().session().unwrap().order().len(), 1);

    harness.get_by_label("Clear").click();
    harness.run();
    assert_eq!(harness.state().session().unwrap().order().len(), 3);
    assert_eq!(
        harness.state().session().unwrap().facet_count(),
        0,
        "no list is narrowing any more"
    );
}

#[test]
fn a_facet_and_a_search_narrow_the_same_table() {
    let mut harness = spread();
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::F);
    harness.run();
    harness
        .input_mut()
        .events
        .push(egui::Event::Text(String::from("another")));
    for _ in 0..8 {
        std::thread::sleep(std::time::Duration::from_millis(40));
        harness.step();
    }
    assert_eq!(
        harness.state().session().unwrap().order(),
        [1, 2],
        "both entries hold the word"
    );

    harness.get_by_label("Jones (1)").click();
    harness.run();
    assert_eq!(
        harness.state().session().unwrap().order(),
        [2],
        "the facet narrowed the rows the query found"
    );
}

#[test]
fn a_group_narrows_by_where_the_work_was_published() {
    let mut harness = spread();
    harness.get_by_label("Journal of Braces (2)").click();
    harness.run();
    assert_eq!(
        harness.state().session().unwrap().order(),
        [0, 1],
        "the two journal articles, and not the book"
    );
}

/// An article made without the journal and title its type asks for, and a type
/// that asks for nothing, so a missing field has somewhere to be missing.
const THIN: &str = "\
@Article{thin2024,
  Author   = {Smith, John},
  Year     = 2024,
}

@Article{blank2023,
  Author   = {Smith, John},
  Title    = {A Study of Braces},
  Year     = 2023,
  Journal  = {  },
}

@Misc{notes2019,
  Author   = {Smith, John},
  Title    = {Notes},
  Year     = 2019,
}
";

fn thin() -> Harness<'static, App> {
    let session = Session::from_text(
        PathBuf::from("thin.bib"),
        THIN,
        &citerack_bib::ParseOptions { recover: true },
    )
    .expect("the input parses");
    harness(App::with_session(session))
}

#[test]
fn an_article_offers_the_fields_its_type_needs() {
    let mut harness = thin();
    harness.state_mut().select(Some(0));
    harness.run();

    // The two fields the file holds, then a box each for the two it lacks.
    assert_eq!(
        boxes(&harness),
        ["Smith, John", "2024", "", ""],
        "a box for the title and the journal, empty and in that order"
    );
    harness.get_by_label("This article wants author, title, journal, year.");
    harness.get_by_label("title");
    harness.get_by_label("journal");
}

#[test]
fn a_missing_required_field_is_marked_rather_than_added() {
    let mut harness = thin();
    harness.state_mut().select(Some(0));
    harness.run();
    assert_eq!(
        harness.query_all_by_label("missing").count(),
        2,
        "the title and the journal are both marked"
    );

    let session = harness.state().session().unwrap();
    let written = citerack_bib::write_str(session.library(), &citerack_bib::WriteOptions::Preserve);
    assert_eq!(
        written, THIN,
        "nothing was written for the boxes that were never filled"
    );
}

#[test]
fn filling_a_missing_field_writes_it_into_the_entry() {
    let mut harness = thin();
    harness.state_mut().select(Some(0));
    harness.run();

    // The title's box is the first the file does not hold, the journal's the
    // second, and the filter box is empty too, hence the step past it. A short
    // word keeps the typing inside the box: a line too long for it repaints.
    harness
        .get_all_by(|node: &AccessKitNode| {
            node.role() == Role::TextInput && node.value().as_deref() == Some("")
        })
        .nth(1)
        .expect("the title's box")
        .click();
    harness.run();
    // The caret goes where a user would put it: the end of the box.
    harness.key_press(egui::Key::End);
    harness.run();
    harness
        .input_mut()
        .events
        .push(egui::Event::Text(String::from("Braces")));
    harness.run();
    harness.key_press(egui::Key::Escape);
    harness.run();

    let session = harness.state().session().unwrap();
    let entry = &session.library().entries()[0];
    assert_eq!(
        entry.field("title").map(|value| value.to_string()),
        Some(String::from("Braces")),
        "the typed title reached the entry"
    );
    assert_eq!(
        entry.field("journal").map(|value| value.to_string()),
        None,
        "the box left empty added no field"
    );
    assert_eq!(harness.state().status(), "Edited, not saved yet");
}

#[test]
fn a_required_field_present_but_blank_says_empty() {
    let mut harness = thin();
    harness.state_mut().select(Some(1));
    harness.run();

    assert_eq!(
        boxes(&harness),
        ["Smith, John", "A Study of Braces", "2023", "  "],
        "a blank field keeps its box, holding what the file holds"
    );
    assert_eq!(
        harness.query_all_by_label("empty").count(),
        1,
        "the journal is named as empty rather than missing"
    );
    assert_eq!(
        harness.query_all_by_label("missing").count(),
        0,
        "nothing is missing from this one"
    );
}

#[test]
fn a_misc_asks_for_no_field_it_does_not_have() {
    let mut harness = thin();
    harness.state_mut().select(Some(2));
    harness.run();

    assert_eq!(
        boxes(&harness),
        ["Smith, John", "Notes", "2019"],
        "no box is drawn for a field the type does not require"
    );
    assert_eq!(harness.query_all_by_label("missing").count(), 0);
    assert_eq!(
        harness
            .query_all_by_label("This misc wants author, title, year.")
            .count(),
        0,
        "a type with nothing required says nothing about what it wants"
    );
}

/// A library whose keys are its author's own work, so a generated key has
/// something to replace.
const NAMED: &str = "\
@Article{first,
  Author   = {Smith, John},
  Title    = {A Study of Braces},
  Year     = 2024,
}

@Article{second,
  Author   = {Smith, John},
  Title    = {Another Study},
  Year     = 2024,
}
";

fn named() -> Harness<'static, App> {
    let session = Session::from_text(
        PathBuf::from("named.bib"),
        NAMED,
        &citerack_bib::ParseOptions { recover: true },
    )
    .expect("the input parses");
    harness(App::with_session(session))
}

#[test]
fn the_table_shows_the_key_the_pattern_would_write() {
    let mut harness = named();
    harness.state_mut().select(Some(0));
    harness.run();

    // The second entry cannot have the key the first one's own fields make, so
    // the column letters it rather than showing one key twice.
    let session = harness.state().session().unwrap();
    assert_eq!(
        session.proposed_keys(),
        ["Smith2024", "Smith2024a"],
        "two entries never end up with one key"
    );
    // The table draws the entries in the order the library holds them, and the
    // lettered key is the second one's.
    assert_eq!(session.len(), 2);
    assert_eq!(session.order(), [0, 1], "the table keeps the file order");
    assert_eq!(
        harness.query_all_by_label("Smith2024").count(),
        1,
        "the first row shows the plain key"
    );
    assert_eq!(
        harness.query_all_by_label("Smith2024a").count(),
        1,
        "the second row shows the letter, since the plain key is taken"
    );
}

#[test]
fn a_bad_pattern_says_why_and_the_keys_go_on_as_they_were() {
    let mut harness = named();
    harness.run();
    // A brace opens a field and never closes it. The caret is put in the box
    // before the brace is typed, so the brace lands at the end of the pattern
    // the box already holds and the box says why it cannot be read.
    harness.state_mut().focus_pattern();
    harness.run();
    harness
        .input_mut()
        .events
        .push(egui::Event::Text(String::from("{")));
    harness.step();

    harness.get_by_label("that pattern is not one: a { is never closed");
    assert_eq!(
        harness.state().session().unwrap().key_pattern(),
        "{author}{year}",
        "a pattern that names no field does not become the setting"
    );
}

#[test]
fn the_generate_button_writes_the_key_the_column_shows() {
    let mut harness = named();
    harness.run();
    harness.get_by_label("Generate keys").click();
    harness.run();

    let session = harness.state().session().unwrap();
    assert_eq!(session.library().entries()[0].key.as_ref(), "Smith2024");
    assert_eq!(
        session.library().entries()[1].key.as_ref(),
        "Smith2024a",
        "two entries never end up with one key"
    );
    assert_eq!(harness.state().status(), "Renamed 2 keys, not saved yet");
    assert!(harness.state().session().unwrap().is_edited());
}

#[test]
fn a_second_press_of_generate_says_the_keys_are_done() {
    let mut harness = named();
    harness.run();
    harness.get_by_label("Generate keys").click();
    harness.run();
    harness.get_by_label("Generate keys").click();
    harness.run();

    assert_eq!(
        harness.state().status(),
        "Every key already follows the pattern, or was written by hand"
    );
    assert_eq!(
        harness.state().session().unwrap().library().entries()[0]
            .key
            .as_ref(),
        "Smith2024",
        "the second press leaves the key the first one wrote"
    );
}

#[test]
fn a_double_click_gives_one_entry_the_key_its_row_shows() {
    let mut harness = named();
    harness.state_mut().select(Some(0));
    harness.run();
    // The second row of the table, away from the columns whose own widgets take
    // the click.
    let at = egui::pos2(400.0, 120.0);
    harness
        .input_mut()
        .events
        .push(egui::Event::PointerMoved(at));
    harness.step();
    for _ in 0..2 {
        for pressed in [true, false] {
            harness.input_mut().events.push(egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::default(),
            });
        }
        harness.step();
    }
    harness.run();

    let session = harness.state().session().unwrap();
    assert_eq!(
        session.library().entries()[1].key.as_ref(),
        "Smith2024a",
        "the entry takes the key its own row shows"
    );
    assert_eq!(
        session.library().entries()[0].key.as_ref(),
        "first",
        "the entry the click did not land on is left as it was"
    );
    assert_eq!(harness.state().status(), "Renamed second to Smith2024a");
}

#[test]
fn the_pattern_box_shows_the_setting_the_library_uses() {
    let harness = named();
    harness.get_by(|node: &AccessKitNode| {
        node.role() == Role::TextInput && node.value().as_deref() == Some("{author}{year}")
    });
}

/// What a machine reports about its own theme, which is what `System` resolves to.
fn system_theme(harness: &mut Harness<'_, App>, theme: Option<egui::Theme>) {
    harness.input_mut().system_theme = theme;
    harness.run();
}

#[test]
fn the_window_starts_out_following_the_system() {
    let mut harness = named();
    system_theme(&mut harness, Some(egui::Theme::Dark));
    assert_eq!(
        harness.state().theme(),
        egui::ThemePreference::System,
        "the switch starts where a user expects it: on the system"
    );
    assert_eq!(harness.state().drawn_theme(&harness.ctx), egui::Theme::Dark);

    // A machine that switches to light at sunset is followed without a click.
    system_theme(&mut harness, Some(egui::Theme::Light));
    assert_eq!(
        harness.state().drawn_theme(&harness.ctx),
        egui::Theme::Light,
        "the system it follows is read again every frame"
    );
}

#[test]
fn a_machine_that_reports_no_theme_is_drawn_light() {
    let mut harness = named();
    system_theme(&mut harness, None);
    assert_eq!(
        harness.state().drawn_theme(&harness.ctx),
        egui::Theme::Light,
        "a window that guesses dark on a light desktop is the one a user blames"
    );
    assert_eq!(harness.state().theme(), egui::ThemePreference::System);
}

#[test]
fn pressing_light_leaves_the_system_behind() {
    let mut harness = named();
    system_theme(&mut harness, Some(egui::Theme::Dark));
    harness.get_by_label("Light").click();
    harness.run();

    assert_eq!(harness.state().theme(), egui::ThemePreference::Light);
    assert_eq!(
        harness.state().drawn_theme(&harness.ctx),
        egui::Theme::Light,
        "a chosen theme outranks the system, in both directions"
    );
    assert_eq!(
        harness.ctx.theme(),
        egui::Theme::Light,
        "the context is drawn in the theme the window chose"
    );
}

#[test]
fn pressing_dark_outranks_a_light_system() {
    let mut harness = named();
    system_theme(&mut harness, Some(egui::Theme::Light));
    harness.get_by_label("Dark").click();
    harness.run();

    assert_eq!(harness.state().drawn_theme(&harness.ctx), egui::Theme::Dark);
    assert_eq!(harness.ctx.theme(), egui::Theme::Dark);
}

#[test]
fn the_system_word_says_which_theme_it_is_showing() {
    let mut harness = named();
    system_theme(&mut harness, Some(egui::Theme::Dark));
    harness.get_by_label("System (dark)");

    // Pressing it after a choice brings the system back rather than being a
    // fourth state nobody can leave.
    harness.get_by_label("Light").click();
    harness.run();
    harness.get_by_label("System (dark)").click();
    harness.run();
    assert_eq!(harness.state().theme(), egui::ThemePreference::System);
    assert_eq!(harness.state().drawn_theme(&harness.ctx), egui::Theme::Dark);
}

/// A library whose entry names a paper beside it, made to exist or not. The path
/// is relative, so the folder is one a test can create and clear up.
fn with_attachment(present: bool) -> (Harness<'static, App>, PathBuf) {
    let dir = std::env::temp_dir().join(format!(
        "citerack-window-{}",
        if present { "present" } else { "missing" }
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("papers")).expect("a folder");
    if present {
        std::fs::write(dir.join("papers/smith2024.pdf"), b"%PDF").expect("a file");
    }
    let text = "\
@Article{smith2024,
  Author = {Smith, John},
  Title  = {A Study of Braces},
  Year   = 2024,
  File   = {smith2024.pdf:papers/smith2024.pdf:PDF},
}
";
    let session = Session::from_text(
        dir.join("library.bib"),
        text,
        &citerack_bib::ParseOptions { recover: true },
    )
    .expect("the input parses");
    let mut harness = harness(App::with_session(session));
    harness.state_mut().select(Some(0));
    harness.run();
    (harness, dir)
}

#[test]
fn an_entry_with_a_paper_lists_it_below_the_field() {
    let (harness, dir) = with_attachment(true);
    harness.get_by_label("smith2024.pdf");
    harness.get_by_label("PDF");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_file_that_is_not_there_is_said_beside_the_field() {
    let (harness, dir) = with_attachment(false);
    // The word says the file is not there; the path says where it was looked for,
    // which is what a reader needs to put it right. Written out in full rather
    // than guessed at from the field.
    assert!(
        harness.query_all_by_label("missing").count() > 0,
        "the row says the file is missing"
    );
    let shown = dir.join("papers/smith2024.pdf");
    harness.get_by_label(&shown.display().to_string());
    // The row says it; no dialog interrupts reading the entry to say it.
    assert_eq!(harness.state().status(), "1 entries");
    let _ = std::fs::remove_dir_all(dir);
}

/// Three entries to pick from, in key order so a range has a known middle.
const THREE: &str = "\
@Article{alpha2020,
  Author = {Alpha, Ann},
  Title  = {First},
  Year   = 2020,
}

@Article{beta2021,
  Author = {Beta, Bob},
  Title  = {Second},
  Year   = 2021,
}

@Article{gamma2022,
  Author = {Gamma, Gil},
  Title  = {Third},
  Year   = 2022,
}
";

fn three() -> Harness<'static, App> {
    let session = Session::from_text(
        PathBuf::from("three.bib"),
        THREE,
        &citerack_bib::ParseOptions { recover: true },
    )
    .expect("the input parses");
    let mut harness = harness(App::with_session(session));
    harness.state_mut().select(Some(0));
    harness.run();
    harness
}

/// Holds `Shift` for one arrow press, which walks the range out by a row.
fn shift_arrow(harness: &mut Harness<'_, App>, key: egui::Key) {
    harness.key_press_modifiers(egui::Modifiers::SHIFT, key);
    harness.run();
}

#[test]
fn shift_and_down_picks_the_rows_between() {
    let mut harness = three();
    shift_arrow(&mut harness, egui::Key::ArrowDown);
    assert_eq!(harness.state().picked(), vec![0, 1]);
    shift_arrow(&mut harness, egui::Key::ArrowDown);
    assert_eq!(
        harness.state().picked(),
        vec![0, 1, 2],
        "the anchor stays put"
    );
    // The pane has made way for the batch box, which says how many are picked.
    harness.get_by_label("3 entries picked");
    assert!(
        harness.query_all_by_label("A Study").count() == 0,
        "the fields of one entry are not what a batch is for"
    );
}

#[test]
fn picking_many_writes_one_field_across_them_and_one_undo_takes_it_back() {
    let mut harness = three();
    harness.state_mut().select(Some(0));
    harness.run();
    shift_arrow(&mut harness, egui::Key::ArrowDown);
    shift_arrow(&mut harness, egui::Key::ArrowDown);

    // The boxes are filled through the window rather than by typing at a hint
    // text, which is what the boxes show before anything is in them. The press is
    // the button's own, as a click on it would be.
    harness.state_mut().type_batch("keywords", "reviewed");
    harness.run();
    harness.get_by_label("Write to all").click();
    harness.run();

    assert_eq!(harness.state().status(), "Wrote keywords to 3 entries");
    let session = harness.state().session().unwrap();
    assert!(session.is_edited());
    for row in 0..3 {
        assert!(
            session.value(row, 3).is_some(),
            "every picked entry gained the field"
        );
    }

    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
    harness.run();
    let session = harness.state().session().unwrap();
    assert!(!session.is_edited(), "one undo took all three");
}

#[test]
fn ctrl_a_picks_every_row_on_screen_and_a_click_lets_it_go() {
    let mut harness = three();
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    harness.run();
    assert_eq!(harness.state().picked(), vec![0, 1, 2]);

    // A plain click on a row is a fresh pick, not a wider one. `click_row` is what
    // the table calls; `select` moves the anchor and leaves the range it had.
    harness.state_mut().clear_pick();
    harness.state_mut().select(Some(1));
    harness.run();
    assert_eq!(harness.state().picked(), vec![1]);
}

#[test]
fn entering_a_batch_that_nobody_takes_says_why_and_keeps_the_typing() {
    let mut harness = three();
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    harness.run();
    // A name a field cannot hold is refused before a byte is written.
    harness.state_mut().type_batch("bad name", "x");
    harness.run();
    harness.get_by_label("Write to all").click();
    harness.run();

    assert_eq!(
        harness.state().batch(),
        ("bad name", "x"),
        "the typing stays"
    );
    assert!(
        harness.state().status().contains("no entry took it"),
        "a refusal is reported rather than swallowed: {}",
        harness.state().status()
    );
    // The sentence is on screen as well as in the style sheet, so it is found twice.
    assert!(
        harness
            .query_all_by_label("no entry took it: bad name is not a name a field can hold")
            .count()
            > 0,
        "the refusal is shown, not only kept in the status line"
    );
    assert!(!harness.state().session().unwrap().is_edited());
}

/// A record as a service would hand it back, so the filing half of an import is
/// tested without a request going out.
fn record() -> citerack_bib::Entry {
    use citerack_bib::{Delimiter, Entry, EntryKind, Field, FieldName};
    let field = |name: &str, value: &str| Field {
        name_raw: name.into(),
        name: FieldName::new(name),
        value: value.into(),
        delimiter: Delimiter::Braces,
    };
    Entry {
        kind: EntryKind::new("Article"),
        key: "".into(),
        fields: vec![
            field("author", "Doe, Jane"),
            field("title", "Imported Study"),
            field("year", "2021"),
            field("doi", "10.1000/xyz"),
        ],
    }
}

#[test]
fn the_import_button_opens_a_box_that_asks_for_an_identifier() {
    let mut harness = harness(App::with_session(session()));
    harness.get_by_label("Import").click();
    harness.run();

    assert!(harness.state().importing());
    harness.get_by_label("Paste a DOI, an arXiv id, or the link either one is shown at.");
    harness.get_by_label("Look up");
    harness.run();
}

#[test]
fn a_record_the_service_sent_can_be_filed_and_the_table_grows_by_one() {
    let mut harness = harness(App::with_session(session()));
    harness.state_mut().offer(record());
    harness.run();

    assert!(harness.state().status().is_empty() || !harness.state().status().starts_with("Import"));
    harness.get_by_label("Add to the library").click();
    harness.run();

    let app = harness.state();
    assert!(!app.importing(), "the box closes once the entry is in");
    assert_eq!(app.session().unwrap().len(), 3);
    assert_eq!(app.status(), "Imported doe2021");
    assert_eq!(
        app.session().unwrap().library().entries()[2].key.as_ref(),
        "doe2021"
    );
}

#[test]
fn a_filed_entry_can_be_taken_straight_back_out() {
    let mut harness = harness(App::with_session(session()));
    harness.state_mut().offer(record());
    harness.run();
    harness.get_by_label("Add to the library").click();
    harness.run();
    assert_eq!(harness.state().session().unwrap().len(), 3);

    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
    harness.run();
    assert_eq!(harness.state().session().unwrap().len(), 2);
    assert!(harness.state().status().contains("an import"));
}

#[test]
fn an_identifier_that_names_nothing_says_so_without_asking_anyone() {
    let mut harness = harness(App::with_session(session()));
    harness.state_mut().type_identifier("nature");
    harness.run();
    harness.get_by_label("Look up").click();
    harness.run();

    match harness.state().lookup_state() {
        citerack::LookupState::Failed(why) => {
            assert_eq!(why, "nature is neither a DOI nor an arXiv id")
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
    assert!(harness.state().session().is_some_and(|s| !s.is_edited()));
}

#[test]
fn a_key_the_library_already_holds_is_moved_out_of_the_way() {
    let mut harness = harness(App::with_session(session()));
    harness.state_mut().offer(record());
    harness.run();
    harness.get_by_label("Add to the library").click();
    harness.run();

    let mut second = record();
    second.fields[1].value = "A Second Study".into();
    harness.state_mut().offer(second);
    harness.run();
    harness.get_by_label("Add to the library").click();
    harness.run();
    assert_eq!(harness.state().status(), "Imported doe2021a");
}
