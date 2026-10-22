#!/usr/bin/env python3
"""Run every check this repository expects, from one command.

A working copy rarely has every tool CI has, so this runs what it can, names the
reason it skipped the rest, and never reports a skip as a pass.

Usage:
    python3 scripts/check_all.py                # everything this machine can run
    python3 scripts/check_all.py --quick        # no cargo
    python3 scripts/check_all.py --list         # what would run, and why
    python3 scripts/check_all.py --only policy --only rust
    python3 scripts/check_all.py --skip rust
    python3 scripts/check_all.py --require-all  # a skip is a failure
    python3 scripts/check_all.py --verbose      # stream each check's output

Exit code: 0 when nothing failed, 1 when a check failed, and with --require-all
also when a check had to be skipped.
"""

from __future__ import annotations

import argparse
import shutil
import subprocess
import sys
import time
from dataclasses import dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PYTHON = sys.executable

# Cheap checks first, so a policy mistake is reported before a build is spent.
GROUPS = ("policy", "repo", "rust", "review")

# Groups that need a Rust toolchain. --quick leaves them out.
SLOW_GROUPS = ("rust",)

# Groups whose findings ask for a second look instead of blocking a merge.
ADVISORY_GROUPS = ("review",)


@dataclass(frozen=True)
class Check:
    """One gate: the steps to run, and what the machine must have for them."""

    name: str
    group: str
    steps: tuple[tuple[str, ...], ...]
    needs: tuple[str, ...] = ()


def py(*args: str) -> tuple[str, ...]:
    """A sibling script, run by the interpreter running this one."""
    return (PYTHON, *args)


CHECKS: tuple[Check, ...] = (
    # Section 1 and 2 of docs/Conventions.md.
    Check("language", "policy", (py("scripts/check_language.py"),)),
    # Section 3. The history is checked from its first commit, so a rewrite of
    # an old message cannot slip in behind a clean recent one.
    Check(
        "commits",
        "policy",
        (py("scripts/check_commits.py", "--base", ""),),
        needs=("git", "commits"),
    ),
    # Section 3.1, reported rather than blocked: a script can measure where the
    # lines landed, not why.
    Check("commit-scope", "review", (py("scripts/check_commit_scope.py"),), needs=("git",)),
    # The checkers are code too, and a checker that silently narrows what it
    # reports is worse than one that fails.
    Check(
        "self-tests",
        "repo",
        (py("-m", "unittest", "discover", "-s", "scripts", "-p", "test_*.py"),),
    ),
    # A sample whose bytes moved invalidates every figure measured on it.
    Check("fixtures", "repo", (py("scripts/fixtures.py", "check"),)),
    Check("fmt", "rust", (("cargo", "fmt", "--all", "--check"),), needs=("cargo",)),
    Check(
        "clippy",
        "rust",
        (
            (
                "cargo",
                "clippy",
                "--workspace",
                "--all-targets",
                "--all-features",
                "--",
                "-D",
                "warnings",
            ),
        ),
        needs=("cargo",),
    ),
    Check("build", "rust", (("cargo", "build", "--workspace", "--all-targets"),), needs=("cargo",)),
    Check("test", "rust", (("cargo", "test", "--workspace"),), needs=("cargo",)),
    # The Windows exe is linked on Linux, so what it binds is decided here. A
    # build machine that already has the DLL would hide a missing import.
    Check(
        "windows-imports",
        "rust",
        (
            (
                "cargo",
                "xwin",
                "build",
                "--release",
                "--target",
                "x86_64-pc-windows-msvc",
                "-p",
                "citerack",
            ),
            (
                "bash",
                "scripts/check_windows_imports.sh",
                "target/x86_64-pc-windows-msvc/release/citerack.exe",
            ),
        ),
        needs=("cargo", "cargo-xwin", "llvm-objdump"),
    ),
    # The Linux binary promises a reader that nothing has to be installed beside
    # it, which holds only while the window stack stays a run-time load.
    Check(
        "linux-links",
        "rust",
        (
            ("cargo", "build", "--release", "-p", "citerack"),
            ("bash", "scripts/check_linux_links.sh", "target/release/citerack"),
        ),
        needs=("cargo", "readelf"),
    ),
    # The engine crates are timed by a benchmark that runs headless, so a
    # regression is visible without opening a window.
    Check(
        "perf",
        "rust",
        (("cargo", "test", "-p", "citerack-store", "--release", "--test", "budget"),),
        needs=("cargo",),
    ),
)


