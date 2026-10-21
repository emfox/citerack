# Performance

Every figure here names the machine, the build and the command that produced it,
because a number nobody can re-derive is a claim rather than a measurement. The
command is always the headless one, so a reader with no display can check it.

## 1. How to reproduce

```bash
cargo build --release
python3 scripts/genlib.py /tmp/bench
./target/release/citerack --open-time /tmp/bench/lib100k.bib
./target/release/citerack --edit-time /tmp/bench/lib100k.bib
./target/release/citerack --save-time /tmp/bench/lib100k.bib
./target/release/citerack --search "Neural" /tmp/bench/lib100k.bib
./target/release/citerack --explain "Neural" /tmp/bench/lib100k.bib
./target/release/citerack --index-report /tmp/bench/lib100k.bib
```

`scripts/genlib.py` writes the four generated libraries below, so a figure can be
re-derived from a clean checkout rather than trusted. They are not committed,
because together they are 140 MiB. The two real libraries live in
`docs/fixtures/games.zip` and §5 gives the command that unpacks them.

`--open-time` reports the wall time of reading, parsing and building the index:
everything the window waits for before it can draw a row. `--edit-time` reports one
field replaced, the short search that follows it, and a second field replaced, which
is the cost a keystroke carries. `--save-time` edits one
field on a copy, writes the whole library and reports how long that took, which is
the window's save path with the display taken out of it. `--search` reports the
query alone, after the index exists. `--explain` prints how many rows each column
holds for each term of a query, which is how a search that returns too much or too
little is traced to a column. `--index-report` prints, for each column, how many
postings it holds, how many distinct four-byte runs, how many whole words, how many
distinct short runs the gram table names, and how many one-row words it holds as
pairs: that is how a library that opens slowly is traced to the column and to the
structure responsible.

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
| 15 000 | 11.0 MiB | 0.23 s | 102 MiB | 15 us |
| 50 000 | 36.8 MiB | 0.78 s | 333 MiB | 16 us |
| 100 000 | 73.6 MiB | 1.64 s | 661 MiB | 16 us |
| 30 000, unique words | 11.9 MiB | 2.85 s | 697 MiB | 95 us |

The first three rows have a flat per-entry cost, which is what makes the figure
usable: a library twice the size costs twice the time, so 200 000 entries is about
four seconds and can be predicted rather than measured.

**The fourth row is why the per-entry figure is not a promise.** It is a quarter
the size of the 100 000-entry library and opens slower, because opening is paid
per distinct word and not per entry. A library whose every abstract is new text
has as many words as it has words, and each new word must be entered into the word
table and into every four-byte run of itself. A real bibliography sits between the
two shapes: it repeats its venues, its authors and much of its vocabulary, but not
its abstracts.

**A word only one row holds costs no gram entry.** A short term is answered from a
table of which words hold each run of one to three bytes, and that table used to
carry an entry for every run of every word, with a start and a length pointing into
a list of word ids. For a word held by one row the row itself is the answer, so it
now goes in a pair of run and row instead, and the gram table is left with the words
that repeat. On the unique-word library that removed the whole gram table: opening
fell from 3.68 s to 2.85 s and the peak from 765 MiB to 697 MiB, with every short
term still answered from a lookup. A gram that names several words now costs one
entry in the table whether it names two or two thousand, because a run of words ends
where the next one starts.

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
| 15 000 | `Neural` | 11 317 | 0.10 ms |
| 50 000 | `Neural` | 37 516 | 0.35 ms |
| 100 000 | `Neural` | 74 989 | 0.72 ms |
| 15 000 | `pre` (a three-byte fragment) | 15 000 | 0.08 ms |
| 50 000 | `pre` | 50 000 | 0.26 ms |
| 100 000 | `pre` | 100 000 | 0.52 ms |
| 100 000 | `Sparse Sampling Manifold Privacy` | 30 967 | 4.46 ms |
| 100 000 | `a` (a one-byte fragment) | 100 000 | 3.18 ms |
| 100 000 | `zzzznotaword` | 0 | 0.04 ms |
| 100 000 | empty (matches all) | 100 000 | 0.28 ms |
| 30 000 unique | `pre` | 100 | 0.05 ms |
| 30 000 unique | `w` (a one-byte fragment) | 30 000 | 0.17 ms |
| 30 000 unique | `a` (a one-byte fragment) | 30 000 | 0.34 ms |

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
100 000 rows in 0.52 ms; before that table existed the same query walked every word
the column had ever used and cost 13.4 ms on the unique-word library.

**A fragment in nearly every row** is the case the ladder cannot show, because its
columns hold forty words each. A real library holds a hundred thousand, and there
`a` reaches §5. The unique-word library is where the one-row words show their worth:
`w` fell from 1.04 ms to 0.17 ms and `a` from 0.87 ms to 0.34 ms, because neither
walks a vocabulary any more and both read a run straight out of the solo table. What
was left was the bitmap the rows are marked in, which is why `a`, a byte in every
row, still costs twice `w`.

