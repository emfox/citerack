# Roadmap

Only what is still open is listed. A row that closes is deleted here and written
into `CHANGELOG.md` in the same commit, in the words a user would use.

Each row names what a user gains and what would have to be true for the work to
be called done. A row without a completion test is an aspiration.

## 1. Before a first release

These are the rows that stand between the current tree and a version somebody
outside this repository could install and use daily.

| Item | What a user gains | Done when |
| --- | --- | --- |
| Inverted index over folded columns | a search on a 100 000-entry library stays under 20 ms | a query costs one pass over the postings of its rarest term, not over every row; `docs/Performance.md` records the new figure at three sizes |
| Editable fields | correcting a typo without a text editor | an edit, a save and a reopen produce the edited text, and the untouched parts of the file are still byte-identical |
| Undo and redo over edits | recovering from a wrong edit | `Ctrl+Z` reverses the last edit and a save after it writes the reverted text |
| Asynchronous save | a large file does not freeze the window while it is written | the status line shows progress and the window stays draggable during a save of 150 MiB |
| Duplicate detection | finding the same paper entered twice | two entries sharing a DOI, or sharing a title and a year, are listed with the pair and the reason |
| Keyboard-only navigation | working without a mouse | opening a file, filtering, selecting and reading a detail pane are all reachable from the keyboard, and the path is written down in `docs/Ui-spec.md` |
| Packaging | installing without a toolchain | a Linux binary and a macOS bundle are produced by CI, with a checksum beside each |

## 2. After that

Rows here are ordered by how many users would notice, not by how interesting the
work is.

| Item | What a user gains | Done when |
| --- | --- | --- |
| Facets for author, journal and type | narrowing by a click instead of a typed query | each facet narrows the table, and the count beside it reflects the other facets rather than the whole file |
| Entry editing that follows the type's required fields | a form that answers what the entry type needs | adding an `@article` offers author, title, journal and year, and flags an empty required one |
| Citation key generation | a key that follows a pattern rather than a shrug | a configurable pattern produces a unique key, and a collision appends a suffix rather than overwriting |
| Bulk operations | retagging or renaming in one step | a multi-row selection can be edited, and the file is still byte-identical outside the edited fields |
| External file attachments | opening the PDF from the entry | a field naming a path opens that file, and a missing file is reported beside the field rather than as a dialog |
| Import from DOI and arXiv | typing an identifier instead of an entry | an identifier produces a complete entry, and a failed lookup leaves the library untouched |
| Locale catalogues | a window in a language other than English | every user-facing string is read from `locales/<tag>/main.ftl` and an unknown tag falls back to English |
| Windows build | running on the platform most students have | the same three commands in `README.md` produce a working binary on a Windows runner |

## 3. Decided against

Kept as a list so the same question is not reopened every few months.

| Idea | Why not |
| --- | --- |
| A database behind the library | the file is the source of truth, and a user who syncs it through their own tool must not find that a second copy has drifted. The index is built at open and thrown away at close |
| A plugin system | it would fix interfaces before the first release says which ones are stable. A crate split is the same benefit with a compiler checking it |
| Cloud sync | it is a service, with an account, a bill and a privacy policy. A file in a folder the user already syncs is the answer that needs no service |
| A separate "quick search" and "advanced search" | two boxes to learn instead of one to type into. Field-qualified terms can be added to the one box when a need is demonstrated |
| Rendering the PDF in the window | a reader has a PDF viewer it already likes. Opening the file hands the job to it |
