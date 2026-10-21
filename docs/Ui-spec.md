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

The sort buttons carry `^` or `v` showing the direction. Clicking a different
field sorts it ascending; clicking the same field again flips the direction.
This is the behaviour of a table header, exposed as a button because the header
is also a drag target for resizing.

Save carries a `*` while an edit is not yet written to the file. There is no
confirmation dialog: a save cannot lose an edit, because a value that cannot be
written where it sits is refused when it is typed rather than when it is saved.
When the save grows asynchronous, the status line becomes its progress.

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

## 6. Groups

Lists the distinct years in the library, newest first, and clicking a year puts
it in the filter box. It is a shortcut for a query a user would otherwise type.

Groups is deliberately thin in this release. It exists to hold the place the
arrangement gives it, and to prove that a facet can narrow the table without the
table knowing what narrowed it. Author, journal and keyword facets are roadmap
items, and each is a different query string rather than a different region.

## 7. Keyboard

| Key | Where | Effect |
| --- | --- | --- |
| `Ctrl`/`Cmd` + `O` | anywhere | open a file |
| `Ctrl`/`Cmd` + `S` | anywhere | save |
| `Ctrl`/`Cmd` + `F` | anywhere | focus the filter box |
| `Escape` | filter box | clear the filter |
| `Up` / `Down` | table | move the selection |
| `Ctrl`/`Cmd` + `Z` | anywhere | take back the last change |
| `Ctrl`/`Cmd` + `Shift` + `Z`, or `Ctrl` + `Y` | anywhere | put the change back |

The list is short because a key binding nobody knows about is a key binding
nobody uses. Each entry here is also reachable from a control, so the keyboard is
a shortcut rather than the only way.

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
