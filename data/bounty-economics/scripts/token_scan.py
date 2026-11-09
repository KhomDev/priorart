#!/usr/bin/env python3
"""The identity-token vocabulary, and the one matcher that scans for it.

Both live here so that the checks CI runs and the checks the test suite runs are
literally the same functions. `scripts/check_hygiene.py` scans the whole
repository; `scripts/check_generic.py` scans the shipped taxonomies. Both import
from this module, and so does `tests/test_token_scan.py`.

Boundaries
----------
A token counts only when it stands alone as a word. The matcher is the explicit
form of a word boundary:

    (?<![A-Za-z0-9_]) TOKEN (?![A-Za-z0-9_])

That is the fix for a real defect. The previous matcher compared the character
either side of a hit against a set that treated `-`, `_` and `.` as separators,
so a word that merely CONTAINED a token was reported as the token. Widening the
separator set was the wrong repair and exempting the words was worse: the correct
boundary is a word boundary.

A token that does not end in a word character gets no trailing boundary, because
none can exist there.
"""

from __future__ import annotations

import re

# A chain, ecosystem or platform name says something about the author's targets.
# This repository promises methodology, not instances.
TOKENS: tuple[str, ...] = (
    "sui", "move", "aptos", "ton", "tact", "func", "solana", "anchor",
    "cosmos", "cosmwasm", "ibc", "cairo", "starknet", "near", "polkadot",
    "bounce", "witness", "precompile", "create2", "erc-",
    "hackenproof", "immunefi", "cantina", "sherlock", "code4rena",
    "dualdefense", "dualdefence", "flashpool", "flash pool", "primacy of impact",
    "primacy of rules", "hats finance", "bugcrowd", "hackerone", "yeswehack",
)

# The subset that is a third-party bug-bounty platform, a platform's PRODUCT
# name, or a platform-specific rule term rather than a chain. A product name
# identifies its platform as surely as the platform's own name does: the 0.4.x
# taxonomy shipped one ("DualDefense") and none of the checks caught it.
# `check_generic.py` uses this against the shipped data.
PLATFORM_TOKENS: tuple[str, ...] = (
    "hackenproof",
    "immunefi",
    "cantina",
    "sherlock",
    "code4rena",
    "dualdefense",
    "dualdefence",
    "flashpool",
    "flash pool",
    "primacy of impact",
    "primacy of rules",
    "hats finance",
    "bugcrowd",
    "hackerone",
    "yeswehack",
)

_WORD_CHAR = re.compile(r"[A-Za-z0-9_]")


def pattern(token: str) -> re.Pattern[str]:
    """The explicit word-boundary pattern for one token."""
    left = r"(?<![A-Za-z0-9_])" if _WORD_CHAR.match(token[0]) else ""
    right = r"(?![A-Za-z0-9_])" if _WORD_CHAR.match(token[-1]) else ""
    return re.compile(left + re.escape(token) + right, re.IGNORECASE)


_CACHE: dict[tuple[str, ...], tuple[tuple[str, re.Pattern[str]], ...]] = {}


def compiled(tokens: tuple[str, ...]) -> tuple[tuple[str, re.Pattern[str]], ...]:
    if tokens not in _CACHE:
        _CACHE[tokens] = tuple((token, pattern(token)) for token in tokens)
    return _CACHE[tokens]


def scan_line(line: str, tokens: tuple[str, ...] = TOKENS) -> list[str]:
    """Every token that stands alone in one line, in vocabulary order."""
    return [token for token, pat in compiled(tokens) if pat.search(line)]


def scan_text(text: str, tokens: tuple[str, ...] = TOKENS) -> list[tuple[str, int]]:
    """(token, 1-based line number) for every token that stands alone."""
    hits: list[tuple[str, int]] = []
    for lineno, line in enumerate(text.splitlines(), start=1):
        for token in scan_line(line, tokens):
            hits.append((token, lineno))
    return hits


def scan_tokens(text: str, tokens: tuple[str, ...] = TOKENS) -> list[str]:
    """The distinct tokens that stand alone anywhere in text, in vocabulary order."""
    seen = {token for token, _ in scan_text(text, tokens)}
    return [token for token in tokens if token in seen]
