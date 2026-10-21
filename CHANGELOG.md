# Changelog

What a reader of the release notes can now do. Anything that changed only inside
the repository is not listed here.

## 0.1.0 - 21 October 2026

The first release. A library opens in a window, a filter box narrows it, and an
edited field is written back without moving a byte the edit did not touch.

### Added

- Open a BibTeX library in a window with a filter box, a sortable entry table of
  key, type, author, title, year and venue, a detail pane for the selected entry,
  and a year facet.
- Save a library byte for byte, so a save that changed nothing leaves the file as
  it was and a field the parser does not model survives a round trip.
- Search every field at once, ignoring case and accents, with space-separated
  terms that may match different fields.
- `citerack --open-time` and `citerack --search`, which measure the engine with no
  display attached.
- A search is answered from an index rather than by reading every entry, so a
  word nobody wrote returns at once and a word somebody wrote costs only the rows
  that hold it. On a library of 100 000 entries a query takes between 0.04 ms and
  4 ms, against 50 ms before.
- A fragment of one to three letters, which is what a filter box holds after the
  first keystroke, is answered as fast as a whole word. Typing one letter into a
  real library of 134 352 entries used to take 17.9 ms and now takes 2.9 ms, and
  three letters into a library of 30 000 entries with no repeated word used to
  take 13.4 ms and now takes 0.05 ms.
- An edit costs the row it touched rather than the library: typing into a field of
  a library of 50 000 entries took 11 s per keystroke and now takes a tenth of a
  millisecond, and the search that follows costs 82 ms once rather than on every
  key.
- `citerack --explain`, which prints how many rows each column holds for a query,
  so a search that returns too much or too little can be traced to a field.
- `citerack --edit-time`, which measures a keystroke, the search after it, and a
  second keystroke, so the cost of an edit can be checked without a window.
- `Ctrl+Z` takes back the last edit and `Ctrl+Shift+Z` puts it back, so a wrong
  value is one keystroke from being undone and a save after it writes the file as
  it was. A run of typing into one field undoes as one step; a pause starts
  another, and an edit to several entries undoes together.
- The whole window works without a mouse. `Up` and `Down` walk the table and stop
  at either end, scrolling the row they land on into view; `Ctrl+D` reaches the
  detail pane, where `Tab` and `Shift+Tab` walk the fields of the entry and
  `Escape` leaves the box with what was typed kept. `Ctrl+O`, `Ctrl+S`, `Ctrl+F`,
  `Ctrl+Z` and `Ctrl+Shift+Z` reach the rest, each of them a shortcut for a
  control that is already there.
- A save now clears the `Save *` marker. Before this, the star stayed after the work
  had been written, so a saved library still looked unsaved and a second save was
  offered work that no longer existed. The window now distinguishes "edited since
  the file was read" from "waiting to be written", which is the question the marker
  was always meant to answer.
- A window opens on hardware whose GL driver is broken. The window is asked for
  with `glow` first and, when the machine will not give it a GL context, the same
  process asks `wgpu` for the same window before reporting anything, so a machine
  that cannot draw one way is tried the other rather than refused.
- `citerack --save-time`, which writes an edited copy and reports how long it took,
  so the save path can be measured without a window.
- A duplicate is named where it already appears: the detail pane shows a line for
  each entry the selected one looks like, and the toolbar counts the pairs. Two
  entries match on a DOI, or on a title and a year, compared with the DOI's
  resolver prefix and the title's case, accents, braces and punctuation folded
  away, so a second copy pasted from a publisher's site is caught as readily as one
  retyped by hand. Pressing the count narrows the table to the entries involved and
  pressing it again brings the rest back.
- A tagged release publishes a Linux binary and a Windows one, each with a
  checksum beside it. The Windows build links its runtime statically, so it runs
  on a machine that never had a compiler installed.
- `citerack --index-report`, which prints where the index spends its memory per
  field, so a library that opens slowly can be traced to the field responsible.
  It also counts the distinct short fragments each field's word list holds and the
  words only one entry holds, which is where a wide vocabulary puts its memory.
- A word only one entry holds no longer costs an entry in the table of short
  fragments that a filter box searches, so a library of short, unrelated entries
  opens in a quarter less time and a quarter less memory: 30 000 entries with no
  repeated word open in 2.85 s and 697 MiB rather than 3.68 s and 765 MiB. A library
  that repeats most of its words pays up to 21 MiB for the change, and a filter box
  over one letter is faster on every library measured.
- A library written in any script searches as well as an English one: the filter
  box splits Greek, Cyrillic, Arabic and CJK text on what separates its words, and
  a Chinese title is matched by a run of its characters. The interface itself is
  English only, and that is a scope decision rather than a limit on what can be
  opened.
- Correct a typo without leaving the window. Every field in the detail pane is a
  text box, and a value is committed when the box loses focus, so what is saved is
  the text you finished typing rather than every keystroke on the way.
- A save writes only the values that were edited, and every other byte of the file
  stays as it was: the delimiters, the spacing around the `=`, the field order,
  the comments and every entry nobody touched. On a real 45 974-entry library,
  editing one title changed 44 bytes of 4 936 765.
- A value that cannot be written where it sits is refused when it is typed, not
  when it is saved: a brace that would close the entry early, or a word in a bare
  field, leaves the file alone and the reason appears under the fields.
- Editing a field costs 25 µs on a 45 974-entry library rather than the second a
  full reindex used to take, because the index is rebuilt before the next search
  rather than on every edit.
- Save carries a `*` while an edit is unsaved.
- A command line, a window and an empty library are now checked without a display:
  the detail pane is opened, typed into and read back from the accessibility tree,
  so an edit that stops reaching the window fails in a test rather than in a
  screenshot a person had to take.
