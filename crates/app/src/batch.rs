//! The box that writes one field across several entries, in the pane's place
//! while more than a row is picked. The field name is typed rather than chosen:
//! a list of the names a library can hold would be a schema it does not have.

use crate::text;
use citerack_store::Session;
use egui::{Color32, RichText, TextEdit, Ui};

/// The colour a batch that did not go through is reported in, the same red the
/// detail pane refuses a value in.
const REFUSAL: Color32 = Color32::LIGHT_RED;

/// What the batch box holds between frames: the field name and the value, kept so
/// a refusal leaves the typing where it was rather than emptying the boxes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Batch {
    /// The field to write, as the user typed it.
    pub name: String,
    /// The value to write.
    pub value: String,
}

/// What a frame of the batch box produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Nothing was written.
    Idle,
    /// A field was written across some entries.
    Wrote {
        /// How many entries took the value.
        written: usize,
        /// How many refused it and were left as they were.
        refused: usize,
    },
}

/// Draws the box for `rows` picked entries. `refusal` is what the last press could
/// not do, if it could not: it outlives the frame so the reason stays on screen
/// beside the boxes that caused it.
pub fn show(
    ui: &mut Ui,
    session: &mut Option<Session>,
    rows: &[usize],
    batch: &mut Batch,
    refusal: &mut Option<String>,
) -> Outcome {
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(text!(EntriesPicked, count: rows.len())).strong());
        ui.weak(text!(PickedHint));
    });
    let Some(session) = session.as_mut() else {
        return Outcome::Idle;
    };
    // What the picked entries already say in this field, said once: a batch that
    // would overwrite four different values is worth knowing about before it does.
    let held = if batch.name.trim().is_empty() {
        String::new()
    } else {
        values_of(session, rows, batch.name.trim())
    };
    ui.separator();
    let mut pressed = false;
    ui.horizontal(|ui| {
        ui.label(RichText::new(text!(Field)).weak());
        ui.add(
            TextEdit::singleline(&mut batch.name)
                .hint_text(text!(FieldHint))
                .desired_width(140.0)
                .id_salt("batch_name"),
        );
    });
    ui.horizontal(|ui| {
        ui.label(RichText::new(text!(Value)).weak());
        let response = ui.add(
            TextEdit::singleline(&mut batch.value)
                .hint_text(text!(ValueHint))
                .desired_width(ui.available_width() - 90.0)
                .id_salt("batch_value"),
        );
        // Enter writes the batch from the value box, which is where the cursor is
        // by the time the value has been typed.
        if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            pressed = true;
        }
        pressed |= ui
            .add_enabled(
                !batch.name.trim().is_empty(),
                egui::Button::new(text!(WriteToAll)),
            )
            .clicked();
    });
    if !held.is_empty() {
        ui.weak(held);
    }
    if let Some(why) = refusal.as_deref() {
        ui.colored_label(REFUSAL, why);
    }
    if !pressed {
        return Outcome::Idle;
    }
    if batch.name.trim().is_empty() {
        return Outcome::Idle;
    }
    let (written, refused) = session.set_field_in(rows, batch.name.trim(), &batch.value);
    if written == 0 {
        // Said here as well as in the status line: the reason stays beside the
        // boxes that caused it, where the next thing typed will be.
        *refusal = Some(if refused > 0 {
            text!(NoEntryTookName, name: batch.name.trim())
        } else {
            text!(NoEntryTookValue, name: batch.name.trim())
        });
        return Outcome::Idle;
    }
    *refusal = None;
    Outcome::Wrote { written, refused }
}

/// A sentence saying what the picked entries already hold in a field, or nothing
/// when they hold one value and the batch would not change it distinguishably.
fn values_of(session: &Session, rows: &[usize], name: &str) -> String {
    let mut seen: Vec<&str> = Vec::new();
    let mut absent = 0;
    for &row in rows {
        let entry = session.order().get(row).copied();
        match entry.and_then(|entry| session.library().entries().get(entry)) {
            Some(own) => match own.field(name) {
                Some(value) => {
                    if !seen.contains(&value) {
                        seen.push(value);
                    }
                }
                None => absent += 1,
            },
            None => absent += 1,
        }
    }
    let values = if seen.len() == 1 && seen[0].is_empty() {
        text!(OneValue)
    } else {
        text!(Values, count: seen.len())
    };
    match (seen.len(), absent) {
        (0, _) => text!(NoneHasIt, name: name),
        (1, 0) => text!(AllSay, value: seen[0]),
        (_, 0) => text!(TheySay, values: values, list: list(&seen)),
        (_, missing) => text!(
            TheySaySomeMissing,
            values: values,
            missing: missing,
            name: name,
            list: list(&seen)
        ),
    }
}

/// Up to three values, quoted, with the rest counted rather than listed.
fn list(values: &[&str]) -> String {
    let shown: Vec<String> = values
        .iter()
        .take(3)
        .map(|value| format!("\"{value}\""))
        .collect();
    if values.len() > 3 {
        text!(
            ListMore,
            shown: shown.join(", "),
            count: values.len() - 3
        )
    } else {
        shown.join(", ")
    }
}
