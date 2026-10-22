# Roadmap

Only what is still open is listed. A row that closes is deleted here and written
into `CHANGELOG.md` in the same commit, in the words a user would use.

Each row names what a user gains and what would have to be true for the work to
be called done. A row without a completion test is an aspiration.

Two versions have shipped, so there is no "before a first release" list left.
Anything decided against stays in §2 so the same question is not reopened.

## 1. Open

Rows are ordered by how many users would notice, not by how interesting the work
is.

| Item | What a user gains | Done when |
| --- | --- | --- |
| UI strings in one catalogue | a window that speaks a language other than English | every word the window shows comes from one file, and a second language file changes the window without a rebuild |
| Import from DOI and arXiv | typing an identifier instead of an entry | an identifier produces a complete entry, and a failed lookup leaves the library untouched |

The catalogue row is what a translated window needs, and what the tree does not
have: the strings are inline in `crates/app/src` and there is no `locales/`
directory.

## 2. Decided against

| Idea | Why not |
| --- | --- |
| A database behind the library | the file is the source of truth, and a user who syncs it through their own tool must not find that a second copy has drifted. The index is built at open and thrown away at close |
| A plugin system | it would fix interfaces before the first release says which ones are stable. A crate split is the same benefit with a compiler checking it |
| Cloud sync | it is a service, with an account, a bill and a privacy policy. A file in a folder the user already syncs is the answer that needs no service |
| A separate "quick search" and "advanced search" | two boxes to learn instead of one to type into. Field-qualified terms can be added to the one box when a need is demonstrated |
| Rendering the PDF in the window | a reader has a PDF viewer it already likes. Opening the file hands the job to it |
| A window in another language | a translated interface is a second thing to keep true, and a user who cannot read the English one is not the reader this release is for. The engine still indexes and matches every script, so a translation would touch the interface and nothing below it — which is why it waits behind the catalogue row rather than being refused outright |
