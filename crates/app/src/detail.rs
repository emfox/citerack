//! The detail pane: every field of the selected entry, in file order, each one
//! editable in place, and below them a box for every required field the entry is
//! missing. A value is committed when its box loses focus.

use crate::attach::{self, Found};
use citerack_bib::{Attachment, Entry, EntryKind, is_required, parse_file_field, wanted};
use citerack_store::Session;
use egui::{Color32, RichText, TextEdit, Ui};

/// The colour a field the type cannot do without is named in. Red here means
/// "this needs you", which is a different thing from the red on a refusal.
const NEEDS: Color32 = Color32::from_rgb(224, 122, 95);

/// The narrowest a field box may be drawn, so a long field name still leaves
/// something to type in rather than a sliver of a box.
const MIN_BOX: f32 = 60.0;

/// The colour a file the field names but the disk does not hold is reported in.
/// Amber rather than red: nothing is broken in the entry, and the library still
/// saves exactly as it was.
const MISSING: Color32 = Color32::from_rgb(214, 168, 92);

/// One field box being typed in: the row, the field, and the text in the box.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Draft {
    /// The row on display the box belongs to.
    pub row: usize,
    /// The field's position in the entry, or for a field the file does not hold
    /// yet, the entry's field count plus its place among the missing ones.
    pub field: usize,
    /// What the box holds, which outlives a frame so a refusal keeps the text.
    pub text: String,
}

/// What a frame of the pane produced, which the window acts on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Nothing in the pane changed the library.
    Idle,
    /// An entry was edited.
    Edited,
    /// A file beside a field was asked for, and the window is to open it.
    Opened(std::path::PathBuf),
}

/// One row of the pane: a field the entry holds, or a required field it lacks.
struct Row {
    /// The name in the case the file used, or as the type table spells it.
    name: String,
    /// Position among the entry's fields, `None` for a field still to be added.
    field: Option<usize>,
    /// What the box starts as.
    value: String,
    /// Whether the type cannot do without this field.
    required: bool,
}

impl Row {
    /// What leaving this box writes: the field it named, or the field it adds.
    /// A gap left empty writes nothing, so the entry keeps the shape the file had.
    fn edit(&self, text: &str) -> Option<Edit> {
        match (self.field, text.trim().is_empty()) {
            (Some(field), _) => Some(Edit::Set(field, text.to_owned())),
            (None, true) => None,
            (None, false) => Some(Edit::Add(self.name.clone(), text.to_owned())),
        }
    }

    /// How much room the name asks for, counted in characters so the pane can
    /// size its column without measuring a font it has not been given.
    fn name_width(&self) -> f32 {
        self.name.chars().count() as f32 * 7.0
    }

    /// How much room the marker asks for, on the same count.
    fn mark_width(&self) -> f32 {
        self.marker()
            .map_or(0.0, |marker| marker.chars().count() as f32 * 6.0)
    }

    /// The word beside the box when the row says why it is there.
    fn marker(&self) -> Option<&'static str> {
        match (self.field, self.required, self.value.trim().is_empty()) {
            (None, _, _) => Some("missing"),
            (Some(_), true, true) => Some("empty"),
            _ => None,
        }
    }
}

/// A field replaced, or a missing one added under the name the type table gave it.
enum Edit {
    Set(usize, String),
    Add(String, String),
}

