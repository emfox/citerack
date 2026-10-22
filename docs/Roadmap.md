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

Nothing is open. The three rows the roadmap held are shipped: attachments, bulk
edits, and an import from an identifier, each in the version it was written for.

The one thing a translator would still need is a second `impl` of the keys in
`crates/app/src/catalogue.rs` and a setting that chose it. That is deliberately not
a row: nobody has asked for a language, and the roadmap does not hold work whose
only justification is that it would be possible.

## 2. Decided against

| Idea | Why not |
| --- | --- |
| A database behind the library | the file is the source of truth, and a user who syncs it through their own tool must not find that a second copy has drifted. The index is built at open and thrown away at close |
| A plugin system | it would fix interfaces before the first release says which ones are stable. A crate split is the same benefit with a compiler checking it |
| Cloud sync | it is a service, with an account, a bill and a privacy policy. A file in a folder the user already syncs is the answer that needs no service |
| A separate "quick search" and "advanced search" | two boxes to learn instead of one to type into. Field-qualified terms can be added to the one box when a need is demonstrated |
| Rendering the PDF in the window | a reader has a PDF viewer it already likes. Opening the file hands the job to it |
| A window in another language | a translated interface is a second thing to keep true, and a user who cannot read the English one is not the reader this release is for. The engine still indexes and matches every script, so a translation would touch the interface and nothing below it — which is why it waits behind the catalogue row rather than being refused outright |
