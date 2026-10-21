# Performance

Every figure here names the machine, the build and the command that produced it,
because a number nobody can re-derive is a claim rather than a measurement. The
command is always the headless one, so a reader with no display can check it.

## 1. How to reproduce

```bash
cargo build --release
./target/release/citerack --open-time path/to/library.bib
./target/release/citerack --search "some query" path/to/library.bib
```

`--open-time` reports the wall time of reading, parsing and building the index:
everything the window waits for before it can draw a row. `--search` reports the
query alone, after the index exists.

The budget tests run the same path and fail when a limit is crossed:

```bash
cargo test -p citerack-store --release --test budget -- --nocapture
```

## 2. Opening

Measured on Linux x86-64, one core of a shared virtual machine, a release build,
`rustc 1.93.0`. The libraries are generated BibTeX with uniform field sets and a
repeating abstract, which is the shape a real library of this size has.

| Entries | Size | Open, whole chain | Per entry |
| --- | --- | --- | --- |
| 15 000 | 22.9 MiB | 0.29 s | 19 us |
| 50 000 | 75.7 MiB | 0.99 s | 20 us |
| 100 000 | 150.6 MiB | 2.03 s | 20 us |
| 134 352 (real file, mostly bare fields) | 13.3 MiB | 0.29 s | 2 us |

The per-entry cost is flat across the first three rows, which is what makes the
figure usable: a library twice the size costs twice the time, so 200 000 entries
is about four seconds and can be predicted rather than measured. The fourth row
is a real file whose entries are a tenth the size, and it shows that the cost
follows the bytes rather than the entry count.

A single parse of 50 000 entries among these takes about 0.10 s of the 0.99 s;
the remainder is folding every field for the index and reading the file.

## 3. Searching

| Entries | Query | Matches | Query time |
| --- | --- | --- | --- |
| 15 000 | `Neural` | 9 562 | 7.3 ms |
| 50 000 | `Neural` | 31 737 | 26.7 ms |
| 100 000 | `Neural` | 63 206 | 50.4 ms |
| 100 000 | empty (matches all) | 100 000 | 7.4 ms |

Two things are worth reading from this table.

**The cost follows the matches, not the rows.** A query that matches nothing
returns in under 3 ms regardless of size, because the scan stops at the first
term that misses. A query that matches two thirds of the library pays for every
row it reports. An inverted index would not change this, since the matches still
have to be listed; it would change the term lookup that today costs a memmem
call per cell.

**A search is not yet constant-time.** The roadmap carries the item that makes a
rare term cheap. Until it lands, the honest statement is: instant up to about
50 000 entries, perceptible above 100 000.

The count `9 562` for `Neural` on the 15 000-entry library was confirmed against
an unrelated BibTeX reader run on the same file with the same query, which
reported the same number. A count that agrees with nobody is a count worth
checking; this one was.

## 4. What is not measured

| Unmeasured | Why it matters |
| --- | --- |
| Time to first frame | it needs a frame-timing harness on a real display; the window is laid out before the file is read, but "before" is a design claim, not a figure |
| Scroll smoothness at 100 000 rows | the table is virtualised, so the frame cost should not depend on row count, but a claim of that kind needs a frame trace |
| Save time on 150 MiB | save is a single write of unchanged bytes and is fast by construction, but it has not been timed |
| Memory at 150 MiB input | the folded columns roughly double the text's size; the multiplier has not been measured |
