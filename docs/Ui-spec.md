# Ui spec

The window is a table with a filter above it and a detail pane beside it. The
layout is small on purpose: every control answers a question a reader asks while
writing a paper, and the habit it serves is "find the entry, read the fields,
type the key into the manuscript".

Nothing here is a pixel specification. What a widget looks like is the window
library's business; what matters is where the eye goes and what a keystroke does.

## 1. Region map

```
+---------------------------------------------------------------+
| Open | Sort Key Author Year Title | Save       toolbar         |
+---------------------------------------------------------------+
| Search [....................................]  filter box     |
+----------+---------------------------------------+------------+
| Groups   |  Key  Type  Author  Title  Year  Venue |  Detail    |
|          |  ...                                  |  pane      |
| Year     |  ...                                  |  fields    |
|  2026    |  ...                                  |            |
|  2025    |                                       |            |
+----------+---------------------------------------+------------+
| 15,000 entries | opened in 0.29 s | path        status line     |
+---------------------------------------------------------------+
```

| Region | Fixed? | Why it sits there |
| --- | --- | --- |
| Toolbar | top, fixed height | commands that change the whole file, reachable before the eye reaches the table |
| Filter box | top, fixed height, full width | the one control used most, and a full width means a long query stays visible |
| Groups | left, resizable, collapsible | a facet list; it narrows what the table holds and never holds data itself |
| Table | centre, takes the remaining space | the main content, so it gets every pixel the others do not claim |
| Detail | right, resizable, collapsible | fields of the selected row, each one editable in place |
| Status line | bottom, fixed height | one sentence about what just happened, in the same place every time |

The five-region split is the arrangement a bibliography tool has trained its
users to expect. It is used here because that expectation is worth keeping, not
because the arrangement is interesting: a reader who has used any such tool will
find the table in the middle and the fields on the right without being told.

## 2. Toolbar

| Control | Behaviour |
| --- | --- |
| Open | asks for a `.bib` file and replaces the session |
| Sort by Key / Author / Year / Title | sorts by that column; clicking the active one reverses it |
| Save | writes the file back, byte for byte outside the fields that were edited |
| Duplicates N | narrows the table to the entries in a duplicate pair, and back |

The sort buttons carry `^` or `v` showing the direction. Clicking a different
field sorts it ascending; clicking the same field again flips the direction.
This is the behaviour of a table header, exposed as a button because the header
is also a drag target for resizing.

Save carries a `*` while an edit is not yet written to the file, and loses it once
the save returns. There is no confirmation dialog: a save cannot lose an edit,
because a value that cannot be written where it sits is refused when it is typed
rather than when it is saved.

`Duplicates N` is disabled when `N` is zero, which is the answer most libraries
give and the one a user wants to read at a glance. Pressing it shows only the rows
in a pair and the button reads `(shown)` until it is pressed again. The search and
the sort are left alone underneath, so the filter can be turned off without the
table losing its place. A count is a count of pairs, not of entries: two copies of
one paper are one pair and two rows.

## 3. Filter box

- Typing filters after 120 ms of quiet, so a word costs one search rather than
  one per character.
- Space separates terms; every term must match, in any field, in any order.
- `Escape` clears the box.
- An empty box shows every entry and restores the sort.

Matching is a case-insensitive substring over the citation key, the entry type,
and every field value. Accents fold: `muller` finds `Müller`.

The result sentence in the status line states three numbers, and each answers a
different question:

```
Found 9,562 results  |  shown 9,562  |  0.007 s
```

`Found` is how many entries match, which tells the reader whether the query was
too broad. `shown` is how many rows are drawn, which differs from `Found` once a
cap exists. The duration is there so a slow query is visible rather than felt.

## 4. Table

| Column | Contents | Notes |
| --- | --- | --- |
| Key | citation key, monospace | the value the reader copies into the manuscript |
| Type | entry type | lower case as written |
| Author | first author's surname | full list is in the detail pane |
| Title | title | truncated with an ellipsis, never wrapped |
| Year | four digits, right-aligned | right-aligned so a column of years reads as numbers |
| Venue | journal, else book title, else publisher | one column, because a reader scans for "where" |

Rows are one line tall and every cell truncates rather than wraps. A wrapped row
makes the position of the next row depend on the length of a title, which is
exactly what a reader scrolling to a remembered entry cannot afford.

The table is virtualised: only the rows on screen are laid out, so opening a file
with 100 000 entries costs the same frame time as one with a thousand. A table
that lays out every row is the second-most common way a bibliography tool
freezes, after parsing on the UI thread.

Clicking a row selects it and fills the detail pane. Arrow keys move the
selection once the table has focus.

## 5. Detail pane

Shows the entry type and key as a header, then every field in the order the file
listed them, with the field name in the weak colour and the value in the normal
one. Fields the parser did not model appear here too, because the pane is drawn
from the parsed list rather than from a fixed schema: whatever the file held is
what the user sees.

Every value is a text box. A value is committed when the box loses focus, so
typing a word does not rewrite the entry once per keystroke, and what is saved is
the text once the user has finished with it.

A value that cannot be written where it sits is refused rather than escaped: a
brace that would close the entry early, a quote that would end a quoted value, or
a word written into a bare field cannot be stored without changing the delimiters,
and the delimiters belong to the file. The box keeps the refused text and the
reason appears under the grid, so the typing is not thrown away.

