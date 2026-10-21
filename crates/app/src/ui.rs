//! The window: a toolbar, a filter box, a three-part body and a status line.

use std::path::{Path, PathBuf};
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

/// How long an edit sits before it counts as a step of its own, so a burst of
/// changes to one entry undoes together rather than one change at a time.
const EDIT_PAUSE: Duration = Duration::from_millis(700);

/// The filter box's id, which `Ctrl+F` needs to reach it before it is drawn.
const FILTER_ID: &str = "filter_query";

/// Opens the window on a library, or on an empty session prompting for one.
pub fn run(path: Option<PathBuf>) -> eframe::Result<()> {
    let name = "citerack";
    let options = |renderer| eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1400.0, 880.0])
            .with_min_inner_size([800.0, 480.0])
            .with_title(name),
        renderer,
        ..Default::default()
    };
    let opened = |cc: &eframe::CreationContext<'_>| {
        Ok(Box::new(App::new(cc, path.clone())) as Box<dyn eframe::App>)
    };
    match eframe::run_native(name, options(eframe::Renderer::Glow), Box::new(opened)) {
        Err(err) if falls_back(&err) => eframe::run_native(
            name,
            options(eframe::Renderer::Wgpu),
            Box::new(move |cc| opened(cc)),
        ),
        other => other,
    }
}

/// Whether the error says the machine cannot give the window the backend it
/// asked for, which is the one case another backend can answer.
pub fn falls_back(err: &eframe::Error) -> bool {
    matches!(
        err,
        eframe::Error::Glutin(_)
            | eframe::Error::NoGlutinConfigs(..)
            | eframe::Error::OpenGL(_)
            | eframe::Error::Wgpu(_)
    )
}

/// A file being read on a worker thread, so the window can appear first.
struct Pending {
    path: PathBuf,
    started: Instant,
    rx: Receiver<Result<Session, citerack_store::StoreError>>,
}

/// What the window shows and what it remembers between frames.
pub struct App {
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
    /// Set by `Ctrl+F`, cleared by the frame that puts the cursor in the filter.
    focus_filter: bool,
    /// A row the table should bring into view on the next frame, which is what a
    /// keyboard move out of sight asks for.
    scroll_to: Option<usize>,
    /// The field of the selected row a key asked for the cursor to go to, cleared
    /// by the pane's own frame.
    focus_field: Option<usize>,
    /// The field whose box held the cursor when the pane was last drawn.
    focused_field: Option<usize>,
    /// Whether `Shift` was held when the keys were read, which turns `Tab` around.
    shift_tab: bool,
}

impl App {
    fn new(_cc: &eframe::CreationContext<'_>, path: Option<PathBuf>) -> Self {
        let mut app = Self::empty();
        if let Some(path) = path {
            app.begin_open(path);
        }
        app
    }

    /// A window with nothing open, which a test then fills in.
    pub fn empty() -> Self {
        Self {
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
            focus_filter: false,
            scroll_to: None,
            focus_field: None,
            focused_field: None,
            shift_tab: false,
        }
    }

    /// Opens an already-read session, skipping the worker a real open uses.
    pub fn with_session(session: Session) -> Self {
        let mut app = Self::empty();
        app.status = format!("{} entries", session.len());
        app.session = Some(session);
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

    /// Selects a row of the current order, as a click on the table would.
    pub fn select(&mut self, row: Option<usize>) {
        if self.selected != row {
            self.draft = None;
        }
        self.selected = row;
    }

    /// The row the detail pane is showing.
    pub fn selected(&self) -> Option<usize> {
        self.selected
    }

    /// The line at the bottom of the window.
    pub fn status(&self) -> &str {
        &self.status
    }

    /// The session, for a test reading the library behind the window.
    pub fn session(&self) -> Option<&Session> {
        self.session.as_ref()
    }

    /// The file being read right now, which is how a test sees that an open was
    /// asked for without a dialog standing in the way.
    pub fn opening(&self) -> Option<&Path> {
        self.pending.as_ref().map(|pending| pending.path.as_path())
    }

    /// Whether the filter box holds the cursor, which `Ctrl+F` is for.
    pub fn filter_focused(&self, ctx: &egui::Context) -> bool {
        ctx.memory(|memory| memory.has_focus(egui::Id::new(FILTER_ID)))
    }

    /// Whether a field box of the detail pane holds the cursor.
    pub fn field_focused(&self) -> bool {
        self.focused_field.is_some()
    }

    /// Which field of the entry the cursor is in, counted in file order.
    pub fn focused_field(&self) -> Option<usize> {
        self.focused_field
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("Open").on_hover_text("Ctrl+O").clicked() {
                self.ask_for_file();
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
                .on_hover_text("Ctrl+S")
                .clicked()
            {
                self.save();
            }
            ui.separator();
            let undo = self.session.as_ref().is_some_and(Session::can_undo);
            let redo = self.session.as_ref().is_some_and(Session::can_redo);
            if ui
                .add_enabled(undo, egui::Button::new("Undo"))
                .on_hover_text("Ctrl+Z")
                .clicked()
            {
                self.undo();
            }
            if ui
                .add_enabled(redo, egui::Button::new("Redo"))
                .on_hover_text("Ctrl+Shift+Z")
                .clicked()
            {
                self.redo();
            }
            ui.separator();
            self.duplicates_button(ui);
        });
    }

