//! The detail pane: every field of the selected entry, in file order, each one
//! editable in place. A value is committed when its box loses focus, so typing
//! does not rewrite the entry on every keystroke.

use citerack_store::Session;
use egui::{Grid, RichText, TextEdit, Ui};

/// One field box being typed in: the row, the field, and the text in the box.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Draft {
    /// The row on display the box belongs to.
    pub row: usize,
    /// The field's position in the entry.
    pub field: usize,
    /// What the box holds, which outlives a frame so a refusal keeps the text.
    pub text: String,
}

/// What a frame of the pane produced, which the window acts on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Nothing in the pane changed the library.
    Idle,
    /// An entry was edited.
    Edited,
}

/// Draws the fields of one entry, or a hint when nothing is selected.
pub fn show(
    ui: &mut Ui,
    session: Option<&mut Session>,
    row: Option<usize>,
    draft: &mut Option<Draft>,
) -> Outcome {
    let (Some(session), Some(row)) = (session, row) else {
        ui.weak("Select an entry to see its fields.");
        return Outcome::Idle;
    };
    let Some(entry) = session.library().entries().get(row) else {
        ui.weak("Select an entry to see its fields.");
        return Outcome::Idle;
    };
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(entry.kind.as_str()).strong());
        ui.label(RichText::new(entry.key.as_ref()).monospace());
    });
    ui.separator();

    let names: Vec<String> = entry
        .fields
        .iter()
        .map(|f| f.name_raw.to_string())
        .collect();
    // Read every value before drawing, since committing one borrows the session
    // mutably and the rows still to be drawn would hold a shared borrow.
    let current: Vec<String> = (0..names.len())
        .map(|index| session.value(row, index).unwrap_or_default().to_owned())
        .collect();
    let mut outcome = Outcome::Idle;
    let mut refusal: Option<&'static str> = None;
    let mut commit: Option<(usize, String)> = None;

    egui::ScrollArea::vertical()
        .id_salt("detail_fields")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            Grid::new("detail_grid")
                .num_columns(2)
                .striped(true)
                .show(ui, |ui| {
                    for (index, name) in names.iter().enumerate() {
                        ui.label(RichText::new(name.as_str()).weak());
                        // The box owns a buffer only while it is being typed in;
                        // between frames the value comes from the library.
                        let typing = matches!(draft, Some(d) if d.row == row && d.field == index);
                        let mut text = match &*draft {
                            Some(d) if typing => d.text.clone(),
                            _ => current[index].clone(),
                        };
                        let width = (ui.available_width() - 8.0).max(80.0);
                        let response = ui.add(
                            TextEdit::multiline(&mut text)
                                .desired_width(width)
                                .desired_rows(1)
                                .id_salt(("field", row, index)),
                        );
                        if response.has_focus() {
                            *draft = Some(Draft {
                                row,
                                field: index,
                                text: text.clone(),
                            });
                        } else if typing && text != current[index] {
                            commit = Some((index, text));
                        } else if typing {
                            *draft = None;
                        }
                        ui.end_row();
                    }
                });
        });

    if let Some((field, text)) = commit {
        match session.set_field(row, field, &text) {
            Ok(()) => {
                outcome = Outcome::Edited;
                *draft = None;
            }
            // The text stays in the box so the typing is not lost.
            Err(err) => refusal = Some(err.as_str()),
        }
    }

    if let Some(refusal) = refusal {
        ui.colored_label(egui::Color32::LIGHT_RED, refusal);
    }
    outcome
}
