//! A window over a sorted entry list: a toolbar, a filter box, a three-part body
//! of groups, table and detail pane, and a status line. The window appears
//! before the file is read, so a large library never shows a frozen frame.

mod columns;
mod detail;
mod ui;

pub use ui::run;
