#!/usr/bin/env python3
"""Repo hygiene: encoding, private paths, and - with the private inputs - tripwire
tokens and the redaction blocklist across the whole git history.

1. **Encoding.** Every tracked file is UTF-8, with no byte-order mark and no
   mojibake. A BOM is not cosmetic: one at the start of `.gitattributes` made git
   ignore `eol=lf` during checkout, so every Windows clone got CRLF files and a
   failing test suite (fixed in 0.6.0).
2. **Private paths.** A path from the private workspace never reaches a public
   file.
3. **Tripwire tokens** (needs the private list). Ecosystem and platform words
   that would say which programs the author works on.
4. **History** (needs the private blocklist). Every blob and every commit message
   in every commit, against the private redaction blocklist and the tripwire
   list. A token deleted in a later commit is still in the history forever.

The private inputs are found in --private-dir, else $RT_PRIVATE_DIR. Without them
checks 3 and 4 report SKIP: CI has no private inputs and runs checks 1 and 2.

Usage:
    python scripts/check_hygiene.py --no-history          # what CI runs
    RT_PRIVATE_DIR=/path/to/private python scripts/check_hygiene.py
    python scripts/check_hygiene.py --json
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

sys.path.insert(0, str(Path(__file__).resolve().parent))

from token_scan import compile_tokens, load_tokens, scan_text  # noqa: E402

SKIP_DIRS = {".git", "__pycache__", ".pytest_cache", ".ruff_cache", ".venv", "venv", "node_modules"}

BLOCKLIST_NAME = "redaction-blocklist.PRIVATE.yaml"
TRIPWIRE_NAME = "tripwire-tokens.PRIVATE.yaml"
DECLARATION_KEYS = {"case_sensitive_groups", "case_insensitive_groups"}

BOM = "\ufeff"

# What a UTF-8 file looks like after a round-trip through a cp1252 reader.
MOJIBAKE = [
    "\u00e2\u20ac\u201d",  # em dash
    "\u00e2\u20ac\u201c",  # en dash
    "\u00e2\u20ac\u2122",  # right single quote
    "\u00e2\u20ac\u0153",  # left double quote
    "\u00c3\u00a9",  # e-acute
    "\u00c3\u00a8",  # e-grave
    "\u00c2\u00a0",  # nbsp
    "\ufffd",  # replacement character
]

# Private-workspace paths. Never allowed anywhere.
PRIVATE = ["KNOWLEDGE/", "_private-tools", "RESULTS/", "submission_guides/", "PROTOCOLS/", "engagements/"]

# Files whose JOB is to name a private path.
PRIVATE_ALLOWED = {
    "scripts/check_hygiene.py": "the checker must name what it searches for",
    "tests/test_hygiene.py": "asserts the checker catches each path",
}


def iter_files() -> list[Path]:
    """Every file that would be committed: tracked plus untracked-but-not-ignored.

    git, not a directory walk, so gitignored private inputs are never scanned
    (and never reported as a leak of themselves). Falls back to a walk.
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
        files = [f for f in files if f.is_file()]
        if files:
            return sorted(files)
    except (OSError, subprocess.CalledProcessError):
        pass
    return sorted(
        p for p in ROOT.rglob("*") if p.is_file() and not any(part in SKIP_DIRS for part in p.parts)
    )


def rel(path: Path) -> str:
    try:
        return path.relative_to(ROOT).as_posix()
    except ValueError:
        return path.as_posix()


def line_of(text: str, needle: str) -> int:
    return text[: text.index(needle)].count("\n") + 1


def check_encoding(files: list[Path]) -> list[str]:
    problems: list[str] = []
    for path in files:
        name = rel(path)
        raw = path.read_bytes()
        if b"\0" in raw[:8000]:
            continue  # binary
        try:
            text = raw.decode("utf-8")
        except UnicodeDecodeError as err:
            problems.append(f"{name}: not valid UTF-8 ({err})")
            continue
        if text.startswith(BOM):
            problems.append(f"{name}:1: UTF-8 byte-order mark - remove it")
        for seq in MOJIBAKE:
            if seq in text:
                problems.append(f"{name}:{line_of(text, seq)}: mojibake {seq!r} - re-save as UTF-8")
    return problems


def check_private(files: list[Path]) -> list[str]:
    problems: list[str] = []
    for path in files:
        name = rel(path)
        if name in PRIVATE_ALLOWED:
            continue
        try:
            text = path.read_text(encoding="utf-8")
        except (UnicodeDecodeError, OSError):
            continue
        for token in PRIVATE:
            if token in text:
                problems.append(f"{name}:{line_of(text, token)}: private path {token!r}")
    return problems


def check_tripwires(files: list[Path], compiled) -> list[str]:
    problems: list[str] = []
    for path in files:
        try:
            text = path.read_text(encoding="utf-8")
        except (UnicodeDecodeError, OSError):
            continue
        for token, line in scan_text(text, compiled):
            problems.append(f"{rel(path)}:{line}: tripwire token {token!r}")
    return problems