/// Draws the fields of one entry, or a hint when nothing is selected. `focus` is
/// the field a key asked for, honoured on this frame only; `focused` reports which
/// box holds the cursor, so a key can walk the entry without knowing widget ids.
pub fn show(
    ui: &mut Ui,
    session: Option<&mut Session>,
    row: Option<usize>,
    draft: &mut Option<Draft>,
    focus: &mut Option<usize>,
    focused: &mut Option<usize>,
) -> Outcome {
    let asked_for = focus.take();
    *focused = None;
    let (Some(session), Some(row)) = (session, row) else {
        ui.weak("Select an entry to see its fields.");
        return Outcome::Idle;
    };
    let library = session.path().to_path_buf();
    let Some(entry) = session.library().entries().get(row) else {
        ui.weak("Select an entry to see its fields.");
        return Outcome::Idle;
    };
    // Everything the pane needs is read out of the library before the duplicate
    // scan borrows the session mutably; the entry itself cannot outlive that.
    let heading = (entry.kind.as_str().to_owned(), entry.key.to_string());
    let rows = rows_of(entry);
    let asked: Vec<&str> = wanted(&entry.kind)
        .iter()
        .filter(|field| field.required)
        .map(|field| field.name)
        .collect();
    let short = rows.iter().filter(|r| r.field.is_none()).count();
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(&heading.0).strong());
        ui.label(RichText::new(&heading.1).monospace());
    });
    // A duplicate is named here rather than in a list elsewhere: the entry is
    // already on screen, so the reason and the other half of the pair belong with
    // it, where a user reading the fields can act on them.
    for (key, reason) in session.duplicates_of(row) {
        ui.colored_label(
            egui::Color32::from_rgb(214, 168, 92),
            format!("Also entered as {key}: {reason}"),
        );
    }
    // What the type asks for, said once above the fields rather than repeated on
    // every row, so a red row can answer "which of them does this one lack".
    if short > 0 && !asked.is_empty() {
        let list = asked.join(", ");
        ui.weak(format!(
            "This {} wants {list}.",
            heading.0.to_ascii_lowercase()
        ));
    }
    ui.separator();
    let mut outcome = Outcome::Idle;
    let mut refusal: Option<&'static str> = None;
    // The row a box held when it lost the cursor, and the text it held then.
    let mut commit: Option<(usize, String)> = None;

    egui::ScrollArea::vertical()
        .id_salt("detail_fields")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            // Three columns placed by hand, since a grid sizes a column by its
            // widest row and can leave the box a width the pane does not have.
            let pane = ui.available_width();
            let names = rows.iter().map(Row::name_width).fold(0.0, f32::max);
            let marks = rows.iter().map(Row::mark_width).fold(0.0, f32::max);
            let gap = ui.spacing().item_spacing.x;
            // A column takes its width and no more: a label left to fill the row
            // would take half the pane and leave the box the other half of a half.
            let column = egui::Layout::left_to_right(egui::Align::Center);
            let marks = if marks > 0.0 { marks + gap } else { 0.0 };
            // Wide enough to take the widest line egui left an item short of its
            // request, which is what a name column of a few characters does.
            let boxed = (pane - names - marks - 3.0 * gap).floor().max(MIN_BOX);
            for (slot, each) in rows.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.allocate_ui_with_layout(egui::vec2(names, 0.0), column, |ui| {
                        ui.set_max_width(names);
                        ui.add(egui::Label::new(if each.required {
                            RichText::new(each.name.as_str()).color(NEEDS)
                        } else {
                            RichText::new(each.name.as_str()).weak()
                        }));
                    });
                    // The box owns a buffer only while it is being typed in;
                    // between frames the value comes from the library.
                    let typing = matches!(draft, Some(d) if d.row == row && d.field == slot);
                    let mut text = match &*draft {
                        Some(d) if typing => d.text.clone(),
                        _ => each.value.clone(),
                    };
                    // One line per field, scrolled sideways when a value is
                    // longer than the pane: a wrapped title would push every
                    // field after it off the bottom of a narrow window.
                    let response = ui.add(
                        TextEdit::singleline(&mut text)
                            .desired_width(boxed)
                            .id_salt(("field", row, slot)),
                    );
                    // Tab and Escape belong to the pane while a box holds the
                    // cursor: egui would otherwise take both and move the cursor
                    // mid-frame, before the key could be acted on.
                    if response.has_focus() {
                        ui.memory_mut(|memory| {
                            memory.set_focus_lock_filter(
                                response.id,
                                egui::EventFilter {
                                    tab: true,
                                    escape: true,
                                    ..Default::default()
                                },
                            );
                        });
                    }
                    if asked_for == Some(slot) {
                        response.request_focus();
                    }
                    if response.has_focus() {
                        *focused = Some(slot);
                        *draft = Some(Draft {
                            row,
                            field: slot,
                            text: text.clone(),
                        });
                    } else if typing && text != each.value {
                        // A gap left empty writes nothing: the entry keeps the
                        // shape the file had until someone fills the box.
                        if !(each.field.is_none() && text.trim().is_empty()) {
                            commit = Some((slot, text.clone()));
                        } else {
                            *draft = None;
                        }
                    } else if typing {
                        *draft = None;
                    }
                    if marks > 0.0 {
                        ui.allocate_ui_with_layout(egui::vec2(marks, 0.0), column, |ui| {
                            ui.set_max_width(marks);
                            ui.weak(each.marker().unwrap_or_default());
                        });
                    }
                });
                // Below the box, at the depth of the value, the files it names.
                // The paths are checked once per frame that draws them: a file
                // opened or deleted outside the window is noticed within one.
                let files = files_of(each);
                if !files.is_empty() {
                    let checked: Vec<(Attachment, Found)> = attach::resolve(&library, &files)
                        .into_iter()
                        .zip(files)
                        .map(|(found, attachment)| (attachment, found))
                        .collect();
                    draw_files(ui, &checked, names + gap, &mut outcome);
                }
            }
        });

    if let Some((slot, text)) = commit {
        let written = match rows[slot].edit(&text) {
            Some(Edit::Set(field, text)) => session.set_field(row, field, &text),
            Some(Edit::Add(name, text)) => session.add_field(row, &name, &text),
            None => Ok(()),
        };
        match written {
            Ok(()) => {
                outcome = Outcome::Edited;
                *draft = None;
            }
            // The text stays in the box so the typing is not lost.
            Err(err) => refusal = Some(err.as_str()),
        }
    }

    if let Some(refusal) = refusal {
        ui.colored_label(Color32::LIGHT_RED, refusal);
    }
    outcome
}

