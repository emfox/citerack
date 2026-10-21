# citerack

A reference manager for people whose bibliography outgrew their tool. It opens a
BibTeX file of a hundred thousand entries in about two seconds, and it never
rewrites a byte it did not change.

The problem it addresses is narrow and concrete: a library that has grown for
twenty years is a file that a general-purpose tool parses on the thread that
draws the window, so the window is a white rectangle for as long as the parse
takes, and the tool has no way to tell the user which of the two is the problem.
citerack separates those two: the file is read on a worker thread, the index is
built before the first row is drawn, and the window reports both.

## What it does today

| | |
| --- | --- |
| Reads | BibTeX, including fields it does not model, which survive a save |
| Saves | byte-identically when nothing was edited |
| Searches | case- and accent-insensitive, over every field, in about 7 ms at 15 000 entries |
| Sorts | key, first author, year, title, without re-reading an entry |
| Shows | a table with key, type, author, title, year and venue, and a detail pane per entry |

Editing, and everything downstream of it, is on the [roadmap](docs/Roadmap.md).

## Install

Every tagged release publishes a Linux binary and a Windows one, each with a
`.sha256` beside it. From a checkout instead:

```bash
cargo build --release
./target/release/citerack library.bib
```

On Windows the build targets MSVC with the runtime linked statically, so the exe
needs no redistributable. It has no signature, so Windows may ask before running
it; the checksum is how to tell it apart from anything else with that name.

Linux needs the usual window-development packages: `libxkbcommon-x11-dev` and
`libfontconfig1-dev` on Debian and Ubuntu. The build has no other system
dependency.

A window is drawn with OpenGL and, on a machine that will not give it a GL
context, with Vulkan or DX12 instead, so a broken GL driver is not a reason to
refuse to start. On Linux that second path needs a Vulkan loader, usually already
present as `libvulkan1`.

## Use

```bash
citerack library.bib                  # open a window
citerack                              # open a window, then pick a file
citerack --open-time library.bib      # parse and index, print the timings
citerack --search "zhou 2024" library.bib
```

`--open-time` and `--search` need no display, which is how the figures in
[docs/Performance.md](docs/Performance.md) are produced and how a bug report can
quote a number that means something.

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