    /// The button that narrows the table to the entries in a duplicate pair. It
    /// says how many pairs there are, since a count of zero is the answer a user
    /// wants most of the time.
    fn duplicates_button(&mut self, ui: &mut egui::Ui) {
        let Some(session) = &mut self.session else {
            return;
        };
        let count = session.duplicate_count();
        let showing = session.showing_only_duplicates();
        let label = if showing {
            format!("Duplicates {count} (shown)")
        } else {
            format!("Duplicates {count}")
        };
        let clicked = ui
            .add_enabled(count > 0, egui::Button::new(label))
            .on_hover_text("show only the entries that look like the same work twice")
            .clicked();
        if clicked {
            session.show_only_duplicates(!showing);
            // The rows the table shows just changed under it, so a selection made
            // in the old list no longer points at what the user was looking at.
            self.selected = None;
            self.draft = None;
            self.status = if showing {
                String::from("Showing every entry")
            } else {
                let pairs = if count == 1 { "pair" } else { "pairs" };
                format!("Showing the entries in {count} duplicate {pairs}")
            };
        }
    }

    /// Moves the selection by `step` rows, clamped to the table. From nothing
    /// selected, a down press takes the first row and an up press the last, so
    /// either key does something wherever the user is.
    fn move_selection(&mut self, step: isize) {
        let rows = self.session.as_ref().map_or(0, |s| s.order().len());
        if rows == 0 {
            return;
        }
        let next = match self.selected {
            Some(at) => (at as isize + step).clamp(0, rows as isize - 1) as usize,
            None if step > 0 => 0,
            None => rows - 1,
        };
        if self.selected != Some(next) {
            self.draft = None;
        }
        self.selected = Some(next);
        self.scroll_to = Some(next);
    }

    /// The key bindings, in one place so `docs/Ui-spec.md` has one thing to
    /// describe. Save and open follow the platform's own conventions, and undo
    /// works wherever the cursor is because the history belongs to the library.
    fn shortcuts(&mut self, ctx: &egui::Context) {
        // The arrows move the table only when no box holds the cursor, where they
        // belong to the text inside it. Asking takes the input lock, so it is asked
        // before the lock is held rather than from inside.
        let typing = ctx.wants_keyboard_input();
        let in_field = self.focused_field.is_some();
        self.shift_tab = ctx.input(|i| i.modifiers.shift);
        let (open, save, find, fields, undo, redo, down, up, tab, escape) = ctx.input(|i| {
            let command = i.modifiers.command;
            let plain = command && !i.modifiers.shift;
            let undo = plain && i.key_pressed(egui::Key::Z);
            let redo = (command && i.modifiers.shift && i.key_pressed(egui::Key::Z))
                || (command && i.key_pressed(egui::Key::Y));
            (
                plain && i.key_pressed(egui::Key::O),
                plain && i.key_pressed(egui::Key::S),
                plain && i.key_pressed(egui::Key::F),
                plain && i.key_pressed(egui::Key::D),
                undo,
                redo,
                !typing && i.key_pressed(egui::Key::ArrowDown),
                !typing && i.key_pressed(egui::Key::ArrowUp),
                // Tab walks the fields of the entry the pane is showing, which is
                // what makes the pane reachable without a click. Elsewhere it is
                // left to egui, which moves through the toolbar and the facets.
                in_field && i.key_pressed(egui::Key::Tab),
                in_field && i.key_pressed(egui::Key::Escape),
            )
        });
        if open {
            self.ask_for_file();
        }
        if save {
            self.save();
        }
        if find {
            self.focus_filter = true;
        }
        if fields {
            self.focus_field = Some(0);
        }
        if undo {
            self.undo();
        } else if redo {
            self.redo();
        }
        if down {
            self.move_selection(1);
        } else if up {
            self.move_selection(-1);
        }
        if tab {
            self.step_field();
        }
        if escape {
            if let Some(focused) = ctx.memory(|memory| memory.focused()) {
                ctx.memory_mut(|memory| memory.surrender_focus(focused));
            }
        }
    }

