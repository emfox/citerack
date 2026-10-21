//! The window: a toolbar, a filter box, a three-part body and a status line.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use citerack_index::{SortField, SortSpec};
use citerack_store::Session;
use egui::{Align, Label, Layout, RichText};
use egui_extras::{Column as TableColumn, TableBuilder};

use crate::columns::{Column, entry_at};
use crate::detail::{self, Draft, Outcome};

/// How long a keystroke waits before the search runs, so a typed word costs one
/// query rather than one per character.
const SEARCH_DEBOUNCE: Duration = Duration::from_millis(120);

/// Row height in points; a two-line title would make every row as tall as it is.
const ROW_HEIGHT: f32 = 20.0;

/// Opens the window on a library, or on an empty session prompting for one.
pub fn run(path: Option<PathBuf>) -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1400.0, 880.0])
            .with_min_inner_size([800.0, 480.0])
            .with_title("citerack"),
        ..Default::default()
    };
    eframe::run_native(
        "citerack",
        options,
        Box::new(move |cc| Ok(Box::new(App::new(cc, path)))),
    )
}

/// A file being read on a worker thread, so the window can appear first.
struct Pending {
    path: PathBuf,
    started: Instant,
    rx: Receiver<Result<Session, citerack_store::StoreError>>,
}

/// What the window shows and what it remembers between frames.
struct App {
    session: Option<Session>,
    pending: Option<Pending>,
    error: Option<String>,
    query: String,
    applied_query: String,
    query_changed_at: Option<Instant>,
    selected: Option<usize>,
    sort: Option<SortSpec>,
    columns: Vec<Column>,
    status: String,
    startup: Option<Instant>,
    table_width: f32,
    /// The field box being typed in, kept between frames so a refusal keeps it.
    draft: Option<Draft>,
}

impl App {
    fn new(_cc: &eframe::CreationContext<'_>, path: Option<PathBuf>) -> Self {
        let mut app = Self {
            session: None,
            pending: None,
            error: None,
            query: String::new(),
            applied_query: String::new(),
            query_changed_at: None,
            selected: None,
            sort: None,
            columns: Column::ALL.to_vec(),
            status: String::new(),
            startup: Some(Instant::now()),
            table_width: 700.0,
            draft: None,
        };
        if let Some(path) = path {
            app.begin_open(path);
        }
        app
    }

    /// Starts reading a file on a worker thread.
    fn begin_open(&mut self, path: PathBuf) {
        let (tx, rx) = mpsc::channel();
        let worker_path = path.clone();
        std::thread::Builder::new()
            .name(String::from("open"))
            .spawn(move || {
                let _ = tx.send(Session::open(&worker_path));
            })
            .ok();
        self.pending = Some(Pending {
            path,
            started: Instant::now(),
            rx,
        });
        self.error = None;
    }

    /// Collects a finished open, and reports how long the whole chain took.
    fn poll_pending(&mut self, ctx: &egui::Context) {
        let Some(pending) = &self.pending else {
            return;
        };
        let Ok(result) = pending.rx.try_recv() else {
            return;
        };
        let elapsed = pending.started.elapsed();
        let path = pending.path.clone();
        self.pending = None;
        match result {
            Ok(session) => {
                self.status = format!(
                    "{} entries  |  opened in {:.2} s  |  {}",
                    session.len(),
                    elapsed.as_secs_f64(),
                    path.display()
                );
                self.session = Some(session);
                self.selected = None;
                self.query.clear();
                self.applied_query.clear();
                self.draft = None;
            }
            Err(err) => self.error = Some(format!("{}: {err}", path.display())),
        }
        ctx.request_repaint();
    }

    /// Runs the deferred search once the user has stopped typing.
    fn settle_query(&mut self, ctx: &egui::Context) {
        let Some(changed_at) = self.query_changed_at else {
            return;
        };
        let due = changed_at + SEARCH_DEBOUNCE;
        if Instant::now() < due {
            ctx.request_repaint_after(due - Instant::now());
            return;
        }
        self.query_changed_at = None;
        let Some(session) = self.session.as_mut() else {
            return;
        };
        let started = Instant::now();
        let result = session.search(&self.query);
        let elapsed = started.elapsed().as_secs_f64();
        self.applied_query.clone_from(&self.query);
        if let Some(spec) = self.sort.clone() {
            session.sort(&spec);
        }
        self.status = if self.applied_query.trim().is_empty() {
            format!("{} entries", session.len())
        } else {
            format!(
                "Found {} results  |  shown {}  |  {:.3} s",
                result.total,
                result.rows.len(),
                elapsed
            )
        };
        self.selected = None;
    }