Only the bytes of an edited value move in the file. The delimiters, the spacing
around the `=`, the order of the fields and every byte of every other entry stay
as the file had them, which is what lets a hand-formatted library be edited here
rather than only read.

When the entry is in a duplicate pair, one line per pair appears under the header,
naming the other entry's key and what the two matched on. A pair is reported once
per reason, so an entry sharing both a DOI and a title with another says so twice:
each is a separate thing a user might fix.

What counts as the same work is deliberately narrow. A shared DOI is one, compared
with the `doi:` and resolver prefixes stripped and the case folded, since a DOI is
case-insensitive. So is a shared title and year, with the title folded, braces
removed, punctuation dropped and spacing collapsed, so `A Study of {N}ested
Braces` and `a study of nested braces` are one title. A title alone is not enough,
and neither is a year alone: without both halves there is nothing to be confident
about, and a guess that is wrong costs more attention than a duplicate that went
unreported.

## 6. Groups

Lists the distinct years in the library, newest first, and clicking a year puts
it in the filter box. It is a shortcut for a query a user would otherwise type.

Groups is deliberately thin in this release. It exists to hold the place the
arrangement gives it, and to prove that a facet can narrow the table without the
table knowing what narrowed it. Author, journal and keyword facets are roadmap
items, and each is a different query string rather than a different region.

## 7. Keyboard

| Key | Where | Effect | Also a control |
| --- | --- | --- | --- |
| `Ctrl`/`Cmd` + `O` | anywhere | open a file | the `Open` button |
| `Ctrl`/`Cmd` + `S` | anywhere | save | the `Save` button |
| `Ctrl`/`Cmd` + `F` | anywhere | put the cursor in the filter box | the box itself |
| `Escape` | filter box | clear the filter | — |
| `Ctrl`/`Cmd` + `Z` | anywhere | take back the last change | the `Undo` button |
| `Ctrl`/`Cmd` + `Shift` + `Z`, or `Ctrl` + `Y` | anywhere | put the change back | the `Redo` button |
| `Down` | table | select the next row | the row itself |
| `Up` | table | select the previous row | the row itself |
| `Ctrl`/`Cmd` + `D` | anywhere | put the cursor in the first field of the entry | the first field box |
| `Tab`, `Shift` + `Tab` | detail pane | next or previous field, wrapping | the fields themselves |
| `Escape` | detail pane | leave the field box, keeping what was typed | clicking outside it |

The list is short because a key binding nobody knows about is a key binding
nobody uses. Each entry here is also reachable from a control, so the keyboard is
a shortcut rather than the only way.

`Ctrl+F` reaches the filter from a field box as well as from the table, because
the cursor is in a field box when a user has just finished an edit. The four
modifier keys fire wherever the focus is; a field box holds no state the library
has not already been given.

The arrows are the exception that proves the rule: they belong to the text when a
box holds the cursor, so they move the table only when no box does. A move stops
at the first and last row rather than wrapping, and the row it lands on is
scrolled into view, so the selection is never somewhere the user cannot see. With
nothing selected, `Down` takes the first row and `Up` the last, so either key
does something from wherever the user starts.

`Ctrl+D` is how the detail pane is reached without a click: it puts the cursor in
the entry's first field, and from there `Tab` walks the fields in file order,
wrapping at either end so one key reaches the whole entry. `Tab` and `Escape`
belong to the pane while a box holds the cursor; everywhere else `Tab` is egui's,
moving through the toolbar and the facets. `Escape` leaves the box and commits
what was typed in it, exactly as clicking elsewhere in the pane would, so leaving
a box never loses an edit.

## 8. Empty and loading states

| State | What is drawn |
| --- | --- |
| No file open | the table area holds one sentence: `Open a .bib file to begin.` |
| File being read | a spinner and `Opening...` in the status line; the toolbar stays live |
| Parse failed | the error's message, with the byte offset and line, in the status line |
| A query with no match | an empty table and `Found 0 results`; the filter box keeps its text |

A parse error names a position rather than an entry, because the parser stops
where it stopped and the reader's next move is to open the file at that line.

## 9. What this spec refuses

- No modal dialog for anything this release does. A modal takes the keyboard away
  from a user who was typing.
- No tree of "folders" over the library. The filter box and the groups list
  cover the same ground with fewer clicks and no state to lose.
- No column that exists only to be sorted. Every column is read.
- No progress bar for opening in this release. It would need a byte offset from
  the parser, and a bar that lies about how far along it is costs more trust
  than the absence of one costs patience. The spinner says "working", which is
  all that is known.

## 10. How these claims are tested

The window is drawn by a function that takes a context, so a test can call it
without an event loop or a display. The test harness sends clicks and keystrokes
through the same input a window sends, and reads the result back from the
accessibility tree rather than from a screenshot: a field is found by the value
it holds, and the claim that a value was committed is checked against the
library behind the window.

Two limits are worth knowing before writing one of these tests. A frame that
carries a key event ends the pass, so a test that needs several keystrokes runs
a frame between them. Text typed into a box arrives as one event, which is what
a paste looks like; a test that wants to replace a value empties the box first.
