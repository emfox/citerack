//! The detail pane: every field of the selected entry, in file order.

use citerack_bib::Entry;
use egui::{Grid, RichText, Ui};

/// Draws the fields of one entry, or a hint when nothing is selected.
pub fn show(ui: &mut Ui, entry: Option<&Entry>) {
    let Some(entry) = entry else {
        ui.weak("Select an entry to see its fields.");
        return;
    };
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(entry.kind.as_str()).strong());
        ui.label(RichText::new(entry.key.as_ref()).monospace());
    });
    ui.separator();
    egui::ScrollArea::vertical()
        .id_salt("detail_fields")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            Grid::new("detail_grid")
                .num_columns(2)
                .striped(true)
                .show(ui, |ui| {
                    for field in &entry.fields {
                        ui.label(RichText::new(field.name_raw.as_ref()).weak());
                        ui.label(&field.value);
                        ui.end_row();
                    }
                });
        });
}
