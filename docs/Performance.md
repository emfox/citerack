# Performance

Every figure here names the machine, the build and the command that produced it,
because a number nobody can re-derive is a claim rather than a measurement. The
command is always the headless one, so a reader with no display can check it.

## 1. How to reproduce

```bash
cargo build --release
./target/release/citerack --open-time path/to/library.bib
./target/release/citerack --search "some query" path/to/library.bib
./target/release/citerack --explain "some query" path/to/library.bib
```

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
`rustc 1.93.0`. The libraries are generated BibTeX with uniform field sets and a
repeating abstract, which is the shape a real library of this size has.

| Entries | Size | Open, whole chain | Peak memory | Per entry |
| --- | --- | --- | --- | --- |
| 15 000 | 22.9 MiB | 0.31 s | 152 MiB | 21 us |
| 50 000 | 75.7 MiB | 1.06 s | 486 MiB | 21 us |
| 100 000 | 150.6 MiB | 2.09 s | 960 MiB | 21 us |

The per-entry cost is flat, which is what makes the figure usable: a library twice
the size costs twice the time, so 200 000 entries is about four seconds and can be
predicted rather than measured.

Building the index costs about a quarter of the opening time and most of the
memory. On the 100 000-entry library, parsing is 0.31 s and folding every field is
1.02 s, which leaves roughly 0.75 s for the index itself. The memory is the folded
text (135 MiB) plus one 4-byte posting per word per row plus one per run of four
bytes inside each word.

**Repeats within a cell are collapsed before they reach the index.** An abstract
of 244 words drawn from a 102-word vocabulary names its row 244 times if every
occurrence is written, and every repeat is thrown away by the dedup afterwards.
Skipping a word a row has already contributed cut the opening time of the
100 000-entry library from 4.23 s to 2.14 s and its peak memory from 1302 MiB to
960 MiB, with no change to any answer the index gives. Real bibliographies repeat
their vocabulary more than this synthetic one does, so the saving there should be
larger.

## 3. Searching

Best of three runs each, after the index exists. Quote includes the process's
start-up, which is why a query matching nothing is not 0.02 ms even though the
lookup is.

| Entries | Query | Matches | Query time |
| --- | --- | --- | --- |
| 15 000 | `Neural` | 9 562 | 0.23 ms |
| 50 000 | `Neural` | 31 737 | 0.73 ms |
| 100 000 | `Neural` | 63 206 | 1.50 ms |
| 15 000 | `Sparse Sampling Manifold Privacy` | 7 457 | 0.91 ms |
| 50 000 | `Sparse Sampling Manifold Privacy` | 24 701 | 2.95 ms |
| 100 000 | `Sparse Sampling Manifold Privacy` | 49 087 | 5.95 ms |
| 100 000 | `ide` (a word fragment) | 6 709 | 4.02 ms |
| 100 000 | `zzzznotaword` | 0 | 0.02 ms |
| 100 000 | empty (matches all) | 100 000 | 0.28 ms |

Two things are worth reading from this table.

**A query costs its own postings, not the library.** A term nobody wrote returns
after one hash lookup per column, whatever the size: `zzzznotaword` costs 0.02 ms
on a library of a hundred thousand entries. A query of four terms pays for the
postings of the rarest term first and only examines the rows that survive, which
is why `Sparse Sampling Manifold Privacy` costs five times `Neural` rather than a
hundred times.

**A search is now constant-time in the library and linear in the matches.** The
same `Neural` query went from 50.4 ms to 1.50 ms on the 100 000-entry library, a
33-fold saving, because a term is looked up rather than searched for. What remains
is proportional to the rows the query has to list: the answer to a query matching
two thirds of a library is still two thirds of a library, and no index avoids that.

**A fragment shorter than four bytes cannot be looked up exactly** and is answered
from the word list instead, which is why `ide` costs more than a whole word at the
same match count. This is the one remaining cost that follows the vocabulary
rather than the hits, and it is 4 ms at 100 000 entries.

The count `9 562` for `Neural` on the 15 000-entry library was confirmed against
an unrelated BibTeX reader run on the same file with the same query, which
reported the same number. A count that agrees with nobody is a count worth
checking; this one was. Every count in this table is also checked against a full
scan of every cell by a test that runs on both the sample library and a generated
one of 20 000 entries.

## 4. What is not measured

| Unmeasured | Why it matters |
| --- | --- |
| Time to first frame | it needs a frame-timing harness on a real display; the window is laid out before the file is read, but "before" is a design claim, not a figure |
| Scroll smoothness at 100 000 rows | the table is virtualised, so the frame cost should not depend on row count, but a claim of that kind needs a frame trace |
| Save time on 150 MiB | save is a single write of unchanged bytes and is fast by construction, but it has not been timed |
| A real library of 100 000 entries | the benchmark libraries are generated with a small vocabulary; a real one with long unique abstracts and DOIs would produce more postings per row, and the opening figures here are the optimistic end |
