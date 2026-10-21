# Conventions

This repository keeps a small set of rules that are checked by a script rather
than by memory. Everything here is enforced by `python3 scripts/check_all.py`,
and a rule that no script can check is written so a reader can check it instead.

## 1. Language

All engineering artifacts are English: identifiers, comments, doc comments, file
names, directory names, commit messages, issue text, and the documents under
`docs/`. The one exception is a document written for a Chinese-speaking reader,
which is named `*.zh-CN.md` so the name itself carries the exemption.

`README.md` is English and `README.zh-CN.md` is its translation; the pair
cross-links.

User-facing strings shown in the window live in `locales/<tag>/main.ftl` and are
never inlined into Rust. The engine crates produce an error *kind* plus
parameters, never a sentence, so a translation can be added without touching
them.

## 2. Comments

A comment says why, never what. The code already says what it does.

| Position | Limit |
| --- | --- |
| Inside a function body | one line, two when genuinely needed |
| Doc comment on an item | one line, two at most |
| Module header | three lines |

No comment block longer than three lines, anywhere. No commented-out code: git
remembers it. No history notes in comments: that is what the commit message is
for. Design rationale belongs in `docs/`, not in the source.

## 3. Commits

```
<type>(<scope>): <subject>
```

The subject is imperative English, has no trailing period, and is at most 75
bytes so it stays on one line in a terminal. A significant change may add a body
of at most two lines saying why. Anything longer is a document.

Allowed types: `feat`, `fix`, `docs`, `chore`, `refactor`, `perf`, `test`,
`build`, `ci`.

The scope is the crate name without the `citerack-` prefix: `bib`, `index`,
`store`, `app`, or `repo` for something that spans all of them.

### 3.1 One change per commit

A commit carries one logical change. A behaviour and the test that would fail
without it travel together; a refactor and a behaviour change do not.

Stage deliberately, and read the diff before writing the message:

```bash
git add <the files of this one change>
git diff --cached --stat      # one reason, or stop and split
git commit
```

Commit when the change is finished, not when the branch is. A split made
afterwards from memory joins hunks that were never one change and separates
hunks that were.

## 4. Checks

`scripts/check_all.py` runs every gate this machine can run, names the reason it
skipped the rest, and never reports a skip as a pass.

```bash
python3 scripts/check_all.py            # everything available
python3 scripts/check_all.py --quick    # no cargo
python3 scripts/check_all.py --list     # what would run, and why
```

| Group | What it holds | Needs |
| --- | --- | --- |
| `policy` | language and comment density, commit messages | `git` |
| `repo` | the checkers' own tests | nothing |
| `rust` | `fmt`, `clippy -D warnings`, `build`, `test` | `cargo` |
| `review` | commits that read as more than one change | `git` |

`review` reports and does not block. A script can measure where lines landed; it
cannot know why, and a rename or an i18n sweep reaches callers in a shape that
looks like a squash.

The commit message is checked at three points, so a violation is caught before it
becomes history:

| Where | What |
| --- | --- |
| `scripts/hooks/commit-msg` | the message being written, before the commit exists |
| `.github/workflows/ci.yml` | the whole reachable history, on every push |
| `python3 scripts/check_commits.py REF` | the same, on demand |

The hook is one line of shell and can be bypassed with `--no-verify`; the CI step
is the one that cannot be. Install the hook once per clone:

```bash
sh scripts/install-hooks.sh
```

### 4.1 Exemptions

`check_language.py` fails on CJK in source and documentation. Four exemptions
exist, each narrow, and each carries its reason in the script:

| Exemption | Why |
| --- | --- |
| any `*.zh-CN.md` | the name declares the language, so a new translation needs no list edit |
| anything under `locales/` | locale catalogues are the translation data |
| `docs/Conventions.md` | it names the rule it enforces |
| `scripts/check_language.py` | it matches CJK by definition |

A new exemption is a line in the script with a one-line reason, so every
exception stays self-documenting.

## 5. Closing a roadmap item

`docs/Roadmap.md` lists only what is still open. Closing an item deletes its row
and, in the same commit, writes what was delivered into the `Unreleased` section
of `CHANGELOG.md` in the words a user would use. An item withdrawn or decided
against says so in the changelog only when a user could notice.

This is what keeps the roadmap from becoming a history.

## 6. Numbers in a document

Any figure quoted in `docs/` names the machine, the command and the build it came
from, or it is not quoted. A number nobody can reproduce is a claim, and claims
belong in a discussion rather than a document.

Performance figures are produced by `crates/app/benches/` and by the CLI's
`--timing` flag, both of which print the input size beside the result so a reader
can see the scale the number applies to.
