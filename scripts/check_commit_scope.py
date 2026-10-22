#!/usr/bin/env python3
"""Report commits that carry more than one logical change.

Section 3.1 of docs/Conventions.md asks for one logical change per commit, and
it also says what one change looks like: a behaviour and the test that would
fail without it travel together, and a feature crosses every crate between the
parser and the window. So the question this script asks is not "did this commit
touch more than one area" — a feature always does — but "is there anything in
this commit that the subject does not account for".

Three tests, each of which only fires when the one above it did not answer:

1. The subject names the area that carries the bulk of the lines. A commit is
   named for where it lands, so `feat(bib):` on a change whose lines are in
   `store` is a message written before the work was done.
2. The areas outside the crates are ones the subject accounts for. `scripts`,
   `docs` and `.github` are not a layer of the program, so two of them carrying
   real lines is two changes wearing the subject of one.
3. Nothing else. A feature crosses every crate between the parser and the
   window, and that is one change.

A subject that names a scope nothing in the commit settled — `feat(app):` with
lines in `index` and `app` — passes, because the window is how the feature lands.
What the checker looks for is a subject written before the work was done, not a
heavy crate count.

Usage:
    check_commit_scope.py [--limit N] [--verbose] [REF]

Exit code: 0 when nothing is worth a second look, 1 otherwise. This is advisory
review input, not a merge gate: it names commits to read again, and whether the
split was right stays a human decision.
"""

import argparse
import re
import subprocess
import sys

# A commit needs this many changed lines in an area before that area can carry a
# second change. Below it, the lines are mentions: a call site, a test that
# follows, a lock file.
MIN_AREA_LINES = 40

# Files generated from another change in the same commit, so they never count as
# an area of their own: regenerating one is part of touching its source.
GENERATED = {"Cargo.lock"}

# The crates, in the order data flows. A scope sits at one of them, and the
# neighbours it may reach are the ones beside it.
LAYERS = ("bib", "index", "store", "app")

SUBJECT_RE = re.compile(r"^([a-z]+)(?:\(([^)]+)\))?: (.+)$")


def area(path: str) -> str:
    """The top-level area a path belongs to, with a crate named by itself."""
    parts = path.split("/")
    if parts[0] == "crates" and len(parts) > 1:
        return f"crates/{parts[1]}"
    if parts[0] == "crates":
        return "crates"
    if len(parts) > 1 and parts[0] in ("scripts", "docs", ".github", "benches",
                                       "tests"):
        return parts[0]
    return parts[0]


def commits(ref: str) -> list[tuple[str, str, dict[str, int]]]:
    """Every commit as (short sha, subject, {area: changed lines})."""
    raw = subprocess.run(
        ["git", "log", "--no-merges", "--no-renames", "--numstat",
         "--format=%x1e%h%x1f%s", ref],
        capture_output=True, text=True, check=True).stdout
    found: list[tuple[str, str, dict[str, int]]] = []
    for record in raw.split("\x1e"):
        record = record.strip("\n")
        if not record:
            continue
        header, _, rest = record.partition("\n")
        sha, _, subject = header.partition("\x1f")
        lines: dict[str, int] = {}
        for line in rest.split("\n"):
            fields = line.split("\t")
            if len(fields) != 3:
                continue
            added, removed, path = fields
            # A binary file states "-" instead of a count.
            count = (int(added) if added.isdigit() else 0) + \
                    (int(removed) if removed.isdigit() else 0)
            if path in GENERATED:
                continue
            key = area(path)
            lines[key] = lines.get(key, 0) + count
        found.append((sha, subject, lines))
    return found


def neighbours(scope: str) -> set[str]:
    """The areas a change at `scope` may reach: its crate and the ones beside it."""
    if scope == "repo":
        return {"crates/" + layer for layer in LAYERS}
    if scope not in LAYERS:
        return set()
    at = LAYERS.index(scope)
    beside = {LAYERS[i] for i in (at - 1, at + 1) if 0 <= i < len(LAYERS)}
    return {"crates/" + layer for layer in beside}


def review(subject: str, lines: dict[str, int]) -> list[str]:
    """Why this commit looks like more than one change, if it does."""
    heavy = sorted(((a, n) for a, n in lines.items() if n >= MIN_AREA_LINES),
                   key=lambda kv: kv[1], reverse=True)
    if len(heavy) < 2:
        return []

    primary, primary_lines = heavy[0]
    match = SUBJECT_RE.match(subject)
    scope = match.group(2) if match else None
    named = f"crates/{scope}" if scope else None

    if named and primary != named and named in lines:
        return [f"subject names {named} but the bulk of the change "
                f"({primary_lines} lines) is in {primary}"]

    rest = [(a, n) for a, n in heavy[1:]]
    # A layer, the layer it reads and the layer it is read by are one thing: a
    # feature in `bib` is written to be shown in `app`, so those crates travel.
    allowed = {a for a, _ in rest if a.startswith("crates/")} | neighbours(scope)
    allowed |= {named} if named else set()
    beyond = [(a, n) for a, n in rest if a not in allowed]
    if len(beyond) < 2:
        return []
    total = sum(lines.values())
    return [
        f"{len(beyond)} areas carry {MIN_AREA_LINES}+ lines and nothing in the "
        f"subject accounts for them: "
        + ", ".join(f"{a} ({n})" for a, n in beyond)
        + f" of {total} lines"
    ]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("ref", nargs="?", default="HEAD")
    parser.add_argument("--limit", type=int, default=10)
    parser.add_argument("--verbose", action="store_true")
    args = parser.parse_args()

    try:
        entries = commits(args.ref)
    except subprocess.CalledProcessError as err:
        print(f"cannot read git history: {err}", file=sys.stderr)
        return 2

    # A shallow clone pins a synthetic root whose diff is the whole tree; it
    # says nothing about how the repository was written.
    roots = set()
    try:
        roots = set(subprocess.run(
            ["git", "rev-list", "--max-parents=0", args.ref],
            capture_output=True, text=True, check=True).stdout.split())

        shallow = subprocess.run(
            ["git", "rev-parse", "--is-shallow-repository"],
            capture_output=True, text=True, check=True).stdout.strip()
    except subprocess.CalledProcessError:
        shallow = "false"

    flagged = []
    for sha, subject, lines in entries:
        if shallow == "true" and any(r.startswith(sha) for r in roots):
            continue
        reasons = review(subject, lines)
        if reasons:
            flagged.append((sha, subject, lines, reasons))

    shown = flagged[:args.limit]
    for sha, subject, lines, reasons in shown:
        print(f"{sha} {subject}")
        for reason in reasons:
            print(f"    ! {reason}")
        if args.verbose:
            for name, count in sorted(lines.items(), key=lambda kv: -kv[1]):
                print(f"      {count:>6}  {name}")

    total = len(entries)
    if flagged:
        print(f"\n{len(flagged)} of {total} commit(s) carry lines a single "
              f"change does not explain (section 3.1)")
        if len(flagged) > len(shown):
            print(f"{len(flagged) - len(shown)} more not shown; "
                  f"raise --limit to see them")
        return 1
    print(f"OK: {total} commit(s) each read as a change of their own")
    return 0


if __name__ == "__main__":
    sys.exit(main())
