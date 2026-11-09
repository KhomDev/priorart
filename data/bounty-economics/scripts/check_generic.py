#!/usr/bin/env python3
"""Assert the shipped taxonomies and schema name no platform and no foreign repo.

This repository promises methodology, not instances. A platform name, or a link
to somebody else's repository, turns a taxonomy into a pointer at real programs
— which is the one thing the repo says it does not ship.

The project's OWN namespace is allowed: the schema's `$id` is one, and pointing
a reader at our own documentation is not a disclosure.

This lives in a script, rather than inline in the CI workflow, so that the check
that runs in CI is the same check the test suite runs.

Usage:
    python scripts/check_generic.py              # data/*.yaml and schemas/*.json
    python scripts/check_generic.py PATH ...     # specific files

Exit codes:
    0  clean
    1  a banned token was found
    2  usage / read error
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

sys.path.insert(0, str(Path(__file__).resolve().parent))

from token_scan import PLATFORM_TOKENS, scan_tokens  # noqa: E402

# Third-party code hosts. A URL fragment, not a word, so it is matched literally.
BANNED_HOSTS = ("gitlab.com/",)

# Our own GitHub namespace. Anything else is somebody else's repository.
OWN_REPO = "khomdev/"

GITHUB_URL_RE = re.compile(
    r"github\.com/([A-Za-z0-9._-]+/[A-Za-z0-9._-]+)", re.IGNORECASE
)

DEFAULT_TARGETS = ("data/*.yaml", "schemas/*.json")


def scan(text: str) -> list[str]:
    """Return why `text` is not generic. An empty list means clean."""
    problems: list[str] = []
    low = text.lower()
    for token in scan_tokens(text, PLATFORM_TOKENS):
        problems.append(f"names a third-party platform or host: {token!r}")
    for host in BANNED_HOSTS:
        if host in low:
            problems.append(f"names a third-party platform or host: {host!r}")
    for match in GITHUB_URL_RE.finditer(text):
        slug = match.group(1)
        if not slug.lower().startswith(OWN_REPO):
            problems.append(f"links a repository that is not ours: github.com/{slug}")
    return problems


def default_targets() -> list[Path]:
    files: list[Path] = []
    for pattern in DEFAULT_TARGETS:
        files.extend(sorted(ROOT.glob(pattern)))
    return files


def main(argv: list[str] | None = None) -> int:
    args = list(sys.argv[1:] if argv is None else argv)
    files = [Path(a) for a in args] if args else default_targets()
    if not files:
        print("ERROR: no files to scan", file=sys.stderr)
        return 2

    problems = 0
    for path in files:
        try:
            text = path.read_text(encoding="utf-8")
        except OSError as err:
            print(f"ERROR: cannot read {path}: {err}", file=sys.stderr)
            return 2
        for problem in scan(text):
            print(f"ERROR: {path}: {problem}", file=sys.stderr)
            problems += 1

    if problems:
        print(
            f"\n{problems} problem(s): the shipped taxonomies must stay generic.",
            file=sys.stderr,
        )
        return 1

    print(f"taxonomies are generic ({len(files)} file(s) scanned)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
