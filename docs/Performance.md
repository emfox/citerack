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
./target/release/citerack --index-report /tmp/bench/lib100k.bib
```

`scripts/genlib.py` writes the four generated libraries below, so a figure can be
re-derived from a clean checkout rather than trusted. They are not committed,
because together they are 140 MiB. The two real libraries live in
`docs/fixtures/games.zip` and §4 gives the command that unpacks them.

`--open-time` reports the wall time of reading, parsing and building the index:
everything the window waits for before it can draw a row. `--search` reports the
query alone, after the index exists. `--explain` prints how many rows each column
holds for each term of a query, which is how a search that returns too much or too
little is traced to a column. `--index-report` prints how many postings, runs and
words each column holds, which is how a library that opens slowly is traced to the
column and the structure responsible.

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
| 15 000 | 11.0 MiB | 0.26 s | 97 MiB | 17 us |
| 50 000 | 36.8 MiB | 0.85 s | 312 MiB | 17 us |
| 100 000 | 73.6 MiB | 1.78 s | 618 MiB | 18 us |
| 30 000, unique words | 11.9 MiB | 3.68 s | 765 MiB | 123 us |

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
| 15 000 | `Neural` | 11 317 | 0.11 ms |
| 50 000 | `Neural` | 37 516 | 0.36 ms |
| 100 000 | `Neural` | 75 092 | 0.71 ms |
| 15 000 | `pre` (a three-byte fragment) | 15 000 | 0.09 ms |
| 50 000 | `pre` | 50 000 | 0.27 ms |
| 100 000 | `pre` | 100 000 | 0.55 ms |
| 100 000 | `Sparse Sampling Manifold Privacy` | 30 653 | 4.38 ms |
| 100 000 | `a` (a one-byte fragment) | 100 000 | 3.36 ms |
| 100 000 | `zzzznotaword` | 0 | 0.04 ms |
| 100 000 | empty (matches all) | 100 000 | 0.26 ms |
| 30 000 unique | `pre` | 100 | 0.05 ms |
| 30 000 unique | `w` (a one-byte fragment) | 30 000 | 1.04 ms |
| 30 000 unique | `a` (a one-byte fragment) | 30 000 | 0.87 ms |

Four things are worth reading from this table.

**A query costs its own postings, not the library.** A term nobody wrote returns
after one hash lookup per column, whatever the size: `zzzznotaword` costs 0.04 ms
on a library of a hundred thousand entries. A query of four terms pays for the
postings of the rarest term first and only examines the rows that survive, which
is why `Sparse Sampling Manifold Privacy` costs six times `Neural` rather than a
hundred times.

**A term is looked up, not searched for.** The same `Neural` query went from
50.4 ms to 0.71 ms on the 100 000-entry library, a 71-fold saving. What remains is
proportional to the rows the query has to list: the answer to a query matching
two thirds of a library is still two thirds of a library, and no index avoids that.

**A fragment shorter than four bytes** is answered from a table of which words hold
each one to three byte run, so it is a lookup like any other term. `pre` names
100 000 rows in 0.55 ms; before that table existed the same query walked every word
the column had ever used and cost 13.4 ms on the unique-word library.

**A fragment in nearly every row** is the case the ladder cannot show, because its
columns hold forty words each. A real library holds a hundred thousand, and there
`a` reaches §4. The unique-word library also shows that `w` can cost more than `a`
(1.04 ms against 0.87 ms): what a short term costs is the number of words it sits
in, not how common it is, and `w` begins more of that library's invented words.

## 4. Whether the figures hold on another library

The question a benchmark has to answer is not what it measured but what it did not.
Four shapes are measured, and the two that come from real users disagree with the
two that are generated in a way worth naming.

| Library | Entries | Open | Memory | What it represents |
| --- | --- | --- | --- | --- |
| ladder, 100 000 | 100 000 | 1.78 s | 618 MiB | a library that repeats itself |
| unique, 30 000 | 30 000 | 3.68 s | 765 MiB | the worst case for a word table |
| `ARCADE` | 45 974 | 0.26 s | 98 MiB | a user's arcade-game catalogue |
| `console` | 134 352 | 0.70 s | 254 MiB | a user's console-game catalogue |

The first two rows come from `scripts/genlib.py` and anyone can redo them. The last
two are real libraries that a user attached to a bug report about slow scrolling.
The archive is committed at `docs/fixtures/games.zip`, with its digest and the
digests of its two members in `docs/fixtures/README.md`, so every figure below can
be redone from a clone:

```bash
unzip -o docs/fixtures/games.zip -d /tmp/fixtures
./target/release/citerack --open-time /tmp/fixtures/console.bib
```

`python3 scripts/fixtures.py check` fails if those bytes ever move, because a
library with a different digest is a different library and these figures would
stop describing it.

The real libraries open *faster* than the generated ones, because a real entry
carries fewer words than a generated abstract does. ARCADE opens in 0.26 s, and
console, at nearly three times its entry count, in 0.70 s: under a second for a
library larger than anything in the ladder. Read the generated ladder as the shape
that costs most per entry, not as the typical one.

**A short term on a real library was the worst case, and the ladder hid it.** The
console catalogue has 108 376 distinct citation keys, so a query for `a` has to
look at 58 591 words in the key column and 20 348 in the title column before it
knows which of 134 352 rows to list. That query cost 17.9 ms, inside the budget
only by luck. What made it slow was not the number of words but the cost of each:
every word cost two hash probes, one into a table of strings to find the word and
one into a table of postings to find its rows.

The walk now reads a flat array indexed by word id, with no string hashing at all,
and the same query costs 2.9 ms. Every other short-term query improved with it:

| Library | Query | Before | After |
| --- | --- | --- | --- |
| console, 134 352 | `a` | 17.9 ms | 2.9 ms |
| ARCADE, 45 974 | `a` | 5.4 ms | 0.88 ms |
| unique, 30 000 | `a` | 3.6 ms | 0.87 ms |
| unique, 30 000 | `pre` | 0.13 ms | 0.05 ms |

**A change that was measured, then removed.** An earlier version of this work kept,
for a short run in nearly every row, the list of rows that lack it, on the theory
that listing 576 absent rows beats reading 107 909 words. On the unique-word
library it did: `a` fell from 29 ms to 3.6 ms. On the real libraries it never
applied at all, because 61 589 absent rows are not cheaper than 58 591 words. Once
the flat array made a word cheap, the table bought 0.87 ms against 0.87 ms on one
library and nothing on the other two, while adding 0.19 s and 0.4 s to opening
them. It was deleted rather than kept for the case that flattered it.

**What depends on the library, and what does not.** The structure of the index does
not depend on how words are distributed. A term is looked up in a hash table of
four-byte runs or a table of words, and the same two tables answer any library, so
no shape of corpus makes a long term cheaper or dearer to find. What does change is
how much build work and memory the word table costs: 30 000 unique-word entries
take 3.68 s and 765 MiB where 100 000 repeating entries take 1.78 s and 618 MiB.

That dependency was a real defect and is now closed. Before the short-term tables a
three-byte term on the unique-word library cost 13.4 ms and a one-byte term 56.2 ms,
nearly three times the budget. `pre` now costs 0.05 ms there and `a` 0.87 ms, and
the console catalogue, four times larger and with far more distinct words, answers
`a` in 2.9 ms.

**The one cost that does not shrink is listing the answer.** A query that matches
every row must return every row, and a hundred thousand `u32`s cost 0.26 ms to
build even on the fast path the empty query takes. No index removes that; it is the
price of the result, not of the search.

## 5. What a high-entropy library would cost

Every library above, real or generated, repeats part of its vocabulary. A reviewer
asked the obvious next question: what if it does not, and what if real data looked
like that? The honest answer is that no real bibliography does, and then a number
for what would happen if one did.

**The shape is built, not found.** `scripts/widen.py` reads `console.bib` out of the
fixture archive and appends N new words to every entry's abstract, leaving the row
count and the entry shape untouched. Every added word is random, so every one is
unique: at N=20 the abstract column holds 2 687 040 distinct words for 134 352 rows,
and no word appears twice anywhere in the column.

| Words added per entry | Distinct words | Open | Peak memory | `a` |
| --- | --- | --- | --- | --- |
| 0 (the real library) | 144 122 | 0.75 s | 253 MiB | 3.0 ms |
| 2 | 412 826 | 2.15 s | 559 MiB | 3.8 ms |
| 5 | 815 882 | 4.41 s | 976 MiB | 4.0 ms |
| 10 | 1 524 000 | 8.01 s | 1 687 MiB | 4.6 ms |
| 20 | 2 831 162 | 16.78 s | 3 120 MiB | 5.3 ms |

```bash
python3 scripts/widen.py docs/fixtures/games.zip /tmp/widen console.bib 0 2 5 10 20
for f in /tmp/widen/*.bib; do ./target/release/citerack --open-time "$f"; done
```

**What regresses, and by how much.** Opening grows from 22 times the real library's
time and 12 times its memory at the twentieth additional word, and the growth is
close to linear in distinct words rather than in rows: the library is 3.6 times its
original size, but its vocabulary is 20 times larger and its opening time 22 times
longer. A query barely notices, from 3.0 ms to 5.3 ms, because a query costs the
words its own term names and not the words that exist.

**Where the memory goes.** `--index-report` names the column, and a build with the
gram table switched off names the structure: on the +20 library the abstract column
holds 26 519 575 postings, and 2 150 of the 3 120 MiB are the gram table, a
`(u64, word)` pair for every run of one to four bytes of every word. The four-byte
run table is only 183 MiB by comparison.

**Why real data cannot have this shape.** Counted on the same files, in words rather
than entries: the console catalogue runs 11.2 tokens per distinct word and ARCADE
9.5, so 91 percent and 89 percent of what they say has been said before. A library
whose every word is new sits at 1.0 by construction. Real bibliographies repeat
their venues, their authors, their publishers, their method names and their
citation keys, and even the console catalogue — auto-imported game metadata rather
than scholarly work — repeats nine words in ten. Two million distinct words would
need two million entries sharing no word with any other, which is not a
bibliography.

**The case that does happen is narrower and cheaper.** The console catalogue is 73
percent single-occurrence words: 105 749 of its 144 122 distinct words appear in
exactly one entry. That is the real version of this problem, and it is why the +20
row is worth publishing rather than dismissing: the direction is real even though
the magnitude is not.

## 6. What is not measured

| Unmeasured | Why it matters |
| --- | --- |
| Time to first frame | it needs a frame-timing harness on a real display; the window is laid out before the file is read, but "before" is a design claim, not a figure |
| Scroll smoothness at 100 000 rows | the table is virtualised, so the frame cost should not depend on row count, but a claim of that kind needs a frame trace |
| Save time on 150 MiB | save is a single write of unchanged bytes and is fast by construction, but it has not been timed |
| A library below a thousand entries | every figure here is a large library; a small one opens in milliseconds, which is fast enough to make the number uninteresting rather than unknown |
