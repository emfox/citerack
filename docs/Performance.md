# Performance

Every figure here names the machine, the build and the command that produced it,
because a number nobody can re-derive is a claim rather than a measurement. The
command is always the headless one, so a reader with no display can check it.

## 1. How to reproduce

```bash
cargo build --release
python3 scripts/genlib.py /tmp/bench
./target/release/citerack --open-time /tmp/bench/lib100k.bib
./target/release/citerack --search "Neural" /tmp/bench/lib100k.bib
./target/release/citerack --explain "Neural" /tmp/bench/lib100k.bib
```

`scripts/genlib.py` writes the four libraries every figure below is measured on,
so a figure can be re-derived from a clean checkout rather than trusted. They are
not committed, because together they are 140 MiB.

`--open-time` reports the wall time of reading, parsing and building the index:
everything the window waits for before it can draw a row. `--search` reports the
query alone, after the index exists. `--explain` prints how many rows each column
holds for each term of a query, which is how a search that returns too much or too
little is traced to a column.

The budget tests run the same path and fail when a limit is crossed:

```bash
cargo test -p citerack-store --release --test budget -- --nocapture
```

## 2. Opening

Measured on Linux x86-64, one core of a shared virtual machine, a release build,
`rustc 1.93.0`. The ladder is generated BibTeX with uniform field sets and a
repeating abstract, which is the shape a real library of this size has; the fourth
row has no repeated word at all.

| Entries | Size | Open, whole chain | Peak memory | Per entry |
| --- | --- | --- | --- | --- |
| 15 000 | 11.0 MiB | 0.28 s | 95 MiB | 19 us |
| 50 000 | 36.8 MiB | 0.98 s | 305 MiB | 20 us |
| 100 000 | 73.6 MiB | 2.01 s | 606 MiB | 20 us |
| 30 000, unique words | 11.9 MiB | 3.78 s | 766 MiB | 126 us |

The first three rows have a flat per-entry cost, which is what makes the figure
usable: a library twice the size costs twice the time, so 200 000 entries is about
four seconds and can be predicted rather than measured.

**The fourth row is why the per-entry figure is not a promise.** It is a quarter
the size of the 100 000-entry library and opens slower, because opening is paid
per distinct word and not per entry. A library whose every abstract is new text
has as many words as it has words, and each new word must be entered into the word
table, into every four-byte run of itself, and into the table of short runs. A real
bibliography sits between the two shapes: it repeats its venues, its authors and
much of its vocabulary, but not its abstracts.

**Repeats within a cell are collapsed before they reach the index.** An abstract
of 244 words drawn from a 102-word vocabulary names its row 244 times if every
occurrence is written, and every repeat is thrown away by the dedup afterwards.
Skipping a word a row has already contributed removes that work entirely, and no
answer the index gives depends on it.

## 3. Searching

Best of five runs each, after the index exists. Quote includes the process's
start-up, which is why a query matching nothing still costs something.

| Entries | Query | Matches | Query time |
| --- | --- | --- | --- |
| 15 000 | `Neural` | 11 317 | 0.12 ms |
| 50 000 | `Neural` | 37 516 | 0.38 ms |
| 100 000 | `Neural` | 75 092 | 0.73 ms |
| 15 000 | `pre` (a three-byte fragment) | 15 000 | 0.10 ms |
| 50 000 | `pre` | 50 000 | 0.31 ms |
| 100 000 | `pre` | 100 000 | 0.61 ms |
| 100 000 | `Sparse Sampling Manifold Privacy` | 30 653 | 4.26 ms |
| 100 000 | `a` (a one-byte fragment) | 100 000 | 3.77 ms |
| 100 000 | `zzzznotaword` | 0 | 0.04 ms |
| 100 000 | empty (matches all) | 100 000 | 0.27 ms |
| 30 000 unique | `pre` | 100 | 0.13 ms |
| 30 000 unique | `w` (a one-byte fragment) | 30 000 | 0.12 ms |
| 30 000 unique | `a` (a one-byte fragment) | 30 000 | 3.60 ms |

Three things are worth reading from this table.

**A query costs its own postings, not the library.** A term nobody wrote returns
after one hash lookup per column, whatever the size: `zzzznotaword` costs 0.04 ms
on a library of a hundred thousand entries. A query of four terms pays for the
postings of the rarest term first and only examines the rows that survive, which
is why `Sparse Sampling Manifold Privacy` costs six times `Neural` rather than a
hundred times.

