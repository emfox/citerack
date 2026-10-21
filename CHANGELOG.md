# Changelog

What a reader of the release notes can now do. Anything that changed only inside
the repository is not listed here.

## Unreleased

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
  first keystroke, is answered as fast as a whole word. Typing three letters into
  a library of 30 000 entries with no repeated word used to take 13 ms and now
  takes 0.13 ms, and a single letter there used to take 56 ms and now takes 3.6 ms.
- `citerack --explain`, which prints how many rows each column holds for a query,
  so a search that returns too much or too little can be traced to a field.
