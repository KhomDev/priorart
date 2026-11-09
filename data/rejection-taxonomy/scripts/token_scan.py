#!/usr/bin/env python3
"""The tripwire matcher. The token LIST is private and is not in this repository.

`scripts/check_hygiene.py` and `tests/test_token_scan.py` both call `scan_text`,
so the check the author runs and the check the tests pin are the same function.

Why the list is private
-----------------------
Until 0.5.1 the tokens were published here. A list of the ecosystem and platform
words an author is careful never to write says which ecosystems and platforms
they work on, which is the thing the list exists to hide. Publishing digests
instead does not help: the tokens are short dictionary words, and anyone can hash
a glossary. So the list lives with the other private inputs and is loaded from
there; without it the tripwire check reports SKIP.

Boundaries
----------
A token counts only when it stands alone as a word:

    (?<![A-Za-z0-9_]) TOKEN (?![A-Za-z0-9_])

A token that does not end in a word character (a prefix such as `abc-`) gets no
trailing boundary, because none can exist there; its leading boundary is what
keeps a longer word ending in the same letters from matching.
"""

from __future__ import annotations

import re
from collections.abc import Iterable
from pathlib import Path

_WORD_CHAR = re.compile(r"[A-Za-z0-9_]")


def pattern(token: str) -> re.Pattern[str]:
    """The explicit word-boundary pattern for one token."""
    left = r"(?<![A-Za-z0-9_])" if _WORD_CHAR.match(token[0]) else ""
    right = r"(?![A-Za-z0-9_])" if _WORD_CHAR.match(token[-1]) else ""
    return re.compile(left + re.escape(token) + right, re.IGNORECASE)


def compile_tokens(tokens: Iterable[str]) -> tuple[tuple[str, re.Pattern[str]], ...]:
    cleaned = [t.strip() for t in tokens if isinstance(t, str) and t.strip()]
    return tuple((t, pattern(t)) for t in cleaned)


def load_tokens(path: Path) -> tuple[str, ...]:
    """Read the private tripwire list: a YAML mapping with a `tokens:` list."""
    import yaml  # noqa: PLC0415 - only needed when the private list exists

    data = yaml.safe_load(path.read_text(encoding="utf-8")) or {}
    tokens = data.get("tokens") if isinstance(data, dict) else None
    if not isinstance(tokens, list) or not tokens:
        raise SystemExit(f"ERROR: {path} has no `tokens:` list")
    return tuple(str(t).strip() for t in tokens if str(t).strip())


def scan_line(line: str, compiled: tuple[tuple[str, re.Pattern[str]], ...]) -> list[str]:
    """Every token that stands alone in one line, in list order."""
    return [token for token, pat in compiled if pat.search(line)]


def scan_text(text: str, compiled: tuple[tuple[str, re.Pattern[str]], ...]) -> list[tuple[str, int]]:
    """(token, 1-based line number) for every token that stands alone."""
    hits: list[tuple[str, int]] = []
    for lineno, line in enumerate(text.splitlines(), start=1):
        for token in scan_line(line, compiled):
            hits.append((token, lineno))
    return hits