**A term is looked up, not searched for.** The same `Neural` query went from
50.4 ms to 0.73 ms on the 100 000-entry library, a 69-fold saving. What remains is
proportional to the rows the query has to list: the answer to a query matching
two thirds of a library is still two thirds of a library, and no index avoids that.

**A fragment shorter than four bytes** is answered from a table of which words hold
each one to three byte run, so it is a lookup like any other term. `pre` names
100 000 rows in 0.61 ms; before that table existed the same query walked every word
the column had ever used and cost 13.4 ms on the unique-word library.

**A fragment in nearly every row is answered backwards.** Walking the words that
hold `a` means reading the postings of a hundred thousand words on the unique-word
library, and some rows still have no `a` in them, so the walk cannot stop early.
The index therefore keeps, for a run that sits in most rows, the short list of rows
that lack it: 576 of them on the unique-word library. `a` costs 3.6 ms there rather
than the 29 ms the forward walk cost, and the case never exceeds the 20 ms budget.
The two directions are checked against each other by a test that compares a term in
most rows against a full scan.

## 4. Whether the figures hold on another library

The question a benchmark has to answer is not what it measured but what it did not.
Four shapes are measured, and they disagree with each other in a way worth naming.

| Library | Entries | Open | Memory | What it represents |
| --- | --- | --- | --- | --- |
| ladder, 100 000 | 100 000 | 2.01 s | 606 MiB | a library that repeats itself |
| unique, 30 000 | 30 000 | 3.78 s | 766 MiB | the worst case for a word table |
| ARCADE | 45 974 | 0.23 s | 98 MiB | real bibliographies |
| console | 134 352 | 0.64 s | 254 MiB | a large real library |

The first two rows come from `scripts/genlib.py` and anyone can redo them. The last
two are real libraries that belong to no repository, so those two numbers can only
be re-derived by whoever still holds the files; they are here as a reference point
outside the generated shapes, not as figures a reader can check.

The real libraries open *faster* than the generated ones, because a real entry
carries fewer words than a generated abstract does. ARCADE opens in 0.23 s, and
console, at nearly three times its entry count, in 0.64 s: under a second for a
library larger than anything in the ladder. Read the generated ladder as the shape
that costs most per entry, not as the typical one.

**What depends on the library, and what does not.** The structure of the index does
not depend on how words are distributed. A term is looked up in a hash table of
four-byte runs, a table of whole words, or a table of short runs, and the same
three tables answer any library, so no shape of corpus can make a long term
cheaper or dearer to find. What does change is how much build work and memory the
word table costs: 30 000 unique-word entries take 3.78 s and 766 MiB where 100 000
repeating entries take 2.01 s and 606 MiB.

That dependency was a real defect and is now closed. On the unique-word library a
three-byte term used to walk the whole vocabulary and cost 13.4 ms, and a one-byte
term cost 56.2 ms, nearly three times the budget. Short terms are now answered by
two tables that do not grow with the vocabulary in the way the walk did: a list of
the words each run sits in, ordered so a query that fills every row stops early,
and a list of the rows a common run is absent from. `pre` fell from 13.4 ms to
0.13 ms, a hundredfold, and `a` from 56.2 ms to 3.6 ms.

**The one cost that does not shrink is listing the answer.** A query that matches
every row must return every row, and a hundred thousand `u32`s cost 0.27 ms to
build even on the fast path the empty query takes. No index removes that; it is the
price of the result, not of the search.

## 5. What is not measured

| Unmeasured | Why it matters |
| --- | --- |
| Time to first frame | it needs a frame-timing harness on a real display; the window is laid out before the file is read, but "before" is a design claim, not a figure |
| Scroll smoothness at 100 000 rows | the table is virtualised, so the frame cost should not depend on row count, but a claim of that kind needs a frame trace |
| Save time on 150 MiB | save is a single write of unchanged bytes and is fast by construction, but it has not been timed |
| A library smaller than 10 000 entries | the table of absent rows is only built above that size, so a small library answers a common short term by walking its words; the walk is cheap there because the vocabulary is small, but the figure has not been taken |
