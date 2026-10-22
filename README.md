# citerack

A BibTeX editor for a `.bib` that has outgrown its tool. It opens a hundred
thousand entries in about 2.6 s, and it never rewrites a byte it did not change.

## Is this for you?

Four questions. Three yeses is a good sign; two is probably enough.

**1. Is your library large?** The figures this tool exists for are the ones past
fifty thousand entries. Below five thousand, JabRef is richer than this and
already fast enough for you.

**2. Is the `.bib` file the library, or a copy of it?** citerack edits the file
directly and keeps no database — no sidecar, no index file, no import step. If
you edit in a manager that owns a database and export when you compile, this is
the wrong shape of tool for you.

**3. Do you want the file in version control?** A save here writes back only the
bytes that changed: editing one title in a 45 974-entry library touched 44 of
4 936 765 bytes. `git diff` on a `.bib` stays readable. This is the design that
the rest follows from.

**4. Do you only need the BibTeX file?** There is no PDF library here — a `file`
field is opened, not managed — no Word or LibreOffice plugin, no sync, no cloud,
no browser connector. If those are the reasons you use what you use now, they
are good reasons and this does not replace them.

And three things that are true of this build today: the interface is English
only, nothing here has been through years of unusual files (see [Where this gets
it wrong](#where-this-gets-it-wrong)), and it is a one-person project that is
weeks old.

## Where the time goes

`citerack --open-time` prints the input size beside the timings, so the number
means something:

```
$ citerack --open-time your.bib
file        your.bib
size        73.60 MiB
entries     100000
open_total  1.833 s
```

(That output is a generated 100 000-entry file on the machine that built 0.2.2;
a busier core reads the same file in 2.56 s. Compare per-entry cost, not the
total.)

If you have a library that feels slow — in this or in anything else — that output
is the most useful thing you can send. The figures, the machine they were taken
on and the command that re-derives each one are in
[docs/Performance.md](docs/Performance.md).

## What it does today

![The window on a library of 15 000 entries](docs/images/window.png)

| | |
| --- | --- |
| Reads | BibTeX, including fields it does not model, which survive a save |
| Edits | any field in place, with `Ctrl+Z` over a run of typing; a field the entry's type needs but the file lacks is named and added |
| Saves | byte-identically when nothing was edited |
| Searches | case- and accent-insensitive, over every field, in about 0.16 ms at 15 000 entries and 1.0 ms at 100 000 for a word; a four-word query costs a few milliseconds |
| Filters | by author, journal, year and type, with a count that is what the filter would actually leave |
| Sorts | key, first author, year, title, without re-reading an entry |
| Names | a duplicate by DOI, or by title and year with case, accents, braces and punctuation folded away |
| Keys | generated from a pattern such as `{author}{year}`, previewed before the file moves |
| Opens | a paper from a `file` field with one press, whoever wrote the path; one that is not there is marked beside the field |
| Batches | one field written across a picked range with a typed name and value, as one undo step |
| Imports | an entry from a DOI or an arXiv id, typed or pasted as a link, filed under a key nothing else holds |
| Speaks | from one catalogue, so the window is one file away from a second language |
| Shows | a table with key, type, author, title, year and venue, and a detail pane per entry |

A real user's 134 352-entry library, attached to a bug report about slow
scrolling elsewhere, opens in about two thirds of a second:

![A library of 134 352 entries, open in 0.63 s](docs/images/large-library.png)

Bulk editing, importing from a DOI, attachments and the catalogue that a
translation would be written against all shipped in 0.2. Nothing is open on the
[roadmap](docs/Roadmap.md), which keeps the ideas that were decided against so the
same question is not reopened.

## Where this gets it wrong

Kept here rather than in a changelog, because it is the part a stranger should
read before trusting a number.

- **It was built against two shapes of library, not twenty years of habits.**
  Everything measured comes from generated files plus two real libraries a user
  attached to a bug report elsewhere — 45 974 and 134 352 entries, committed at
  `docs/fixtures/games.zip`. Generated files have uniform fields. Yours probably
  has a decade of exceptions. Expect to be the one who finds them.
- **It is conservative about what it does not understand.** A construct the
  parser does not recognise is read as text and written back unchanged; the file
  is never silently normalised. The cost is that some files it will refuse to
  make a change to rather than guess.
- **It measures itself, and the measurements are on one machine.** The absolute
  seconds in `docs/Performance.md` come from one shared virtual core. Compare
  per-entry cost across machines, not total seconds.
- **The interface is English only.** The strings live in one catalogue and a
  second language is a file, not a rewrite, but only English exists.
- **It is a one-person project.** No support contract, no funders, no release
  cadence. Issues are answered when they are answered.

## Install

Every tagged release publishes a Linux binary and a Windows one, each with a
`.sha256` beside it. The newest is
[`v0.2.2`](https://github.com/emfox/citerack/releases/latest). Download it and run
it; on a Linux desktop nothing has to be installed first, because the binary links
only `libc`, `libm` and `libgcc_s` and loads the window stack — X11, Wayland, GL,
Vulkan — from whatever the desktop already has.

From a checkout instead. Beyond a Rust toolchain and a C linker, nothing has to be
installed: every window library the build touches — X11, Wayland, xkbcommon, GL,
EGL — is reached through a `dlopen` feature, so nothing is probed or linked while
compiling.

```bash
cargo build --release -p citerack
./target/release/citerack library.bib
```

The binary that comes out links `libc`, `libm` and `libgcc_s` and nothing else;
`scripts/check_linux_links.sh` fails the build if that ever stops being true.

On Windows the build targets MSVC with the runtime linked statically, so the exe
needs no redistributable. It has no signature, so Windows may ask before running
it; the checksum is how to tell it apart from anything else with that name.

A window is drawn with OpenGL and, on a machine that will not give it a GL
context, with Vulkan or DX12 instead, so a broken GL driver is not a reason to
refuse to start. On Linux that second path needs a Vulkan loader, usually already
present as `libvulkan1`.

Nothing has to be installed beside the binaries: the Windows exe binds only
libraries that every Windows has. The shader compiler the DX12 path needs
is taken from the system directory if it is there, and the path is left out if it
is not.

## Use

```bash
citerack library.bib                  # open a window
citerack                              # open a window, then pick a file
citerack --open-time library.bib      # parse and index, print the timings
citerack --search "zhou 2024" library.bib
```

The measuring flags need no display, which is how the figures in
[docs/Performance.md](docs/Performance.md) are produced and how a bug report can
quote a number that means something: `--open-time`, `--edit-time`, `--save-time`,
`--search`, `--explain` and `--index-report`. The last two say where a query spent
its rows, and where the index spent its memory.

In the window: `Ctrl+O` opens a library, `Ctrl+S` saves it, `Ctrl+F` reaches the
filter box, and `Ctrl+Z` and `Ctrl+Shift+Z` take back and put back an edit.
`Up` and `Down` walk the table, `Ctrl+D` reaches the fields of the selected entry,
and `Tab` walks them. Nothing needs a mouse.

`System`, `Dark` and `Light` at the right end of the toolbar choose the theme; the
window opens following the desktop.

## How it is built

Four crates, one dependency direction:

```
bib  ->  index  ->  store  ->  app
```

`bib` parses and writes. `index` folds, matches and sorts. `store` owns the
`Session` and is the only crate that touches disk. `app` is the only crate that
knows a screen exists. Everything below `app` runs headless, so the path a click
takes is the path the tests take.

[docs/Architecture.md](docs/Architecture.md) explains why each boundary is where
it is; [docs/Ui-spec.md](docs/Ui-spec.md) says what the window shows and why.

## Contribute

```bash
sh scripts/install-hooks.sh      # once per clone
python3 scripts/check_all.py     # every gate this machine can run
```

Commit messages, comment density and the English-only rule for source are held by
scripts rather than by memory; [docs/Conventions.md](docs/Conventions.md) states
them and `scripts/check_all.py` enforces them.