def has_commits() -> bool:
    """Whether HEAD names a commit, which the history checks need."""
    done = subprocess.run(
        ["git", "rev-parse", "--verify", "HEAD"],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    return done.returncode == 0


def precondition(name: str) -> tuple[bool, str]:
    """Whether this machine can run the check, and why not when it cannot."""
    if name == "git":
        if shutil.which("git") is None:
            return False, "git is not on PATH"
        return True, ""
    if name == "commits":
        if not has_commits():
            return False, "the repository has no commits yet"
        return True, ""
    if name == "cargo":
        if shutil.which("cargo") is None:
            return False, "cargo is not on PATH (https://rustup.rs)"
        return True, ""
    if name == "cargo-xwin":
        if shutil.which("cargo-xwin") is None:
            return False, "cargo-xwin is not on PATH (cargo install cargo-xwin)"
        return True, ""
    if name == "llvm-objdump":
        if shutil.which("llvm-objdump") is None:
            return False, "llvm-objdump is not on PATH (apt install llvm)"
        return True, ""
    if name == "readelf":
        if shutil.which("readelf") is None:
            return False, "readelf is not on PATH (apt install binutils)"
        return True, ""
    raise ValueError(f"unknown precondition {name!r}")


def blocked(check: Check) -> str:
    """The first unmet precondition, or an empty string when nothing blocks."""
    for need in check.needs:
        ok, reason = precondition(need)
        if not ok:
            return reason
    return ""


def run(check: Check, verbose: bool) -> tuple[bool, str, float]:
    """Run every step in order; return (passed, captured output, seconds)."""
    started = time.perf_counter()
    captured: list[str] = []
    passed = True
    for step in check.steps:
        command = list(step)
        if verbose:
            print(f"    $ {' '.join(command)}", flush=True)
        done = subprocess.run(
            command,
            cwd=ROOT,
            text=True,
            encoding="utf-8",
            errors="replace",
            capture_output=not verbose,
        )
        if not verbose:
            captured.append(done.stdout or "")
            captured.append(done.stderr or "")
        if done.returncode != 0:
            passed = False
            break
    return passed, "".join(captured), time.perf_counter() - started


def selected(only: list[str], skip: list[str], quick: bool) -> list[Check]:
    """The checks the flags leave in, keeping the declared order."""
    chosen: list[Check] = []
    for check in CHECKS:
        if quick and check.group in SLOW_GROUPS:
            continue
        if only and check.name not in only and check.group not in only:
            continue
        if check.name in skip or check.group in skip:
            continue
        chosen.append(check)
    return chosen


def describe(check: Check) -> str:
    """The command a check runs, for --list."""
    steps = [
        " ".join("python3" if arg == PYTHON else arg for arg in step)
        for step in check.steps
    ]
    return " && ".join(steps)


def main() -> int:
    parser = argparse.ArgumentParser(
        description=__doc__,
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument("--quick", action="store_true",
                        help="skip the groups that need cargo")
    parser.add_argument("--list", action="store_true",
                        help="print the plan instead of running it")
    parser.add_argument("--only", action="append", default=[], metavar="NAME",
                        help="a check or group to run; repeatable")
    parser.add_argument("--skip", action="append", default=[], metavar="NAME",
                        help="a check or group to leave out; repeatable")
    parser.add_argument("--require-all", action="store_true",
                        help="treat a skipped check as a failure")
    parser.add_argument("--verbose", action="store_true",
                        help="stream each check's output as it runs")
    args = parser.parse_args()

    for stream in (sys.stdout, sys.stderr):
        if hasattr(stream, "reconfigure"):
            stream.reconfigure(errors="replace")

    chosen = selected(args.only, args.skip, args.quick)
    if not chosen:
        print("nothing selected; --list shows what is available")
        return 2

    plan = [(c, blocked(c)) for c in chosen]

    if args.list:
        for group in GROUPS:
            entries = [(c, why) for c, why in plan if c.group == group]
            if not entries:
                continue
            print(f"{group}:")
            for check, why in entries:
                state = f"SKIP  {why}" if why else "run"
                print(f"  {check.name:<16} {state}")
                if not why:
                    print(f"  {'':<16} {describe(check)}")
        return 0

    failed: list[str] = []
    noticed: list[str] = []
    skipped: list[tuple[str, str]] = []
    passed: list[str] = []
    started = time.perf_counter()
    group = ""

    for check, why in plan:
        if check.group != group:
            group = check.group
            print(f"\n== {group} ==")
        label = f"{check.group}/{check.name}"
        if why:
            skipped.append((label, why))
            print(f"  {check.name:<16} skip    {why}")
            continue
        ok, output, elapsed = run(check, args.verbose)
        if ok:
            passed.append(label)
            print(f"  {check.name:<16} ok      {elapsed:5.1f}s")
            continue
        if check.group in ADVISORY_GROUPS:
            noticed.append(label)
            print(f"  {check.name:<16} note    {elapsed:5.1f}s")
        else:
            failed.append(label)
            print(f"  {check.name:<16} FAIL    {elapsed:5.1f}s")
        if not args.verbose and output.strip():
            for line in output.strip().splitlines()[-40:]:
                print(f"      {line}")

    total = time.perf_counter() - started
    print("\n== summary ==")
    print(f"  {len(passed)} passed, {len(failed)} failed, "
          f"{len(noticed)} noted, {len(skipped)} skipped in {total:.1f}s")

    if noticed:
        print("\n  noted, and worth a look rather than a block:")
        for label in noticed:
            print(f"    {label}")
    if skipped:
        print("\n  skipped, with the reason:")
        for label, why in skipped:
            print(f"    {label:<24} {why}")
    if failed:
        print("\n  failed:")
        for label in failed:
            print(f"    {label}")
        print("\nNothing here is advisory: a failed check blocks a merge.")
        return 1
    if args.require_all and skipped:
        print("\n--require-all: a skipped check counts as a failure.")
        return 1
    print("\nOK: every check that could run passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
