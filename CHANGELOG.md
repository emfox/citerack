# Changelog

What a user can do now that they could not before. Each release names the thing;
`docs/` explains the why.

## 0.1.1 - 22 October 2026

### Added

- **Light, dark or follow the desktop.** Three words at the right end of the
  toolbar choose it; the window opens on `System`, and a machine that reports no
  preference is drawn light.
- **Generate citation keys from fields.** A pattern such as `{author}{year}` is
  previewed in a `Generated` column, duplicates take a letter, and one press writes
  them all. Pressing twice changes nothing; `Undo` takes it back.
- **Required fields are marked.** The detail pane names a field the entry's type
  needs in red, says `missing` or `empty`, and adds it when you write in the empty
  box.
- **Groups panel.** Filter by author, journal, year or entry type. A pressed value
  stays listed as `(let go)`, `Clear` releases all of them, and counts are what the
  filter would actually leave.

## 0.1.0 - 21 October 2026

The first release: a library opens in a window, a filter box narrows it, and an
edited field is written back without moving a byte the edit did not touch.

### Added

- **Open and browse** a BibTeX library: filter box, sortable table of key, type,
  author, title, year and venue, detail pane, year facet.
- **Search** every field at once, ignoring case and accents: 4 ms on 100 000
  entries, 2.9 ms after one letter on 134 352.
- **Edit** any field in place, each keystroke 25 µs on 45 974 entries.
- **Save byte for byte**: editing one title changed 44 bytes of 4 936 765, and an
  entry nobody touched survives a round trip as it came.
- **Refuse** a value that cannot be written where it sits — a stray brace, a word
  in a bare field — when it is typed, with the reason under the fields.
- **Undo** with `Ctrl+Z` / `Ctrl+Shift+Z`; a run of typing undoes as one step.
- **Find duplicates** by DOI, or by title and year with case, accents, braces and
  punctuation folded away.
- **Keyboard only**: `Up`/`Down` walk the table, `Ctrl+D` the detail pane,
  `Ctrl+O`/`S`/`F` the rest.
- **Search non-Latin scripts** — Greek, Cyrillic, Arabic and CJK split on their own
  word boundaries; a Chinese title matches by a run of characters.
- **Runs where other tools do not**: if the GL driver is broken the same process
  asks `wgpu` for the window instead of refusing to open.
- **A tagged release** publishes a Linux binary and a Windows one, each with a
  checksum beside it; the Windows build needs no redistributable.
- **Measure without a display**: `--open-time`, `--search`, `--edit-time`,
  `--save-time`, `--explain` and `--index-report`, which reproduce the figures
  above on a machine of your own.
