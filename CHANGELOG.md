# Changelog

What a user can do now that they could not before. Each release names the thing;
`docs/` explains the why.

## 0.2.0 - 22 October 2026

What a reader who filed papers from a PDF, or edited the same field across a
hundred entries, could not do before. Nothing a saved file carries has moved: an
edit still writes back only the bytes of the field it changed.

### Added

- **Open the paper from the entry.** A `file` field lists the files it names under
  the box; a press hands one to the desktop. JabRef, Zotero and Mendeley paths all
  read, including Mendeley's `$\backslash$` and a Windows drive.
- **A missing file is said where it was named.** The row reads `missing` with the
  path that was looked for, and nothing interrupts the window.
- **One field written across many entries.** `Shift`-click or `Shift`-arrows pick
  a range, `Ctrl+A` the screen; the pane becomes a box asking for a field and a
  value, and the batch is a single `Undo`.
- **An entry from a DOI or an arXiv id.** `Import` takes the id or the link it is
  shown at, shows what came back, and files it under a key no entry holds yet. A
  lookup that fails says why and leaves the library as it was.
- **Every word the window shows comes from one file.** `crates/app/src/catalogue.rs`
  is the whole interface vocabulary, and a check fails on a sentence written at the
  widget that draws it. The words are still English; a translation is now one file.

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
