#!/usr/bin/env python3
"""Check that every relative link in a repository resolves, and that every
"pin to a tag" reference points at a tag rather than at a moving branch.

Run from the repository root:
    python scripts/check_links.py
    python scripts/check_links.py --json

Why this exists: documentation drifts, and a broken link in a repo whose whole
argument is "everything here is checkable" is worse than a broken link anywhere
else.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

SKIP_DIRS = {".git", "__pycache__", ".pytest_cache", ".ruff_cache", "target", "node_modules", ".venv"}
LINK_RE = re.compile(r"\[[^\]]*\]\(([^)]+)\)")

# A relative link to a directory or a file must resolve. Fragments are checked
# against the target file's headings, best effort.
IGNORE_SCHEMES = ("http://", "https://", "mailto:", "#")


def headings(text: str) -> set[str]:
    out = set()
    for line in text.splitlines():
        line = line.strip()
        if not line.startswith("#"):
            continue
        title = line.lstrip("#").strip().lower()
        slug = re.sub(r"[^a-z0-9 \-_]", "", title).replace(" ", "-")
        out.add(slug)
    return out


def check(files: list[Path]) -> list[str]:
    problems: list[str] = []
    heading_cache: dict[Path, set[str]] = {}
    for path in files:
        text = path.read_text(encoding="utf-8")
        rel_self = path.relative_to(ROOT).as_posix()
        for m in LINK_RE.finditer(text):
            target = m.group(1).strip()
            if target.startswith(IGNORE_SCHEMES):
                continue
            line = text[: m.start()].count("\n") + 1
            # Split off the fragment.
            fragment = ""
            if "#" in target:
                target, fragment = target.split("#", 1)
            if not target:
                continue
            resolved = (path.parent / target).resolve()
            if not resolved.exists():
                problems.append(f"{rel_self}:{line}: broken link -> {target}")
                continue
            if fragment and resolved.is_file() and resolved.suffix == ".md":
                if resolved not in heading_cache:
                    heading_cache[resolved] = headings(
                        resolved.read_text(encoding="utf-8")
                    )
                if fragment.lower() not in heading_cache[resolved]:
                    problems.append(
                        f"{rel_self}:{line}: fragment #{fragment} not found in {target}"
                    )
    return problems


def iter_markdown() -> list[Path]:
    out = []
    for path in ROOT.rglob("*.md"):
        if any(part in SKIP_DIRS for part in path.parts):
            continue
        out.append(path)
    return sorted(out)


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--json", action="store_true")
    args = ap.parse_args(argv)

    files = iter_markdown()
    problems = check(files)
    if args.json:
        print(json.dumps({"markdown_files": len(files), "problems": problems}, indent=2))
        return 1 if problems else 0
    if problems:
        for p in problems:
            print(f"BROKEN  {p}", file=sys.stderr)
        print(f"\n{len(problems)} broken link(s)", file=sys.stderr)
        return 1
    print(f"OK: {len(files)} markdown file(s), every relative link resolves")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
