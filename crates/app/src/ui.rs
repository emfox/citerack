//! The window: a toolbar, a filter box, a three-part body and a status line.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use citerack_index::{SortField, SortSpec};
use citerack_store::{Facet, Session};
use egui::{Align, Label, Layout, RichText, Theme, ThemePreference};
use egui_extras::{Column as TableColumn, TableBuilder};

use crate::attach;
use crate::batch::{self, Batch};
use crate::columns::{Column, entry_at};
use crate::detail::{self, Draft, Outcome};
use crate::import;
use crate::lookup::{Lookup, State};

/// How long a keystroke waits before the search runs, so a typed word costs one
/// query rather than one per character.
const SEARCH_DEBOUNCE: Duration = Duration::from_millis(120);

/// Row height in points; a two-line title would make every row as tall as it is.
const ROW_HEIGHT: f32 = 20.0;

/// How long an edit sits before it counts as a step of its own, so a burst of
/// changes to one entry undoes together rather than one change at a time.
const EDIT_PAUSE: Duration = Duration::from_millis(700);

/// How many values a facet list shows before it says how many it left out.
const LISTED: usize = 12;

/// The filter box's id, which `Ctrl+F` needs to reach it before it is drawn.
const FILTER_ID: &str = "filter_query";

/// The key pattern's id, so a test can tell the setting apart from the fields.
const PATTERN_ID: &str = "key_pattern";

/// Opens the window on a library, or on an empty session prompting for one.
pub fn run(path: Option<PathBuf>) -> eframe::Result<()> {
    let name = "citerack";
    let options = |renderer| eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1400.0, 880.0])
            .with_min_inner_size([800.0, 480.0])
            .with_title(name),
        renderer,
        wgpu_options: wgpu_options(),
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

/// The backends the second renderer may use, which on Windows leaves D3D12 out
/// where the shader compiler it needs was not found at startup.
#[cfg(windows)]
fn wgpu_options() -> eframe::egui_wgpu::WgpuConfiguration {
    use eframe::egui_wgpu::{WgpuConfiguration, WgpuSetup};

    let mut options = WgpuConfiguration::default();
    if let (false, WgpuSetup::CreateNew(setup)) =
        (crate::windows::shader_compiler(), &mut options.wgpu_setup)
    {
        setup
            .instance_descriptor
            .backends
            .remove(wgpu::Backends::DX12);
    }
    options
}