    /// Applies a sort, keeping the current filter.
    fn apply_sort(&mut self, field: SortField) {
        let descending = matches!(&self.sort, Some(s) if s.field == field && !s.descending);
        let spec = SortSpec { field, descending };
        if let Some(session) = self.session.as_mut() {
            session.sort(&spec);
        }
        self.sort = Some(spec);
        self.selected = None;
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("Open").clicked() {
                if let Some(path) = rfd_pick() {
                    self.begin_open(path);
                }
            }
            ui.separator();
            ui.label("Sort");
            for (label, field) in [
                ("Key", SortField::Key),
                ("Author", SortField::FirstAuthor),
                ("Year", SortField::Year),
                ("Title", SortField::Title),
            ] {
                let active = matches!(&self.sort, Some(s) if s.field == field);
                let arrow = match (&self.sort, active) {
                    (Some(s), true) if s.descending => " v",
                    (Some(_), true) => " ^",
                    _ => "",
                };
                if ui
                    .selectable_label(active, format!("{label}{arrow}"))
                    .clicked()
                {
                    self.apply_sort(field);
                }
            }
            ui.separator();
            let unsaved = self.session.as_ref().is_some_and(Session::is_edited);
            let label = if unsaved { "Save *" } else { "Save" };
            if ui
                .add_enabled(self.session.is_some(), egui::Button::new(label))
                .clicked()
            {
                if let Some(session) = &self.session {
                    match session.save() {
                        Ok(()) => self.status = String::from("Saved"),
                        Err(err) => self.status = format!("Save failed: {err}"),
                    }
                }
            }
        });
    }

    fn filter_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Search");
            let before = self.query.clone();
            let response = ui.add(
                egui::TextEdit::singleline(&mut self.query)
                    .hint_text("key, author, title... (space separates terms)")
                    .desired_width(f32::INFINITY),
            );
            // Compare the text rather than trusting `changed()`: an input method
            // or a synthetic key event can edit the buffer without flagging it.
            if self.query != before {
                self.query_changed_at = Some(Instant::now());
            }
            if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.query.clear();
                self.query_changed_at = Some(Instant::now());
            }
        });
    }

    fn groups(&mut self, ui: &mut egui::Ui) {
        ui.heading("Groups");
        ui.separator();
        ui.weak("Year");
        let mut years: Vec<String> = Vec::new();
        if let Some(session) = &self.session {
            for entry in session.library().entries() {
                if let Some(year) = entry.field("year") {
                    if !years.iter().any(|y| y == year) {
                        years.push(year.to_owned());
                    }
                }
            }
        }
        years.sort();
        years.reverse();
        for year in years.iter().take(40) {
            if ui.selectable_label(false, year).clicked() {
                self.query.clone_from(year);
                self.query_changed_at = Some(Instant::now());
            }
        }
    }

    /// Column widths in proportion to what each one holds, so the title gets
    /// the room an average title needs and a year column does not.
    fn column_sizes(&self) -> Vec<f32> {
        let total: f32 = self.columns.iter().map(|c| c.initial_width()).sum();
        let available = (self.table_width - 16.0).max(600.0);
        self.columns
            .iter()
            .map(|c| (c.initial_width() / total) * available)
            .collect()
    }

    fn table(&mut self, ui: &mut egui::Ui) {
        let widths = self.column_sizes();
        let rows = self.session.as_ref().map_or(0, |s| s.order().len());
        let selected = self.selected;
        let session = self.session.as_ref();
        let mut clicked = None;

        let mut table = TableBuilder::new(ui)
            .striped(true)
            .resizable(true)
            .sense(egui::Sense::click())
            .max_scroll_height(f32::INFINITY);
        for width in widths {
            table = table.column(TableColumn::initial(width).at_least(40.0));
        }
        table
            .header(22.0, |mut header| {
                for column in &self.columns {
                    header.col(|ui| {
                        ui.strong(column.header());
                    });
                }
            })
            .body(|body| {
                body.rows(ROW_HEIGHT, rows, |mut row| {
                    let index = row.index();
                    row.set_selected(selected == Some(index));
                    let entry = entry_at(session, index);
                    for column in &self.columns {
                        row.col(|ui| {
                            let text = column.cell(entry);
                            if column.is_numeric() {
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    ui.add(Label::new(text).truncate());
                                });
                            } else {
                                ui.add(Label::new(text).truncate());
                            }
                        });
                    }
                    if row.response().clicked() {
                        clicked = Some(index);
                    }
                });
            });

        if let Some(index) = clicked {
            if self.selected != Some(index) {
                self.draft = None;
            }
            self.selected = Some(index);
        }
    }

    fn status_line(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if self.pending.is_some() {
                ui.spinner();
                ui.label("Opening...");
            }
            if let Some(err) = &self.error {
                ui.colored_label(egui::Color32::LIGHT_RED, err);
            }
            ui.label(&self.status);
        });
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_pending(ctx);
        self.settle_query(ctx);

        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            self.toolbar(ui);
        });
        egui::TopBottomPanel::top("filter").show(ctx, |ui| {
            self.filter_bar(ui);
        });
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            self.status_line(ui);
        });

        egui::SidePanel::left("groups")
            .resizable(true)
            .default_width(180.0)
            .show(ctx, |ui| self.groups(ui));

        egui::SidePanel::right("detail")
            .resizable(true)
            .default_width(340.0)
            .show(ctx, |ui| {
                let outcome =
                    detail::show(ui, self.session.as_mut(), self.selected, &mut self.draft);
                if outcome == Outcome::Edited {
                    self.status = String::from("Edited, not saved yet");
                }
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            self.table_width = ui.available_width();
            if self.session.is_some() {
                self.table(ui);
            } else if self.pending.is_none() {
                ui.centered_and_justified(|ui| {
                    ui.label(RichText::new("Open a .bib file to begin.").weak());
                });
            }
        });

        if let Some(startup) = self.startup {
            let ready = self.session.is_some() || self.error.is_some();
            if ready {
                let elapsed = startup.elapsed();
                if self.status.is_empty() {
                    self.status = format!("ready in {:.2} s", elapsed.as_secs_f64());
                }
                self.startup = None;
            } else {
                ctx.request_repaint_after(Duration::from_millis(16));
            }
        }
    }
}

/// Asks the platform for a bibliography file.
fn rfd_pick() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter("BibTeX", &["bib"])
        .pick_file()
}
