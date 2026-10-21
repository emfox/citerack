# Fixtures

Sample libraries used by the performance page and by hand testing. The files here
are the samples **exactly as they arrived**, so a checksum means something and a
sample can be rebuilt the way its owner published it. Nothing is unpacked or
rewritten in place; a script that needs a different shape derives it into a
temporary directory.

Keep this directory small. A fixture is added only when a figure, a test or a bug
report depends on it, and a large download is recorded here rather than copied in
when it is still fetchable from its source.

## Files

| File | Bytes | Entries | What it is |
| --- | --- | --- | --- |
| `games.zip` | 2 876 135 | — | an archive of two real user libraries; holds `ARCADE.bib` and `console.bib` |

## Where each sample came from

`games.zip` was attached to a public bug report by the user who hit the problem, so
it is a real library rather than one written for a test.

| Field | Value |
| --- | --- |
| Download | `https://github.com/JabRef/jabref/files/12431973/games.zip` |
| Reported in | a public issue about the entry table getting slow at large sizes |
| Uploaded | 2023-08-24, which is also the mtime inside the archive |
| Licence | not stated by the uploader; treat it as third-party content that is referenced, not redistributed under this project's licence |

The archive holds:

| Member | Bytes | Entries | Shape |
| --- | --- | --- | --- |
| `ARCADE.bib` | 4 936 765 | 45 974 | arcade-game catalogue, `@Misc`, four fields, ~100 bytes an entry |
| `console.bib` | 13 903 784 | 134 352 | console-game catalogue, the same shape, with non-ASCII titles |

## Checksums

`CHECKSUMS` beside this file holds one `sha256` per sample. It is written and
verified by `scripts/fixtures.py`, so no digest in this folder was typed by hand:

```bash
python3 scripts/fixtures.py check    # verify the files that are here
python3 scripts/fixtures.py write    # record a new or replaced sample
```

The digests of the two members inside the archive are recorded here too, because
they are what the performance page quotes, and an unzip that produces different
bytes is a different library:

```text
sha256  f92a1d1fe22c23325edcbd70620a68e3e8ee5a6c46f147152836ba8ae146fd1b  ARCADE.bib
sha256  2c5ceb37921d5fca7e873180f543ca869a2ea1cb4f37135d0e3fde3ae5716e46  console.bib
```

Those two live inside `games.zip`, so `scripts/fixtures.py` cannot see them; check
them after unpacking, into a temporary directory rather than in place:

```bash
unzip -o docs/fixtures/games.zip -d /tmp/fixtures
sha256sum /tmp/fixtures/ARCADE.bib /tmp/fixtures/console.bib
```

## Derived shapes

The performance page also measures libraries that are not here, because they are
generated:

- `scripts/genlib.py` writes the synthetic ladder and the unique-word library.
- `scripts/widen.py` takes `console.bib` and appends N unique words to every entry,
  which is how the page shows what a high-entropy library costs. It reads a
  fixture and writes to a directory you name, so nothing in this folder changes.
