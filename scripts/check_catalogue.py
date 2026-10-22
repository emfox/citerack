#!/usr/bin/env python3
"""Fail when the window's words are written where the window draws them.

Every sentence a user reads has to live in `crates/app/src/catalogue.rs`, so that a
second language is one file and nothing else moves. This script finds the literals
that escaped: text handed to an egui label, button or hint, and text assigned to
the status line.

See docs/Conventions.md for the rule.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
SCAN = REPO / "crates" / "app" / "src"

# The one file allowed to hold the words.
CATALOGUE = "catalogue.rs"

# Calls that put a string on screen. A literal here is a sentence the user reads.
RENDERERS = re.compile(
    r"\.(label|button|small_button|selectable_label|heading|strong|weak|link"
    r"|on_hover_text|hint_text|colored_label|monospace)\s*\("
)

# The status line and the refusal lines are read too, though they are drawn by
# `status_line` rather than by the code that words them.
ASSIGNMENTS = re.compile(r"\b(status|refusal|batch_refusal)\s*=")

STRING_LITERAL = re.compile(r'"((?:[^"\\]|\\.)*)"')

# A literal that is a word a user reads: letters, at least two of them. A single
# letter is a column hint or an arrow, and a string with no letters is a separator.
READABLE = re.compile(r"[A-Za-z]{2,}")

# Words that are not a sentence: widget ids, format hints, and the file extensions
# the dialog filters by. Each is a fact about the code, not about the language.
NOT_PROSE = {
    "author}{year",
}

# A line whose literal is handed to the formatter already, through `Text`, is fine;
# what is left to catch is the literal that never reached a key.
SKIP_FILES = {CATALOGUE}


def prose_literals(line: str) -> list[str]:
    """The literals in one line that a user would read as words."""
    out: list[str] = []
    for literal in STRING_LITERAL.findall(line):
        if not READABLE.search(literal):
            continue
        if literal in NOT_PROSE:
            continue
        # A path, an id or a format spec is not a sentence. A space or a capital
        # after the first character is, which is the shape every sentence has.
        out.append(literal)
    return out


def check() -> list[str]:
    errors: list[str] = []
    if not SCAN.is_dir():
        return [f"crates/app/src is not where this script looks: {SCAN}"]
    for path in sorted(SCAN.rglob("*.rs")):
        if path.name in SKIP_FILES:
            continue
        rel = path.relative_to(REPO).as_posix()
        for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            if line.strip().startswith("//"):
                continue
            if not (RENDERERS.search(line) or ASSIGNMENTS.search(line)):
                continue
            for literal in prose_literals(line):
                errors.append(f"{rel}:{number}: {literal!r} is written where it is drawn")
    return errors


def main() -> int:
    errors = check()
    print("[catalogue] every word the window shows comes from one file")
    if not errors:
        print("\nOK: no sentence is written at the widget")
        return 0
    print(f"\nFAILED: {len(errors)} literal(s) outside the catalogue\n")
    for error in errors:
        print(f"  {error}")
    print("\nAdd a key to crates/app/src/catalogue.rs and use it through `text!`.")
    return 1


if __name__ == "__main__":
    sys.exit(main())
