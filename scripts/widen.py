"""Derive a high-entropy library from a real one, without touching the fixture.

Performance.md quotes what a library costs when its vocabulary stops repeating.
Real bibliographies do not have that shape, so it has to be built: this script
takes a fixture, appends N unique words to the abstract of every entry, and writes
the result to a directory the caller names. The row count and the entry shape stay
as they were, so the only thing that changes is how many distinct words exist.

    python3 scripts/widen.py docs/fixtures/games.zip /tmp/widen 0 2 5 10 20

Reads the archive straight out of `docs/fixtures`, so nothing is unpacked there.
"""

import io
import random
import re
import sys
import zipfile

LETTERS = "abcdefghijklmnopqrstuvwxyz"


def entries(text):
    """The whole file split into one block per entry, in file order."""
    return [b for b in re.split(r"\n(?=@)", text) if b.strip().startswith("@")]


def widen(text, extra, seed):
    """The library with `extra` new words appended to every entry's abstract."""
    rng = random.Random(seed)
    out = []
    for block in entries(text):
        block = block.rstrip()
        if extra and block.endswith("}"):
            words = " ".join(
                "w%d%s"
                % (
                    rng.randrange(10**6),
                    "".join(rng.choice(LETTERS) for _ in range(5)),
                )
                for _ in range(extra)
            )
            block = block[:-1] + "abstract = {%s},\n}\n" % words
        out.append(block + "\n\n")
    return "".join(out)


def read(member, archive):
    with zipfile.ZipFile(archive) as bundle:
        return bundle.read(member).decode("utf-8", errors="replace")


def main(archive, target, member, steps):
    text = read(member, archive)
    stem = member.rsplit(".", 1)[0]
    for extra in steps:
        path = "%s/%s_plus%02d.bib" % (target, stem, extra)
        with open(path, "w") as handle:
            handle.write(widen(text, extra, 7))
        print(path)


if __name__ == "__main__":
    if len(sys.argv) < 5:
        raise SystemExit(__doc__)
    main(sys.argv[1], sys.argv[2], sys.argv[3], [int(x) for x in sys.argv[4:]])
