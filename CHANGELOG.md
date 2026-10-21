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