    /// Moves the cursor to the next or previous field of the entry, wrapping at
    /// either end so a user pressing Tab repeatedly cycles the whole entry.
    fn step_field(&mut self) {
        let (Some(current), Some(count)) = (self.focused_field, self.field_count()) else {
            return;
        };
        if count == 0 {
            return;
        }
        let step = if self.shift_tab { count - 1 } else { 1 };
        self.focus_field = Some((current + step) % count);
    }

    /// How many fields the entry on display has.
    fn field_count(&self) -> Option<usize> {
        let row = self.selected?;
        self.session
            .as_ref()?
            .library()
            .entries()
            .get(row)
            .map(|entry| entry.fields.len())
    }

    /// Opens the platform's file dialog, which blocks until the user answers.
    fn ask_for_file(&mut self) {
        if let Some(path) = rfd_pick() {
            self.begin_open(path);
        }
    }

    /// Writes the library back, and says in the status line how it went.
    fn save(&mut self) {
        let Some(session) = &mut self.session else {
            return;
        };
        self.status = match session.save() {
            Ok(()) => String::from("Saved"),
            Err(err) => format!("Save failed: {err}"),
        };
    }

    /// Closes a change once the typing has paused, which is what makes a run of
    /// edits one step and a later visit to the same entry a second.
    fn settle_edits(&mut self) {
        if let Some(session) = &mut self.session {
            session.settle_at(Instant::now(), EDIT_PAUSE);
        }
    }

    /// Reverses the last change, and says in the status line what it undid.
    fn undo(&mut self) {
        let Some(session) = &mut self.session else {
            return;
        };
        self.draft = None;
        self.status = match session.undo() {
            Some(what) => format!("Undid {what}"),
            None => String::from("Nothing to undo"),
        };
        // The row a restore touched may no longer be where the filter looks, and
        // a search right after an undo has to read the text that is back.
        self.rematch();
    }

    /// Puts back what the last undo reversed.
    fn redo(&mut self) {
        let Some(session) = &mut self.session else {
            return;
        };
        self.draft = None;
        self.status = match session.redo() {
            Some(what) => format!("Redid {what}"),
            None => String::from("Nothing to redo"),
        };
        self.rematch();
    }

    /// Runs the filter again against whatever the library now holds, so the
    /// table never shows a row the index no longer matches.
    fn rematch(&mut self) {
        let query = self.applied_query.clone();
        if let Some(session) = &mut self.session {
            if query.trim().is_empty() {
                session.clear_search();
            } else {
                session.search(&query);
            }
            if let Some(sort) = &self.sort {
                session.sort(sort);
            }
        }
    }

    fn filter_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Search");
            let before = self.query.clone();
            let response = ui.add(
                egui::TextEdit::singleline(&mut self.query)
                    .id(egui::Id::new(FILTER_ID))
                    .hint_text("key, author, title... (space separates terms)")
                    .desired_width(f32::INFINITY),
            );
            if self.focus_filter {
                response.request_focus();
                self.focus_filter = false;
            }
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
        // A row reached by a key is brought into view, and a row reached by a
        // click is already there under the pointer.
        if let Some(row) = self.scroll_to.take() {
            table = table.scroll_to_row(row, None);
        }
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
        self.draw(ctx);
    }
}

impl App {
    /// One frame, without an eframe frame around it, so a test can call it.
    pub fn draw(&mut self, ctx: &egui::Context) {
        self.poll_pending(ctx);
        self.settle_query(ctx);
        self.shortcuts(ctx);
        self.settle_edits();

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
                let outcome = detail::show(
                    ui,
                    self.session.as_mut(),
                    self.selected,
                    &mut self.draft,
                    &mut self.focus_field,
                    &mut self.focused_field,
                );
                if outcome == Outcome::Edited {
                    self.status = String::from("Edited, not saved yet");
                    if let Some(session) = &mut self.session {
                        session.edited_at(Instant::now());
                    }
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