## 4. Saving

A save writes the whole library, since a release that reformatted half a file would
be worse than a slow one. The cost is therefore linear in the file, and the question
is only whether a large library makes the window wait long enough to notice.

| Library | Entries | Written | Save |
| --- | --- | --- | --- |
| `lib100k.bib` | 100 000 | 73.60 MiB | 0.067 s |
| `lib50k.bib` | 50 000 | 36.79 MiB | 0.020 s |
| `lib15k.bib` | 15 000 | 11.02 MiB | 0.004 s |
| The console catalogue | 45 974 | 4.71 MiB | 0.002 s |

That is under a millisecond per mebibyte, so the 150 MiB file this page used to
leave unmeasured would take about 0.13 s: one frame's worth of pause, not a freeze.
This is why saving stayed synchronous. A worker thread, a progress line and a rule
for what a user may edit mid-write would be a state machine in place of 130 ms.

The real catalogue is faster per byte than the generated ladder because most of its
bytes are a URL that no field touches, and a save copies them through rather than
re-rendering them.

## 5. Editing

A keystroke replaces one field of one row, and the index used to answer it by
cloning every table of every column and rebuilding the column from scratch: 7.9 s
per keystroke on 50 000 entries and 31.7 s on 100 000, which made the filter box
unusable on a library that opened in under two seconds. A field is now edited in
place, and the tables that name a word rather than a row are laid out again by the
next search that needs them rather than by each key.

| Library | Entries | One edit | The short search after it | A second edit |
| --- | --- | --- | --- | --- |
| `lib15k.bib` | 15 000 | 0.0000 s | 0.019 s | 0.0001 s |
| `lib50k.bib` | 50 000 | 0.0001 s | 0.082 s | 0.0001 s |
| `lib100k.bib` | 100 000 | 0.0001 s | 0.195 s | 0.0001 s |
| `lib30k_unique.bib` | 30 000 | 0.0000 s | 2.31 s | 0.0000 s |

```bash
for f in lib15k lib50k lib100k lib30k_unique; do
  ./target/release/citerack --edit-time "/tmp/bench/$f.bib"
done
```

**An edit is free and the search after it is not.** A keystroke costs a tenth of a
millisecond on every library here, because it touches the row's own postings: the
words it no longer says come out of the lists they are in and the words it says now
go in. What an edit cannot patch is the table that names a word, since adding one
word to a gram moves every entry after it. That table is dropped instead, and the
first short query after the edit builds the ones it reads: a term of four bytes or
more is answered from the run table, which an edit keeps current, so a query like
`surname` never triggers a layout at all.

**The cost of a layout is the vocabulary of the column, not the row count.** 15 000
entries pay 19 ms and 100 000 pay 195 ms, because most of the work is a column
whose words are nearly all distinct: the `doi` column of the ladder holds 100 000
distinct values and costs 0.17 s of the 0.195 s, while every other column together
costs 8 ms. The last row is the shape this page already calls the worst case: a
library whose every word is new pays 2.3 s, and 1.9 s of it is one column's solo
table of 19.6 million entries. That is a real cost and it is not hidden, but it is
paid once per edit rather than once per key, and only when the query is short
enough to need it.

**What it was, measured the same way.** The same edit on the same libraries:

| Library | Before | After |
| --- | --- | --- |
| `lib15k.bib` | 1.07 s | 0.0000 s |
| `lib50k.bib` | 7.90 s | 0.0001 s |
| `lib100k.bib` | 31.67 s | 0.0001 s |
| `lib30k_unique.bib` | 119.13 s | 0.0000 s |

The old cost was proportional to the library, which is why it was invisible on a
sample and fatal on a real one: two minutes for one keystroke on a library of 30 000
entries, against a tenth of a millisecond now. The new cost is proportional to the
row, which is what a keystroke should depend on.

## 6. Whether the figures hold on another library

The question a benchmark has to answer is not what it measured but what it did not.
Four shapes are measured, and the two that come from real users disagree with the
two that are generated in a way worth naming.

| Library | Entries | Open | Memory | What it represents |
| --- | --- | --- | --- | --- |
| ladder, 100 000 | 100 000 | 1.64 s | 661 MiB | a library that repeats itself |
| unique, 30 000 | 30 000 | 2.85 s | 697 MiB | the worst case for a word table |
| `ARCADE` | 45 974 | 0.21 s | 110 MiB | a user's arcade-game catalogue |
| `console` | 134 352 | 0.67 s | 278 MiB | a user's console-game catalogue |

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
carries fewer words than a generated abstract does. ARCADE opens in 0.21 s, and
console, at nearly three times its entry count, in 0.67 s: under a second for a
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
| console, 134 352 | `a` | 17.9 ms | 2.95 ms |
| ARCADE, 45 974 | `a` | 5.4 ms | 0.97 ms |
| unique, 30 000 | `a` | 3.6 ms | 0.34 ms |
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
take 2.85 s and 697 MiB where 100 000 repeating entries take 1.64 s and 661 MiB.

