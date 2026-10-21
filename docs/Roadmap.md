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
| Duplicate detection | finding the same paper entered twice | two entries sharing a DOI, or sharing a title and a year, are listed with the pair and the reason |
| A second renderer behind the first | a window that opens on hardware where the GL driver is broken | when `glow` fails to start, the same process is started again with `wgpu` and the window appears instead of exiting with an error |

Asynchronous saving was on this list and came off it in October 2026, not because it
was done but because it was measured and the reason for it did not hold: a save
costs under a millisecond per mebibyte, so the 150 MiB file the row named takes
about 0.13 s. `docs/Performance.md` §4 has the figures and the command that
re-derives them. A pause that short does not justify a worker thread, and a file
that large is rare enough that the miss would not be reported.

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
| A window in another language | a translated interface is a second thing to keep true, and a user who cannot read the English one is not the reader this release is for. The engine still indexes and matches every script, and every user-facing string is already in one catalogue, so the work can be picked up without touching the engine |
