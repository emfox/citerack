//! The box that fetches one entry from a DOI or an arXiv id. It takes the pane's
//! place while it is open: one identifier is typed, one button asks, and the answer
//! is either an entry filed into the library or a sentence saying what went wrong.

use citerack_index::read_identifier;
use citerack_store::Session;
use egui::{Color32, RichText, TextEdit, Ui};

use crate::lookup::{Lookup, State, key_for};

/// The colour a lookup that did not go through is reported in, the same red the
/// rest of the window uses for a refusal.
const REFUSED: Color32 = Color32::LIGHT_RED;

/// What the box did this frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Nothing happened.
    Idle,
    /// The request went out, for this identifier.
    Asking(String),
    /// An entry was filed, at this row and under this key.
    Filed { row: usize, key: String },
    /// The box was closed without filing anything.
    Closed,
}

/// Draws the import box. `typed` is the identifier under the cursor, `state` is what
/// the last request did, and `lookup` is the request in flight.
pub fn show(
    ui: &mut Ui,
    session: &mut Option<Session>,
    typed: &mut String,
    state: &mut State,
    lookup: &mut Lookup,
) -> Outcome {
    let mut outcome = Outcome::Idle;
    ui.horizontal(|ui| {
        ui.heading("Import");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Close").clicked() {
                lookup.cancel();
                *state = State::Idle;
                outcome = Outcome::Closed;
            }
        });
    });
    ui.label("Paste a DOI, an arXiv id, or the link either one is shown at.");
    ui.add_space(4.0);

    let field = ui.add(
        TextEdit::singleline(typed)
            .id_salt("import-id")
            .hint_text("10.1038/nphys1170 or 2101.01234"),
    );
    let asked = ui
        .add_enabled(!typed.trim().is_empty(), egui::Button::new("Look up"))
        .clicked()
        || field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
    if asked && !lookup.is_asking() {
        match read_identifier(typed) {
            Ok(found) => {
                lookup.ask(&found);
                *state = State::Asking(found.id.clone());
                outcome = Outcome::Asking(found.id);
            }
            Err(why) => *state = State::Failed(why),
        }
    }

    match state.clone() {
        State::Asking(id) => {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(format!("Asking for {id}"));
            });
        }
        State::Failed(why) => {
            ui.label(RichText::new(why).color(REFUSED));
        }
        State::Read(record) => {
            let wanted = key_for(&record);
            let key = session
                .as_ref()
                .map_or_else(|| wanted.clone(), |session| session.key_for(&wanted));
            ui.add_space(4.0);
            ui.label(RichText::new(format!("{} · {key}", record.kind)).strong());
            if let Some(title) = record.field("title") {
                ui.label(truncate(title, 90));
            }
            if let Some(authors) = record.field("author") {
                ui.label(RichText::new(truncate(authors, 90)).weak());
            }
            ui.add_space(4.0);
            if ui
                .add_enabled(session.is_some(), egui::Button::new("Add to the library"))
                .clicked()
                && let Some(entry) = session.as_mut().map(|session| (session, record))
            {
                let (session, record) = entry;
                match import(session, *record, &key) {
                    Ok(row) => {
                        *state = State::Idle;
                        outcome = Outcome::Filed { row, key };
                    }
                    Err(why) => *state = State::Failed(why),
                }
            }
        }
        State::Idle => {}
    }
    ui.add_space(6.0);
    ui.label(
        RichText::new(
            "Crossref answers for a DOI and arXiv for an id. A lookup that fails \
             leaves the library untouched.",
        )
        .weak(),
    );
    outcome
}

/// Files a record under `key`, taking the key actually given back since the library
/// may have had to move it out of the way of its own.
fn import(session: &mut Session, entry: citerack_bib::Entry, key: &str) -> Result<usize, String> {
    let mut entry = entry;
    entry.key = key.into();
    session
        .import(entry)
        .map_err(|why| format!("the record could not be filed: {why}"))
}

fn truncate(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        return text.to_owned();
    }
    let head: String = text.chars().take(width).collect();
    format!("{head}...")
}
