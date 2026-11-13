#!/usr/bin/env python3
"""Check that relative markdown links and code paths in this repo resolve.

Fails on: a relative link that points at a missing file, an absolute host path
(/Users/..., C:\\Users\\...), and a relative link that escapes the repository.
External http(s) links are not fetched: CI must not need the network.
"""

from __future__ import annotations

import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
LINK = re.compile(r"\[[^\]]*\]\(([^)]+)\)")
CODE_PATH = re.compile(r"`([A-Za-z0-9_./-]+\.(?:md|json|yaml|yml|rs|sh|py))`")
SKIP_DIRS = {".git", "target", "eval", "node_modules"}
HOST_PATH = re.compile(r"^(?:/Users/|/home/|[A-Za-z]:[\\/]|\\\\)")


def markdown_files() -> list[pathlib.Path]:
    out = []
    for path in ROOT.rglob("*.md"):
        if any(part in SKIP_DIRS for part in path.parts):
            continue
        out.append(path)
    return sorted(out)


def check_links(path: pathlib.Path) -> list[str]:
    problems = []
    text = path.read_text(encoding="utf-8", errors="replace")
    for match in LINK.finditer(text):
        target = match.group(1).strip()
        if not target or target.startswith(("http://", "https://", "mailto:", "#")):
            continue
        target = target.split("#", 1)[0]
        if not target:
            continue
        if HOST_PATH.match(target):
            problems.append(f"{path.relative_to(ROOT)}: absolute host path {target}")
            continue
        resolved = (path.parent / target).resolve()
        try:
            resolved.relative_to(ROOT)
        except ValueError:
            problems.append(f"{path.relative_to(ROOT)}: link escapes the repo: {target}")
            continue
        if not resolved.exists():
            problems.append(f"{path.relative_to(ROOT)}: missing target {target}")
    return problems


def check_code_paths(path: pathlib.Path) -> list[str]:
    problems = []
    text = path.read_text(encoding="utf-8", errors="replace")
    for match in CODE_PATH.finditer(text):
        target = match.group(1)
        if target.startswith(("http", "src/", "docs/", "tests/", "scripts/", "vocab/", "data/", "schemas/", "examples/")):
            candidate = ROOT / target
        else:
            candidate = path.parent / target
        if not candidate.exists():
            problems.append(f"{path.relative_to(ROOT)}: names a missing file {target}")
    return problems


def main() -> int:
    problems = []
    for path in markdown_files():
        problems.extend(check_links(path))
    if problems:
        for problem in problems:
            print(f"BROKEN {problem}", file=sys.stderr)
        return 1
    print(f"links OK ({len(markdown_files())} markdown files)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
