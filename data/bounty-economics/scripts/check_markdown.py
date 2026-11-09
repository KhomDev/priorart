#!/usr/bin/env python3
"""Check that every Markdown file renders, and that any README pin names a real version.

A shell rewrite mangled backticks across all three sibling repositories: fence
lines became "`@" in two of them, and here a code span in CONTRIBUTING.md picked
up two stray backticks. Two of them also told readers to pin a tag that did not
exist. This is the same check the siblings run. It catches line-level fence damage;
a stray backtick inside a sentence is not caught, because it looks like valid
inline code.

What it checks:

  - No line in a tracked .md file starts with "`@".
  - No line consists of a lone backtick. That is a fence with two backticks
    missing, and GitHub renders the block under it as one run-on paragraph.
  - Every tracked .md file has an even number of ``` fence lines.
  - Every "bounty-economics/tree/vX.Y.Z" link in README.md names the version
    in pyproject.toml.

Run: python scripts/check_markdown.py
Exit code 0 = clean, 1 = a problem, 2 = could not run.
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent


def tracked_markdown() -> list[Path]:
    try:
        out = subprocess.run(
            ["git", "ls-files", "*.md"],
            cwd=REPO_ROOT,
            capture_output=True,
            text=True,
            check=True,
        ).stdout
    except (OSError, subprocess.CalledProcessError) as exc:
        print("check_markdown: git ls-files failed: " + str(exc), file=sys.stderr)
        sys.exit(2)
    return [REPO_ROOT / line for line in out.splitlines() if line]


def main() -> int:
    problems = []

    for path in tracked_markdown():
        text = path.read_text(encoding="utf-8")
        rel = path.relative_to(REPO_ROOT).as_posix()
        mangled = re.findall(r"^\s*`@", text, re.MULTILINE)
        if mangled:
            problems.append(
                f"{rel}: {len(mangled)} line(s) start with '`@'; a code fence is three backticks"
            )
        lone = re.findall(r"^[ \t]*`[ \t]*$", text, re.MULTILINE)
        if lone:
            problems.append(
                f"{rel}: {len(lone)} line(s) hold a lone backtick; a code fence is three backticks"
            )
        fences = re.findall(r"^\s*```", text, re.MULTILINE)
        if len(fences) % 2:
            problems.append(f"{rel}: odd number of code fences ({len(fences)}); one block is unclosed")

    pyproject = (REPO_ROOT / "pyproject.toml").read_text(encoding="utf-8")
    vm = re.search(r'^version\s*=\s*"([^"]+)"', pyproject, re.MULTILINE)
    if not vm:
        print("check_markdown: no version in pyproject.toml", file=sys.stderr)
        return 2
    readme = (REPO_ROOT / "README.md").read_text(encoding="utf-8")
    for tag in sorted(set(re.findall(r"bounty-economics/tree/(v[\w.\-]+)", readme))):
        if tag != "v" + vm.group(1):
            problems.append(f"README.md pins {tag} but pyproject.toml is {vm.group(1)}")

    if problems:
        print(f"check_markdown: {len(problems)} problem(s)")
        for p in problems:
            print("  " + p)
        return 1
    print("check_markdown: fences well formed, pinned tag matches pyproject.toml")
    return 0


if __name__ == "__main__":
    sys.exit(main())
