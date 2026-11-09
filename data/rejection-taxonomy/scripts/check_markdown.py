#!/usr/bin/env python3
"""Check that every Markdown file renders, and that the README pins a real version.

Two defects shipped in the initial public release and nothing caught them:

  1. A shell rewrite turned code-fence lines into "`@", so four README blocks
     rendered as plain text on GitHub.
  2. The README told readers to pin a tag that did not exist.

What it checks:

  - No line in a tracked .md file starts with "`@".
  - Every tracked .md file has an even number of ``` fence lines.
  - Every "rejection-taxonomy/tree/vX.Y.Z" link in README.md names the version
    in pyproject.toml.
  - CITATION.cff `version` and the newest CHANGELOG.md release heading agree with
    pyproject.toml, and CITATION.cff `date-released` is that release's date.

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
        fences = re.findall(r"^\s*```", text, re.MULTILINE)
        if len(fences) % 2:
            problems.append(f"{rel}: odd number of code fences ({len(fences)}); one block is unclosed")

    pyproject = (REPO_ROOT / "pyproject.toml").read_text(encoding="utf-8")
    vm = re.search(r'^version\s*=\s*"([^"]+)"', pyproject, re.MULTILINE)
    if not vm:
        print("check_markdown: no version in pyproject.toml", file=sys.stderr)
        return 2
    readme = (REPO_ROOT / "README.md").read_text(encoding="utf-8")
    for tag in sorted(set(re.findall(r"rejection-taxonomy/tree/(v[\w.\-]+)", readme))):
        if tag != "v" + vm.group(1):
            problems.append(f"README.md pins {tag} but pyproject.toml is {vm.group(1)}")

    version = vm.group(1)
    citation = (REPO_ROOT / "CITATION.cff").read_text(encoding="utf-8")
    cv = re.search(r'^version:\s*"?([^"\n]+)"?', citation, re.MULTILINE)
    cd = re.search(r"^date-released:\s*(\S+)", citation, re.MULTILINE)
    if not cv or cv.group(1).strip() != version:
        problems.append(f"CITATION.cff version is {cv.group(1) if cv else None}, pyproject.toml is {version}")
    changelog = (REPO_ROOT / "CHANGELOG.md").read_text(encoding="utf-8")
    top = re.search(r"^## \[(\d+\.\d+\.\d+)\] - (\d{4}-\d{2}-\d{2})", changelog, re.MULTILINE)
    if not top or top.group(1) != version:
        problems.append(f"newest CHANGELOG.md release is {top.group(1) if top else None}, pyproject.toml is {version}")
    elif not cd or cd.group(1) != top.group(2):
        problems.append(f"CITATION.cff date-released is {cd.group(1) if cd else None}, CHANGELOG says {top.group(2)}")

    if problems:
        print(f"check_markdown: {len(problems)} problem(s)")
        for p in problems:
            print("  " + p)
        return 1
    print("check_markdown: fences well formed; README pin, CITATION.cff and CHANGELOG agree with pyproject.toml")
    return 0


if __name__ == "__main__":
    sys.exit(main())