def load_blocklist(path: Path) -> tuple[set[str], set[str]]:
    """(case-sensitive, case-insensitive) tokens. Undeclared groups are case-insensitive,
    the stricter of the two."""
    import yaml  # noqa: PLC0415

    data = yaml.safe_load(path.read_text(encoding="utf-8")) or {}
    if not isinstance(data, dict):
        raise SystemExit(f"ERROR: {path} is not a YAML mapping")
    cs_groups = set(data.get("case_sensitive_groups") or [])
    cs: set[str] = set()
    ci: set[str] = set()
    for group, values in data.items():
        if group in DECLARATION_KEYS or not isinstance(values, list):
            continue
        for value in values:
            if isinstance(value, str) and value.strip():
                (cs if group in cs_groups else ci).add(value.strip())
    if not cs and not ci:
        raise SystemExit(f"ERROR: blocklist {path} is empty")
    return cs, ci


def blocklist_hits(text: str, cs: set[str], ci: set[str]) -> list[str]:
    hits = [t for t in cs if re.search(rf"(?<![A-Za-z0-9_]){re.escape(t)}(?![A-Za-z0-9_])", text)]
    low = text.lower()
    hits += [t for t in ci if re.search(rf"(?<![a-z0-9_]){re.escape(t.lower())}(?![a-z0-9_])", low)]
    return sorted(set(hits))


def git(*args: str) -> str:
    return subprocess.run(
        ["git", *args], cwd=ROOT, capture_output=True, text=True, encoding="utf-8", errors="replace"
    ).stdout


def check_history(cs: set[str], ci: set[str], compiled) -> list[str]:
    """Every blob and every commit/tag message reachable from any ref."""
    problems: list[str] = []
    seen_blobs: set[str] = set()
    for commit in git("rev-list", "--all").split():
        message = git("log", "-1", "--format=%B", commit)
        hits = blocklist_hits(message, cs, ci) + [t for t, _ in scan_text(message, compiled)]
        if hits:
            problems.append(f"{commit[:8]} commit message: {len(set(hits))} token(s)")
        for line in git("ls-tree", "-r", commit).splitlines():
            meta, name = line.split("\t", 1)
            _mode, kind, sha = meta.split()
            if kind != "blob" or sha in seen_blobs:
                continue
            seen_blobs.add(sha)
            content = git("cat-file", "-p", sha)
            hits = blocklist_hits(content, cs, ci) + [t for t, _ in scan_text(content, compiled)]
            if hits:
                # Count only: the report must not itself print a private token.
                problems.append(f"{commit[:8]} {name}: {len(set(hits))} private token(s)")
    for tag in git("tag", "-l").split():
        message = git("tag", "-l", "--format=%(contents)", tag)
        if blocklist_hits(message, cs, ci) or scan_text(message, compiled):
            problems.append(f"tag {tag} message: private token(s)")
    return problems


def private_dir(arg: str | None) -> Path | None:
    value = arg or os.environ.get("RT_PRIVATE_DIR")
    return Path(value) if value else None


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--json", action="store_true")
    ap.add_argument("--private-dir", help="directory holding the private blocklist and tripwire list")
    ap.add_argument("--no-history", action="store_true", help="skip the git-history scan")
    args = ap.parse_args(argv)

    files = iter_files()
    results: dict[str, list[str]] = {
        "encoding": check_encoding(files),
        "private": check_private(files),
    }
    skipped: list[str] = []

    pdir = private_dir(args.private_dir)
    tripwire_path = pdir / TRIPWIRE_NAME if pdir else None
    blocklist_path = pdir / BLOCKLIST_NAME if pdir else None
    compiled = ()
    if tripwire_path and tripwire_path.exists():
        compiled = compile_tokens(load_tokens(tripwire_path))
        results["tripwires"] = check_tripwires(files, compiled)
    else:
        skipped.append("tripwires (private token list not found; set --private-dir or RT_PRIVATE_DIR)")
    if args.no_history:
        skipped.append("history (--no-history)")
    elif blocklist_path and blocklist_path.exists():
        cs, ci = load_blocklist(blocklist_path)
        results["history"] = check_history(cs, ci, compiled)
    else:
        skipped.append("history (private blocklist not found)")

    failed = any(results.values())
    if args.json:
        print(json.dumps({"files_scanned": len(files), **results, "skipped": skipped}, indent=2))
        return 1 if failed else 0
    for label, problems in results.items():
        for p in problems:
            print(f"{label.upper()}  {p}", file=sys.stderr)
    for s in skipped:
        print(f"SKIP  {s}")
    if failed:
        print(f"\n{sum(len(v) for v in results.values())} problem(s)", file=sys.stderr)
        return 1
    print(f"OK: {len(files)} files - {', '.join(results)} clean")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