That dependency was a real defect and is now closed. Before the short-term tables a
three-byte term on the unique-word library cost 13.4 ms and a one-byte term 56.2 ms,
nearly three times the budget. `pre` now costs 0.05 ms there and `a` 0.34 ms, and
the console catalogue, four times larger and with far more distinct words, answers
`a` in 2.95 ms.

**The one cost that does not shrink is listing the answer.** A query that matches
every row must return every row, and a hundred thousand `u32`s cost 0.28 ms to
build even on the fast path the empty query takes. No index removes that; it is the
price of the result, not of the search.

## 7. What a high-entropy library would cost

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
| 0 (the real library) | 144 122 | 0.64 s | 278 MiB | 2.95 ms |
| 2 | 412 826 | 1.77 s | 554 MiB | 3.6 ms |
| 5 | 815 882 | 3.23 s | 843 MiB | 3.7 ms |
| 10 | 1 524 000 | 6.19 s | 1 339 MiB | 3.5 ms |
| 20 | 2 831 162 | 11.34 s | 2 361 MiB | 2.79 ms |

```bash
python3 scripts/widen.py docs/fixtures/games.zip /tmp/widen console.bib 0 2 5 10 20
for f in /tmp/widen/*.bib; do ./target/release/citerack --open-time "$f"; done
```

**What regresses, and by how much.** Opening grows to 18 times the real library's
time and 8.5 times its memory at the twentieth additional word, and the growth is
close to linear in distinct words rather than in rows: the library is 3.8 times its
original size, but its vocabulary is 20 times larger and its opening time 18 times
longer. A query does not regress at all: `a` costs 2.79 ms on the +20 library against
2.95 ms on the real one, because a query costs the words its own term names and not
the words that exist, and a run of one byte names every word in the column either way.

**Where the memory goes.** `--index-report` names the column and the structure. On the
+20 library the abstract column holds 26 519 575 postings, 2 687 040 distinct words,
and 52 879 237 entries in the table of one-byte, two-byte and three-byte runs of those
words, which no longer has anything else in it: every word there is held by one row.
The four-byte run table is 737 476 entries, a thirty-fifth of that.

**What this change bought, measured against the version before it.** The same five
libraries, opened by both builds on the same machine:

| Words added per entry | Open before | Open now | Memory before | Memory now |
| --- | --- | --- | --- | --- |
| 0 | 0.73 s | 0.64 s | 253 MiB | 278 MiB |
| 2 | 2.09 s | 1.77 s | 547 MiB | 554 MiB |
| 5 | 4.13 s | 3.23 s | 954 MiB | 843 MiB |
| 10 | 7.55 s | 6.19 s | 1 649 MiB | 1 339 MiB |
| 20 | 15.54 s | 11.34 s | 3 118 MiB | 2 361 MiB |

The first row is a loss and is worth naming: on the real library the peak grew 25
MiB, because two thirds of its words are held by one row and a pair of run and row
is less compact, entry for entry, than a word id into a list shared by many words.
From the fifth added word onward the trade reverses and keeps growing, to 27 percent
of the opening time and 24 percent of the memory at the top of the ramp. The reading
is that the index now costs what its vocabulary costs, and a library that repeats
ninety words in a hundred pays a little for the shape it does not have.

**Why real data cannot have this shape.** Counted on the same files, in words rather
than entries: the console catalogue runs 11.2 tokens per distinct word and ARCADE
9.5, so 91 percent and 89 percent of what they say has been said before. A library
whose every word is new sits at 1.0 by construction. Real bibliographies repeat
their venues, their authors, their publishers, their method names and their
citation keys, and even the console catalogue — auto-imported game metadata rather
than scholarly work — repeats nine words in ten. Two million distinct words would
need two million entries sharing no word with any other, which is not a
bibliography.

**The case that does happen is narrower and cheaper.** The console catalogue is 66
percent single-occurrence words: 95 804 of its 144 122 distinct words appear in
exactly one entry, and `--index-report` counts them in the column that holds them.
That is the real version of this problem, and it is why the +20 row is worth
publishing rather than dismissing: the direction is real even though the magnitude
is not. The title column there holds 20 350 such words out of 42 602, and the key
column 95 804 out of 108 376, which is why the change that was made for the invented
library also touches a real one.

## 8. What is not measured

| Unmeasured | Why it matters |
| --- | --- |
| Time to first frame | it needs a frame-timing harness on a real display; the window is laid out before the file is read, but "before" is a design claim, not a figure |
| Scroll smoothness at 100 000 rows | the table is virtualised, so the frame cost should not depend on row count, but a claim of that kind needs a frame trace |
| A library below a thousand entries | every figure here is a large library; a small one opens in milliseconds, which is fast enough to make the number uninteresting rather than unknown |
