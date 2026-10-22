# Architecture

citerack exists because a bibliography tool should not make its user wait for
the file to be the size of a book. The architecture is one long argument about
where the waiting goes: out of the window, and into a stage the user can see and
interrupt.

## 1. Shape

```
crates/bib     parse, model, write          no I/O, no UI, no threads
crates/index   fold, match, search, sort    reads a Library, owns no state
crates/store   Session: open, search, save  the only crate that touches disk
crates/app     window and CLI               the only crate that knows a screen exists
```

Dependencies point one way: `app` → `store` → `index` → `bib`. Nothing lower
knows anything about a layer above it, so the engine can be timed, tested and
benchmarked without a display, and `cargo test -p citerack-store` covers the path
a click takes.

The four crates are separate rather than one binary with modules because the
boundary is the thing worth defending. A `use eframe` that crept into `store`
would be a compile error rather than a review comment.

## 2. Why the parser keeps the file

`citerack-bib` holds three things a rewrite depends on:

| Kept | Why |
| --- | --- |
| The source text | rewriting an unchanged library has to produce an identical file |
| Every entry's byte span | the editor scrolls to a row, and needs to know where its bytes are |
| Every skipped block's span | `@comment`, `@preamble` and `@string` are not entries, but losing them loses the file |

A field this crate does not model is not a problem, because it does not model
fields at all: an `Entry` is a type, a key, and the fields in file order, each
holding the text the file spelled and how it was delimited. Nothing is
normalised, nothing is dropped, and no field name is special-cased in the parser.

The consequence, and the reason for the design, is that `write(Preserve)` on an
unmodified library is byte-identical to the input. That is the property a user
notices when they open a file in two tools and one of them reformats it: the
diff is noise, and the history is ruined. A test asserts it directly rather than
asserting on a parse tree that could hide the loss.

Parsing is a single pass over bytes. A `String` is only materialised per field,
after its span is known, so a 150 MiB file does not become a 150 MiB `Vec<char>`.
Field names and the entry type keep the spelling the file used, so a rewrite
reproduces `@Article` as `@Article` and `TITLE` as `TITLE`.

## 3. Why search is a column store

`Searcher::build` folds every searched field of every entry once, into one column
per field, with `None` where an entry lacks it. A query then walks the columns
and touches each byte once. Folding at build time is what makes a query cheap:
case, accents and the TeX escape `{\"u}` are all already resolved when the user
types.

This is a deliberate trade. Memory roughly doubles, because the folded text is
the text again. In exchange, a query never allocates a folded copy and never
re-folds a field twice.

`Matcher` requires every term to appear somewhere in the entry, in any order,
which is what a user expects from a single search box. A term may match a
different field from the term next to it: `zhou 2024` finds the entry whose
author is Zhou and whose year is 2024 without the user knowing the field names.

### 3.1 What used to be slow

The first query against 100 000 entries used to take about 50 ms, because it
scanned `n × fields` cells and folded each one. A search is answered from an
inverted index over the folded columns now, and the same query costs 0.72 ms;
`docs/Performance.md` §3 has the figures and the command that re-derives them.

## 4. Why the window is built after the data

`Session::open` runs on a worker thread and the window is laid out before the
thread returns. The status line shows a spinner, the table shows nothing, and the
window is responsive while the file is still being read. A front end that opens
the file on the UI thread is the failure this design exists to avoid: the user
sees a white rectangle, cannot tell whether the program is working, and has no
button to cancel it.

The measurement that motivated it, and the numbers this crate is held to, are in
`docs/Performance.md`.

## 5. Why sorting is precomputed

`SortKey::of` computes the four values a view sorts on — surname, year as a
number, title without its leading article, folded key — once per entry. Sorting
then reorders a `Vec<usize>` of row indices and touches no entry text, so a sort
is a permutation rather than a re-read.

Keeping the display order as a separate vector of indices, rather than reordering
the entries themselves, is what lets filter and sort compose: `search` replaces
the vector, `sort` reorders whatever is in it, and the library under both never
moves. It is also what makes an entry's identity stable across a sort, which the
selection and the detail pane depend on.

## 6. What the front end may and may not do

- It may read a `Session` and draw from it.
- It may not parse, fold, sort or match. Every one of those is a call into a
  crate that can be tested without a window.
- It may not hold text from a library for longer than a frame. A cell is
  rendered from a borrowed `&Entry`, so a large library does not also become a
  large view model.

The one thing the front end owns is a pending query. A keystroke does not run a
search: it records a time, and 120 ms of quiet later the search runs. Without
that, typing a word runs the engine once per character, and a query is a query
however few characters asked for it.

## 7. Why an attachment is read as text, then opened by the desktop

The `file` field has no definition in BibTeX and three writers in practice, and
they disagree: JabRef separates with `:\`, Zotero writes a MIME type where JabRef
writes `PDF`, Mendeley leaves `$\backslash$` where a backslash belongs. Reading
them is a text problem, so `citerack-bib` answers it — `parse_file_field` splits
and unescapes, and does not touch a disk, which is what makes it testable as a
transformation over strings rather than with a fixture folder.

Resolving is a separate step, in `app`, because that is where the library's own
path is known: a relative path is counted from beside the library, which is the
only base a reader can be sure a manager meant. A path that resolves to nothing
does not become a dialog. It becomes a row that says `missing` under the field
that named it, and the window is never interrupted to report it.

Opening hands the path to the platform's own opener — `xdg-open`, `open`,
`cmd /c start`. That is deliberate rather than lazy: linking a toolkit to show a
PDF would add libraries to a binary whose whole claim is that it links three, and
a reader already has a viewer they like. The same reasoning is why the roadmap
refuses to render a PDF in the window.

## 8. Portability

`eframe` with `glow` and no default features is the smallest window that runs on
X11, Wayland and the three desktop platforms without a system toolkit of its
own. The file dialog is `rfd`, which speaks to the desktop portal rather than
linking GTK. No crate in the workspace names a platform outside `app`'s
dependency list.

The headless path — `citerack --open-time` and `citerack --search` — is not a
debugging aid bolted on. It is how CI measures the engine, how a bug report
quotes a number, and how this document's claims can be re-derived on a machine
with no display at all.
