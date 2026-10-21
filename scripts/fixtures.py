"""Record and verify the checksums of the samples in docs/fixtures.

The samples are kept exactly as they arrived, so a digest is worth writing down.
This script is the only thing that writes `docs/fixtures/CHECKSUMS`, which keeps
the file and the bytes from drifting apart.

    python3 scripts/fixtures.py write    # record what is there now
    python3 scripts/fixtures.py check    # verify, and fail if anything moved
"""

import hashlib
import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent / "docs" / "fixtures"
RECORD = ROOT / "CHECKSUMS"


def digest(path):
    sha = hashlib.sha256()
    with open(path, "rb") as handle:
        for block in iter(lambda: handle.read(1 << 20), b""):
            sha.update(block)
    return sha.hexdigest()


# Text that describes the folder rather than being a sample in it.
PROSE = {"README.md", "CHECKSUMS"}


def samples():
    """Every sample file, which is every regular file that is not documentation."""
    return sorted(p for p in ROOT.iterdir() if p.is_file() and p.name not in PROSE)


def write():
    lines = [
        "# sha256 of every sample in docs/fixtures, written by scripts/fixtures.py.",
        "# Verify with: python3 scripts/fixtures.py check",
    ]
    for path in samples():
        lines.append("%s  %s" % (digest(path), path.name))
    RECORD.write_text("\n".join(lines) + "\n")
    print("wrote %d digest(s) to %s" % (len(lines) - 2, RECORD.relative_to(ROOT.parent.parent)))


def check():
    if not RECORD.exists():
        print("no CHECKSUMS yet; run: python3 scripts/fixtures.py write")
        return 1
    recorded = {}
    for line in RECORD.read_text().splitlines():
        if line.startswith("#") or not line.strip():
            continue
        sha, name = line.split()
        recorded[name] = sha
    failures = 0
    for name, sha in sorted(recorded.items()):
        path = ROOT / name
        if not path.exists():
            print("missing  %s" % name)
            failures += 1
        elif digest(path) != sha:
            print("changed  %s" % name)
            failures += 1
        else:
            print("ok       %s" % name)
    for path in samples():
        if path.name not in recorded:
            print("unlisted %s" % path.name)
            failures += 1
    return 1 if failures else 0


if __name__ == "__main__":
    command = sys.argv[1] if len(sys.argv) > 1 else "check"
    if command == "write":
        write()
        raise SystemExit(0)
    if command == "check":
        raise SystemExit(check())
    raise SystemExit("usage: fixtures.py [write|check]")