#[cfg(not(windows))]
fn wgpu_options() -> eframe::egui_wgpu::WgpuConfiguration {
    eframe::egui_wgpu::WgpuConfiguration::default()
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
    /// The text in the key pattern box, which is what the user is in the middle of
    /// writing and may not read as a pattern yet.
    pattern: String,
    /// Whether the next frame should put the caret in the pattern box.
    focus_pattern: bool,
    /// The row the table has picked, which the detail pane is showing.
    selected: Option<usize>,
    /// The other end of a range picked with a modifier or a held key, in show
    /// order. `None` is a single row; `Some(at)` picks every row between the two.
    reach: Option<usize>,
    sort: Option<SortSpec>,
    columns: Vec<Column>,
    status: String,
    startup: Option<Instant>,
    table_width: f32,
    /// The field box being typed in, kept between frames so a refusal keeps it.
    draft: Option<Draft>,
    /// The boxes a batch write is typed into, kept so a refusal keeps them.
    batch: Batch,
    /// Why the last batch press wrote to nobody, kept beside the boxes.
    batch_refusal: Option<String>,
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
    /// The reason the key pattern in the box cannot be used, shown beside it.
    refusal: String,
    /// Which theme the window asks the context for, kept here rather than read
    /// back from it so a frame can be drawn without asking.
    theme: ThemePreference,
    /// Whether the import box has the pane, and what it holds.
    importing: bool,
    identifier: String,
    lookup: Lookup,
    lookup_state: State,
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
            pattern: String::from(citerack_bib::DEFAULT_KEY_PATTERN),
            focus_pattern: false,
            selected: None,
            reach: None,
            sort: None,
            columns: Column::ALL.to_vec(),
            status: String::new(),
            startup: Some(Instant::now()),
            table_width: 700.0,
            draft: None,
            batch: Batch::default(),
            batch_refusal: None,
            focus_filter: false,
            scroll_to: None,
            focus_field: None,
            focused_field: None,
            shift_tab: false,
            refusal: String::new(),
            theme: ThemePreference::System,
            importing: false,
            identifier: String::new(),
            lookup: Lookup::default(),
            lookup_state: State::Idle,
        }
    }

    /// Opens an already-read session, skipping the worker a real open uses.
    pub fn with_session(session: Session) -> Self {
        let mut app = Self::empty();
        app.status = format!("{} entries", session.len());
        app.pattern = session.key_pattern();
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
                self.clear_selection();
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
        self.clear_selection();
    }

    /// Applies a sort, keeping the current filter.
    fn apply_sort(&mut self, field: SortField) {
        let descending = matches!(&self.sort, Some(s) if s.field == field && !s.descending);
        let spec = SortSpec { field, descending };
        if let Some(session) = self.session.as_mut() {
            session.sort(&spec);
        }
        self.sort = Some(spec);
        self.clear_selection();
    }

    /// Selects a row of the current order, as a click on the table would.
    pub fn select(&mut self, row: Option<usize>) {
        if self.selected != row {
            self.draft = None;
        }
        self.selected = row;
    }

    /// Drops the selection and the range with it.
    fn clear_selection(&mut self) {
        self.selected = None;
        self.reach = None;
    }

    /// Picks a row as a click does: one row, and the range let go.
    fn click_row(&mut self, row: usize) {
        if self.selected != Some(row) {
            self.draft = None;
        }
        self.selected = Some(row);
        self.reach = None;
    }

    /// Adds every row between the selection and `row`, as `Shift` and a click do.
    /// The anchor stays where it was, so dragging the end walks the range out and
    /// back rather than growing it one row at a time.
    fn extend_to(&mut self, row: usize) {
        self.reach = (self.selected != Some(row)).then_some(row);
    }

    /// The rows the table is showing as picked, in show order.
    pub fn picked(&self) -> Vec<usize> {
        let Some(from) = self.selected else {
            return Vec::new();
        };
        let Some(to) = self.reach else {
            return vec![from];
        };
        let (low, high) = if from <= to { (from, to) } else { (to, from) };
        (low..=high).collect()
    }

    /// Lets the pick go back to one row, as a press with no modifier does.
    pub fn clear_pick(&mut self) {
        self.reach = None;
    }

    /// Fills the batch boxes, which is what a user typing in them does, so a test
    /// can reach the press without a pointer.
    pub fn type_batch(&mut self, name: &str, value: &str) {
        self.batch.name = name.to_owned();
        self.batch.value = value.to_owned();
        self.batch_refusal = None;
    }

    /// The field and value in the batch boxes, as they stand.
    pub fn batch(&self) -> (&str, &str) {
        (&self.batch.name, &self.batch.value)
    }

    /// Whether more than one row is picked, which is what the batch box acts on.
    pub fn picking_many(&self) -> bool {
        self.picked().len() > 1
    }

    /// Puts the caret in the key pattern box, the way a click there does, so a
    /// test can type a pattern the file is not using yet.
    pub fn focus_pattern(&mut self) {
        self.focus_pattern = true;
    }

    /// Which theme the window is set to follow.
    pub fn theme(&self) -> ThemePreference {
        self.theme
    }

    /// Chooses a theme, as pressing one of the three words in the toolbar does.
    pub fn set_theme(&mut self, theme: ThemePreference) {
        self.theme = theme;
    }

    /// The theme the window is drawn in, which is `System` read out: a machine
    /// that reports nothing is drawn light rather than dark.
    pub fn drawn_theme(&self, ctx: &egui::Context) -> Theme {
        match self.theme {
            ThemePreference::System => ctx.system_theme().unwrap_or(Theme::Light),
            ThemePreference::Dark => Theme::Dark,
            ThemePreference::Light => Theme::Light,
        }
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
            if ui
                .add_enabled(self.session.is_some(), egui::Button::new("Import"))
                .on_hover_text("an entry from a DOI or an arXiv id")
                .clicked()
            {
                self.importing = true;
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
            self.keys(ui);
            ui.separator();
            self.duplicates_button(ui);
            self.theme_switch(ui);
        });
    }

    /// The three themes as three words, right-aligned where every other
    /// application puts this switch. `System` says which theme it resolves to, so a
    /// press that changes nothing visible still reads as an answer.
    fn theme_switch(&mut self, ui: &mut egui::Ui) {
        let system = ui.ctx().system_theme();
        let here = match system {
            Some(Theme::Dark) => "System (dark)",
            Some(Theme::Light) => "System (light)",
            None => "System",
        };
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            for (theme, label) in [
                (ThemePreference::Light, "Light"),
                (ThemePreference::Dark, "Dark"),
                (ThemePreference::System, here),
            ] {
                if ui.selectable_label(self.theme == theme, label).clicked() {
                    self.theme = theme;
                }
            }
        });
    }

    /// The key pattern and the button that gives every entry the key it makes.
    /// The box is the setting and the button is the act, so a pattern can be
    /// tried on the Generated column before anything in the file moves.
    fn keys(&mut self, ui: &mut egui::Ui) {
        let Some(session) = &mut self.session else {
            return;
        };
        ui.label("Key");
        // The box keeps its own text, so a pattern the parser has not been given
        // yet is still on screen to be finished. A pattern only reaches the
        // session once it reads, which is why a refusal has to be drawn beside it.
        let field = ui
            .add_sized(
                [150.0, 20.0],
                egui::TextEdit::singleline(&mut self.pattern)
                    .id_salt(PATTERN_ID)
                    .hint_text(citerack_bib::DEFAULT_KEY_PATTERN),
            )
            .on_hover_text("the fields a generated key is made of, as {author}{year}");
        if self.focus_pattern {
            field.request_focus();
            self.focus_pattern = false;
        }
        if field.changed() {
            match session.set_key_pattern(&self.pattern) {
                Ok(()) => {
                    self.status = format!("Keys follow {}", self.pattern);
                    self.refusal.clear();
                }
                Err(err) => self.refusal = format!("that pattern is not one: {err}"),
            }
        }
        let enabled = self.session.as_ref().is_some_and(|s| !s.is_empty());
        if ui
            .add_enabled(enabled, egui::Button::new("Generate keys"))
            .on_hover_text("give every entry whose key the pattern makes the key it makes now")
            .clicked()
        {
            self.generate_all();
        }
        if !self.refusal.is_empty() {
            ui.colored_label(egui::Color32::from_rgb(224, 122, 95), &self.refusal);
        }
    }

    /// Gives the entry on a displayed row the key the Generated column shows
    /// beside it, which is what a double click asks for.
    fn generate_key(&mut self, row: usize) {
        let Some(session) = &mut self.session else {
            return;
        };
        let Some(entry) = session.order().get(row).copied() else {
            return;
        };
        let Some(now) = session.proposed_key_at(row) else {
            return;
        };
        let was = session
            .library()
            .entries()
            .get(entry)
            .map(|entry| entry.key.to_string())
            .unwrap_or_default();
        match session.set_key(entry, &now) {
            Ok(()) => {
                self.status = format!("Renamed {was} to {now}");
                session.edited_at(Instant::now());
            }
            Err(err) => self.status = err.as_str().to_owned(),
        }
    }

    /// Gives every entry whose key the pattern makes the key it makes now.
    fn generate_all(&mut self) {
        let Some(session) = &mut self.session else {
            return;
        };
        match session.generate_keys() {
            Ok(0) => {
                self.status =
                    String::from("Every key already follows the pattern, or was written by hand");
            }
            Ok(written) => {
                self.status = format!("Renamed {written} keys, not saved yet");
                session.edited_at(Instant::now());
            }
            Err(err) => self.status = err.as_str().to_owned(),
        }
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
            self.clear_selection();
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
    fn move_selection(&mut self, step: isize, extend: bool) {
        let rows = self.session.as_ref().map_or(0, |s| s.order().len());
        if rows == 0 {
            return;
        }
        // A held `Shift` moves the far end of the range rather than the anchor, so
        // a range is walked out one row per press from a single starting point.
        let from = if extend || self.reach.is_some() {
            self.reach.or(self.selected)
        } else {
            self.selected
        };
        let next = match from {
            Some(at) => (at as isize + step).clamp(0, rows as isize - 1) as usize,
            None if step > 0 => 0,
            None => rows - 1,
        };
        if self.selected.is_none() || (!extend && self.selected != Some(next)) {
            self.draft = None;
        }
        if self.selected.is_none() {
            self.selected = Some(next);
        } else if extend {
            self.extend_to(next);
        } else {
            self.selected = Some(next);
            self.reach = None;
        }
        self.scroll_to = Some(next);
    }

    /// Draws the batch box in the pane's place while several rows are picked.
    fn detail_off(&mut self, ui: &mut egui::Ui) {
        let rows = self.picked();
        let outcome = batch::show(
            ui,
            &mut self.session,
            &rows,
            &mut self.batch,
            &mut self.batch_refusal,
        );
        let batch::Outcome::Wrote { written, refused } = outcome else {
            // A press that wrote to nobody left its reason in the box above the
            // button; the status line says it too, since that is where a user
            // looks after a press that seemed to do nothing.
            if let Some(why) = &self.batch_refusal {
                self.status = why.clone();
            }
            return;
        };
        let noun = if written == 1 { "entry" } else { "entries" };
        self.status = if refused == 0 {
            format!("Wrote {} to {written} {noun}", self.batch.name.trim())
        } else {
            format!(
                "Wrote {} to {written} {noun}; {refused} could not take it",
                self.batch.name.trim()
            )
        };
        self.batch_refusal = None;
        // The batch is a change like any other, so the save dot is set and the
        // pause before an undo step begins now.
        if let Some(session) = &mut self.session {
            session.edited_at(Instant::now());
        }
    }

    /// Draws the import box, and reads an answer that arrived since the last frame.
    fn import_box(&mut self, ui: &mut egui::Ui) {
        if let Some(answer) = self.lookup.read() {
            self.lookup_state = match answer {
                Ok(record) => State::Read(Box::new(record)),
                Err(why) => State::Failed(why),
            };
        }
        let outcome = import::show(
            ui,
            &mut self.session,
            &mut self.identifier,
            &mut self.lookup_state,
            &mut self.lookup,
        );
        match outcome {
            import::Outcome::Asking(id) => self.status = format!("Asking for {id}"),
            import::Outcome::Filed { row, key } => {
                self.importing = false;
                self.identifier.clear();
                self.lookup_state = State::Idle;
                // The new row is picked, so the pane shows what arrived rather than
                // leaving the reader to find it in a table it may be sorted away in.
                self.select(Some(row));
                self.status = format!("Imported {key}");
            }
            import::Outcome::Closed => self.importing = false,
            import::Outcome::Idle => {}
        }
    }

    /// Whether the import box has the pane, for a test reading the window.
    pub fn importing(&self) -> bool {
        self.importing
    }

    /// Opens the import box as pressing the toolbar's button does.
    pub fn begin_import(&mut self) {
        self.importing = true;
    }

    /// Types an identifier into the import box, without asking for it.
    pub fn type_identifier(&mut self, text: &str) {
        self.importing = true;
        self.identifier = text.to_owned();
    }

    /// What the import box is saying, for a test that cannot wait for a network.
    pub fn lookup_state(&self) -> &State {
        &self.lookup_state
    }

    /// Puts a record in the box as if a service had answered with it, which is how
    /// the filing half is tested without a request going out.
    pub fn offer(&mut self, record: citerack_bib::Entry) {
        self.importing = true;
        self.lookup_state = State::Read(Box::new(record));
    }

    /// Picks every row the table is showing, which is what a batch on the whole
    /// result of a search is done from.
    fn select_all(&mut self) {
        let rows = self.session.as_ref().map_or(0, |s| s.order().len());
        if rows == 0 {
            return;
        }
        self.selected = Some(0);
        self.reach = Some(rows - 1);
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
        let shift = ctx.input(|i| i.modifiers.shift);
        let (open, save, find, fields, undo, redo, down, up, tab, escape, all) = ctx.input(|i| {
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
                // Only when no box holds the cursor, where it means "select all
                // the text" and is left to egui.
                !typing && plain && i.key_pressed(egui::Key::A),
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
            self.move_selection(1, shift);
        } else if up {
            self.move_selection(-1, shift);
        }
        if all {
            self.select_all();
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
            .map(detail::rows_count)
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
        ui.horizontal(|ui| {
            ui.heading("Groups");
            let narrowed = self.session.as_ref().map_or(0, Session::facet_count);
            if narrowed > 0 && ui.small_button("Clear").clicked() {
                if let Some(session) = &mut self.session {
                    session.clear_facets();
                }
                self.after_narrowing();
            }
        });
        ui.separator();
        // The lists the question "narrow it" is asked with, in the order a reader
        // narrows: who, then where, then when, then what kind.
        let mut pressed: Option<(Facet, String)> = None;
        let mut cleared: Option<Facet> = None;
        for facet in Facet::ALL {
            // Read the list, the pick and the count in one go, since counting a
            // list is the only thing here that touches the whole table.
            let Some(session) = &mut self.session else {
                break;
            };
            let values = session.facet_values(facet).to_vec();
            let picked = session.picked(facet).map(str::to_owned);
            let shown = values.len();
            let heading = match &picked {
                Some(picked) => format!("{}: {picked}", facet.heading()),
                None => format!("{} ({shown})", facet.heading()),
            };
            // A short list opens so its values are read without a click; a long
            // one stays shut, since its first twelve would fill the panel.
            let open = shown <= LISTED;
            egui::CollapsingHeader::new(heading)
                .id_salt(facet.heading())
                .default_open(open)
                .show(ui, |ui| {
                    if let Some(picked) = &picked {
                        if ui.small_button("Show every value").clicked() {
                            cleared = Some(facet);
                        }
                        // The heading already names the value, so this press says
                        // what it does rather than repeating the word above it.
                        if ui
                            .add(egui::Button::selectable(
                                true,
                                format!("{picked}  (let go)"),
                            ))
                            .clicked()
                        {
                            pressed = Some((facet, picked.clone()));
                        }
                        ui.separator();
                    }
                    for value in values.iter().take(LISTED) {
                        if picked.as_deref() == Some(value.label.as_str()) {
                            continue;
                        }
                        let label = format!("{} ({})", value.label, value.count);
                        if ui
                            .add(egui::Button::selectable(false, label).frame(false))
                            .clicked()
                        {
                            pressed = Some((facet, value.label.clone()));
                        }
                    }
                    if shown > LISTED {
                        ui.weak(format!("and {} more", shown - LISTED));
                    }
                });
        }
        if let Some(facet) = cleared
            && let Some(session) = &mut self.session
        {
            session.clear_facet(facet);
            self.after_narrowing();
        }
        if let Some((facet, value)) = pressed
            && let Some(session) = &mut self.session
        {
            session.toggle_facet(facet, &value);
            self.after_narrowing();
        }
    }

    /// Called when a facet changed, since the rows behind the selection are not
    /// the ones the user was looking at any more.
    fn after_narrowing(&mut self) {
        self.clear_selection();
        self.draft = None;
        let shown = self.session.as_ref().map_or(0, |s| s.order().len());
        let narrowing = self.session.as_ref().map_or(0, |s| s.facet_count());
        self.status = if narrowing == 0 {
            format!("{shown} entries")
        } else {
            let lists = if narrowing == 1 { "list" } else { "lists" };
            format!("{shown} entries  |  narrowed by {narrowing} facet {lists}")
        };
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
        let picked = self.picked();
        let session = self.session.as_ref();
        let mut clicked = None;
        // A click that holds `Shift` walks the range out to the row it landed on
        // rather than replacing the pick, which is one gesture for many rows.
        let mut extended = None;
        // The key a double click writes, which is the one the Generated column
        // already shows for that row.
        let mut rename = None;

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
                    row.set_selected(picked.contains(&index));
                    let entry = entry_at(session, index);
                    for column in &self.columns {
                        row.col(|ui| {
                            let text = match column {
                                Column::Generated => session
                                    .and_then(|s| s.proposed_key_at(index))
                                    .unwrap_or_default(),
                                _ => column.cell(entry),
                            };
                            if column.is_numeric() {
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    ui.add(Label::new(text).truncate());
                                });
                            } else {
                                ui.add(Label::new(text).truncate());
                            }
                        });
                    }
                    let response = row.response();
                    if response.clicked() {
                        if response.ctx.input(|i| i.modifiers.shift) {
                            extended = Some(index);
                        } else {
                            clicked = Some(index);
                        }
                    }
                    // A second click on a row gives its entry the key the next
                    // column shows, which is one gesture for one entry.
                    if response.double_clicked() {
                        rename = Some(index);
                    }
                });
            });

        if let Some(index) = clicked {
            self.click_row(index);
        } else if let Some(index) = extended {
            self.extend_to(index);
        }
        if let Some(index) = rename {
            self.generate_key(index);
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
        // The theme is set every frame, not once at startup, because a machine
        // that changes its mind mid-session reports it as input and nothing else
        // would notice.
        ctx.set_theme(ThemePreference::System);
        ctx.options_mut(|options| options.fallback_theme = Theme::Light);
        ctx.set_theme(self.theme);
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
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| self.groups(ui));
            });

        egui::SidePanel::right("detail")
            .resizable(true)
            .default_width(340.0)
            .show(ctx, |ui| {
                if self.importing {
                    self.import_box(ui);
                    return;
                }
                if self.picking_many() {
                    self.detail_off(ui);
                    return;
                }
                let outcome = detail::show(
                    ui,
                    self.session.as_mut(),
                    self.selected,
                    &mut self.draft,
                    &mut self.focus_field,
                    &mut self.focused_field,
                    true,
                );
                match outcome {
                    Outcome::Edited => {
                        self.status = String::from("Edited, not saved yet");
                        if let Some(session) = &mut self.session {
                            session.edited_at(Instant::now());
                        }
                    }
                    // A file is opened by the desktop, and the window says which
                    // one so a click that started nothing is visible as such.
                    Outcome::Opened(path) => {
                        self.status = match attach::open(&path) {
                            Ok(()) => format!("Opened {}", path.display()),
                            Err(why) => format!("{why}: {}", path.display()),
                        };
                    }
                    Outcome::Idle => {}
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
