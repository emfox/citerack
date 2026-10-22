#!/usr/bin/env python3
"""Enforce the English-only and comment-density policy. See docs/Conventions.md."""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent

CJK_RANGES = (
    (0x3000, 0x303F),   # CJK punctuation
    (0x3040, 0x30FF),   # Hiragana / Katakana
    (0x3400, 0x4DBF),   # CJK extension A
    (0x4E00, 0x9FFF),   # CJK unified ideographs
    (0xF900, 0xFAFF),   # CJK compatibility
    (0xFF00, 0xFFEF),   # fullwidth forms
    (0x20000, 0x2FA1F),  # CJK extension B and beyond
)

# An exemption is a path and the one-line reason it exists.
ALLOWLIST_FILES = {
    "docs/Conventions.md": "the policy names the rule it enforces",
    "scripts/check_language.py": "this script matches CJK by definition",
}

CODE_SUFFIXES = {".rs", ".toml", ".py", ".sh", ".yml"}
DOC_SUFFIXES = {".md", ".rst"}

COMMENT_RE = re.compile(r"^\s*(///|//!|//)")
MAX_COMMENT_BLOCK = 3

# Build output and vendored sources are not this repository's prose.
SKIP_PREFIXES = ("target/", ".git/", "vendor/")

# A test that checks how non-Latin text is split cannot spell its input any other
# way, so inside a test module a string literal may hold CJK. The exemption stops
# at the literal: a comment in the same file is still prose and still fails.
TEST_MODULE_RE = re.compile(r"^\s*#\[cfg\(test\)\]")
STRING_LITERAL_RE = re.compile(r'"(?:[^"\\]|\\.)*"')


def first_cjk(text: str) -> tuple[int, str] | None:
    for idx, ch in enumerate(text):
        cp = ord(ch)
        if any(lo <= cp <= hi for lo, hi in CJK_RANGES):
            return idx, ch
    return None


def without_literals(line: str) -> str:
    """The line with every string literal removed, so only prose and names remain."""
    return STRING_LITERAL_RE.sub("", line)


def test_module_lines(lines: list[str]) -> set[int]:
    """The line numbers inside a `#[cfg(test)]` module, by brace depth."""
    inside: set[int] = set()
    in_test = False
    depth = 0
    pending = False
    for number, line in enumerate(lines, 1):
        if pending:
            # The module's own line carries the brace the body is counted from.
            depth += line.count("{") - line.count("}")
            pending = False
            if depth <= 0:
                in_test = False
                continue
            inside.add(number)
            continue
        if not in_test:
            if TEST_MODULE_RE.match(line):
                in_test = True
                pending = True
            continue
        depth += line.count("{") - line.count("}")
        if depth <= 0:
            # The brace that closes the module is not inside it.
            in_test = False
            continue
        inside.add(number)
    return inside


def is_allowlisted(rel: str) -> bool:
    if rel in ALLOWLIST_FILES:
        return True
    return any(rel.startswith(p) for p in SKIP_PREFIXES)


def iter_targets() -> list[Path]:
    """Every text file the language policy applies to."""
    out: list[Path] = []
    for p in REPO.rglob("*"):
        if not p.is_file():
            continue
        rel = p.relative_to(REPO).as_posix()
        if is_allowlisted(rel):
            continue
        if "/target/" in rel:
            continue
        if p.suffix in CODE_SUFFIXES or p.suffix in DOC_SUFFIXES:
            out.append(p)
    return sorted(out)


def check_cjk(files: list[Path]) -> list[str]:
    errors: list[str] = []
    for p in files:
        rel = p.relative_to(REPO).as_posix()
        try:
            lines = p.read_text(encoding="utf-8").splitlines()
        except (UnicodeDecodeError, OSError):
            continue
        tested = test_module_lines(lines) if p.suffix == ".rs" else set()
        hits = []
        for number, line in enumerate(lines, 1):
            # Inside a test module the literal is the data under test; the rest of
            # the line, a comment or a name, is still held to the rule.
            subject = without_literals(line) if number in tested else line
            if first_cjk(subject):
                hits.append(number)
        if not hits:
            continue
        preview = "; ".join(f"L{i}" for i in hits[:6])
        more = f" (+{len(hits) - 6} more)" if len(hits) > 6 else ""
        errors.append(f"{rel}: CJK found at {preview}{more}")
    return errors


def check_comment_density(files: list[Path]) -> list[str]:
    errors: list[str] = []
    for p in files:
        if p.suffix != ".rs":
            continue
        rel = p.relative_to(REPO).as_posix()
        lines = p.read_text(encoding="utf-8").splitlines()
        run = 0
        start = 0
        for i, line in enumerate(lines, 1):
            if COMMENT_RE.match(line):
                if run == 0:
                    start = i
                run += 1
            else:
                if run > MAX_COMMENT_BLOCK:
                    errors.append(
                        f"{rel}: comment block of {run} lines at L{start}-L{start + run - 1}"
                    )
                run = 0
        if run > MAX_COMMENT_BLOCK:
            errors.append(f"{rel}: comment block of {run} lines at L{start}")
    return errors


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--cjk-only", action="store_true", help="skip the comment check")
    ap.add_argument("--density-only", action="store_true", help="skip the CJK check")
    args = ap.parse_args()

    files = iter_targets()
    failures: list[str] = []

    print(f"Scanned {len(files)} files\n")

    if not args.density_only:
        cjk = check_cjk(files)
        print(f"[language] {len(cjk)} file(s) contain CJK")
        failures += cjk

    if not args.cjk_only:
        dense = check_comment_density(files)
        print(f"[comments] {len(dense)} comment block(s) exceed {MAX_COMMENT_BLOCK} lines")
        failures += dense

    if not failures:
        print("\nOK: policy satisfied")
        return 0

    print(f"\nFAILED: {len(failures)} violation(s)\n")
    for f in failures:
        print(f"  {f}")
    print("\nSee docs/Conventions.md for the rules.")
    return 1


if __name__ == "__main__":
    sys.exit(main())