/// How many boxes the pane draws for an entry: its fields, then the required ones
/// it lacks. The files listed under a box are read, not typed in, so not counted.
pub fn rows_count(entry: &Entry) -> usize {
    rows_of(entry).len()
}

/// The files a row names, in the order the field lists them. Empty for a field
/// that is not a `file` field, and for one that names nothing it can open.
fn files_of(row: &Row) -> Vec<Attachment> {
    if row.name.eq_ignore_ascii_case("file") {
        parse_file_field(&row.value)
    } else {
        Vec::new()
    }
}

/// Draws the files a `file` field names, each a press away from opening. A missing
/// one is said here, beside the field, rather than as a dialog over the window.
fn draw_files(ui: &mut Ui, files: &[(Attachment, Found)], indent: f32, outcome: &mut Outcome) {
    for (attachment, found) in files {
        ui.horizontal(|ui| {
            ui.add_space(indent);
            match found {
                Found::File(path) => {
                    let label = if attachment.name.is_empty() {
                        path.file_name()
                            .map(|name| name.to_string_lossy().into_owned())
                            .unwrap_or_else(|| path.display().to_string())
                    } else {
                        attachment.name.clone()
                    };
                    if ui
                        .link(label)
                        .on_hover_text(path.display().to_string())
                        .clicked()
                    {
                        *outcome = Outcome::Opened(path.clone());
                    }
                    if !attachment.kind.is_empty() {
                        ui.weak(&attachment.kind);
                    }
                }
                Found::Missing(path) => {
                    ui.colored_label(MISSING, "missing");
                    ui.weak(path.display().to_string());
                }
            }
        });
    }
}

/// The rows to draw: the fields the file holds in its own order, then a box for
/// every required field the entry lacks.
fn rows_of(entry: &Entry) -> Vec<Row> {
    let kind: &EntryKind = &entry.kind;
    let mut rows: Vec<Row> = entry
        .fields
        .iter()
        .enumerate()
        .map(|(index, field)| Row {
            name: field.name_raw.to_string(),
            field: Some(index),
            value: field.value.to_string(),
            required: is_required(kind, field.name.as_str()),
        })
        .collect();
    // A field the file holds stays as it is, box and all, even where its value
    // reads as nothing: the empty box is the one a reader can type into without
    // risking a second copy of a field the entry already has.
    for wanted in wanted(kind) {
        let held = std::iter::once(wanted.name)
            .chain(wanted.either.iter().copied())
            .any(|name| entry.field(name).is_some());
        if wanted.required && !held {
            rows.push(Row {
                name: wanted.name.to_string(),
                field: None,
                value: String::new(),
                required: true,
            });
        }
    }
    rows
}
