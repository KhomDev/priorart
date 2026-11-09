#!/usr/bin/env python3
"""Repo-wide hygiene: encoding, identity tokens, private paths, links.

scripts/check_generic.py guards the SHIPPED DATA (data/*.yaml, schemas/*.json)
against naming a platform. This guards the whole repository, including the prose
and the scripts, against the two other ways a disclosure gets in:

1. **Encoding.** A UTF-8 em dash re-saved as cp1252 is invisible in a diff and
   wrong in every renderer. The byte sequences are checked directly.
2. **Identity tokens.** A chain, ecosystem or platform name says something about
   the author's targets. This repository promises methodology, not instances.
3. **Private paths.** A path from the author's private workspace must never reach
   a public file.

Usage:
    python scripts/check_hygiene.py
    python scripts/check_hygiene.py --json
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# `scripts/` must be importable whether this file is run as a script or imported
# by the test suite, so that both use the same matcher.
sys.path.insert(0, str(Path(__file__).resolve().parent))

from token_scan import scan_text  # noqa: E402

SKIP_DIRS = {
    ".git",
    "__pycache__",
    ".pytest_cache",
    ".ruff_cache",
    ".venv",
    "venv",
    "node_modules",
    "dist",
    "build",
}

MOJIBAKE = [
    "\u00e2\u20ac\u201d",
    "\u00e2\u20ac\u201c",
    "\u00e2\u20ac\u2122",
    "\u00e2\u20ac\u0153",
    "\u00c3\u00a9",
    "\u00c3\u00a8",
    "\u00c2\u00a0",
    "\ufffd",
]

PRIVATE = [
    "KNOWLEDGE/", "_private-tools", "RESULTS/", "submission_guides/",
    "PROTOCOLS/", "engagements/",
]

# The ONE file that legitimately contains identity tokens: the module whose whole
# job is to declare them. This is not a general allowlist, and it is now the only
# one: the matcher is boundary-based, so an ordinary English word that merely
# contains a token needs no exemption, and the line-level exemption for sentences
# that disclaim an ecosystem is gone with it.
ALLOWED = {
    "scripts/token_scan.py": "declares the token vocabulary it scans for",
}


def iter_files() -> list[Path]:
    """Every file that would be committed: tracked plus untracked-but-not-ignored.

    Using git rather than a directory walk keeps gitignored local files (a private
    ledger, a scratch note) out of the scan without an exemption for them. Falls
    back to a walk when git is unavailable.
    """
    try:
        out = subprocess.run(
            ["git", "ls-files", "--cached", "--others", "--exclude-standard"],
            cwd=ROOT,
            capture_output=True,
            text=True,
            check=True,
        ).stdout
        files = [ROOT / line for line in out.splitlines() if line.strip()]
        if files:
            return sorted(files)
    except (OSError, subprocess.CalledProcessError):
        pass
    walked = []
    for path in ROOT.rglob("*"):
        if not path.is_file():
            continue
        if any(part in SKIP_DIRS for part in path.parts):
            continue
        walked.append(path)
    return sorted(walked)


def rel(path: Path) -> str:
    return path.relative_to(ROOT).as_posix()


def check_encoding(files: list[Path]) -> list[str]:
    problems = []
    for path in files:
        try:
            text = path.read_text(encoding="utf-8")
        except (UnicodeDecodeError, OSError):
            continue
        for seq in MOJIBAKE:
            if seq in text:
                line = text[: text.index(seq)].count("\n") + 1
                problems.append(f"{rel(path)}:{line}: mojibake - re-save as UTF-8")
    return problems


def check_tripwires(files: list[Path]) -> list[str]:
    problems = []
    for path in files:
        name = rel(path)
        if name in ALLOWED:
            continue
        try:
            text = path.read_text(encoding="utf-8")
        except (UnicodeDecodeError, OSError):
            continue
        for token, lineno in scan_text(text):
            problems.append(
                f"{name}:{lineno}: identity token {token!r} - this repository "
                f"ships methodology, not instances"
            )
    return problems


def check_private(files: list[Path]) -> list[str]:
    problems = []
    for path in files:
        name = rel(path)
        if name in ALLOWED or name == "scripts/check_hygiene.py":
            continue
        try:
            text = path.read_text(encoding="utf-8")
        except (UnicodeDecodeError, OSError):
            continue
        for token in PRIVATE:
            if token in text:
                line = text[: text.index(token)].count("\n") + 1
                problems.append(f"{name}:{line}: private path {token!r}")
    return problems


def check_history() -> list[str]:
    """Scan every blob in every commit, not just the working tree.

    A token deleted in a later commit is still in the history forever, so a
    working-tree scan cannot replace this one. It found real leaks in the sibling
    repositories, and it found the language-extension list that used to ship in
    this repository's worked examples.
    """
    import subprocess

    def git(*args: str) -> str:
        return subprocess.run(
            ["git", *args],
            cwd=ROOT,
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
        ).stdout

    problems: list[str] = []
    seen: set[tuple[str, str]] = set()
    for commit in git("log", "--all", "--format=%H").split():
        for name in git("ls-tree", "-r", "--name-only", commit).splitlines():
            if (commit, name) in seen:
                continue
            seen.add((commit, name))
            # A file whose JOB is to list these tokens is not a leak in history
            # either. The allowlist is shared with the working-tree scan.
            if name in ALLOWED:
                continue
            content = git("show", f"{commit}:{name}")
            for token, lineno in scan_text(content):
                problems.append(
                    f"{commit[:8]} {name}:{lineno}: identity token {token!r}"
                )
    return problems


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--json", action="store_true")
    ap.add_argument(
        "--no-history",
        action="store_true",
        help="skip the git-history scan",
    )
    args = ap.parse_args(argv)

    files = iter_files()
    encoding = check_encoding(files)
    tripwires = check_tripwires(files)
    private = check_private(files)
    history = [] if args.no_history else check_history()

    if args.json:
        print(
            json.dumps(
                {
                    "files_scanned": len(files),
                    "encoding": encoding,
                    "tripwires": tripwires,
                    "private": private,
                    "history": history,
                },
                indent=2,
            )
        )
        return 1 if (encoding or tripwires or private or history) else 0

    total = len(encoding) + len(tripwires) + len(private) + len(history)
    if not total:
        print(
            f"OK: {len(files)} files - encoding clean, no identity tokens, no private "
            f"paths, nothing in history"
        )
        return 0
    for label, problems in (
        ("ENCODING", encoding),
        ("TRIPWIRE", tripwires),
        ("PRIVATE", private),
        ("HISTORY", history),
    ):
        for p in problems:
            print(f"{label}  {p}", file=sys.stderr)
    print(f"\n{total} problem(s)", file=sys.stderr)
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
