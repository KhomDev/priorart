"""Every number in the prose is pinned to data/summary.json.

Until 0.5.1 the docs quoted numbers by hand, and they drifted: 97-row arithmetic
survived three data releases, and a claim about the largest out-of-scope sub-cause
was simply false. Two rules now hold:

1. Each phrase in CLAIMS, formatted with today's aggregates, appears in its file.
2. Every number-bearing phrase in those files (a percentage, "N of M", "N rows",
   "n = N", "N logged") sits inside one of the CLAIMS phrases for that file.

So a number cannot be added to the prose without being registered here, and a
registered number cannot drift from the data. The README's generated block is
excluded: stats.py --check covers it.
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "scripts"))

import vocab  # noqa: E402

S = json.loads((ROOT / "data" / "summary.json").read_text(encoding="utf-8"))
T = S["total"]
TRI = S["triage"]
FAM = S["bug_family"]
PREV = S["preventable_by"]


def pct(n: int) -> str:
    return f"{100.0 * n / T:.1f}%"


#: file -> phrases that must appear verbatim, built from the aggregates.
CLAIMS: dict[str, list[str]] = {
    "README.md": [
        f"**{T} logged bug-bounty rejections",
        f"n = {T}, one researcher",
        f"a closed vocabulary of {len(vocab.BUG_FAMILIES)} families",
        f"{T} logged submissions classified",
    ],
    "CITATION.cff": [
        f"{T} logged submissions classified",
        f"A personal log of {T} rejected",
        f"{S['public_record_preventable']} of {T} ({S['public_record_preventable_pct']}%)",
        f"{PREV['own_log']} more only by the author's own",
    ],
    "data/SCHEMA.md": [
        f"Closed vocabulary of {len(vocab.BUG_FAMILIES)} families",
    ],
    "docs/method.md": [
        f"n = {T}, one researcher",
    ],
    "docs/patterns.md": [
        f"across {T} logged rejections",
        f"**{TRI['duplicate']} of {T} rejections ({pct(TRI['duplicate'])}) were duplicates.**",
        f"{S['repeat_rows']} rows repeat a family",
        f"across {S['repeated_families']} families (the largest has {S['largest_family']} rows)",
        f"{PREV['own_log']} rejections could have been stopped by nothing but",
        f"**{TRI['below_payout_floor']} rows ({pct(TRI['below_payout_floor'])}) were confirmed valid",
        f"the impact was fabricated: **{S['analysis_split']['strict']} row",
        f"**{S['analysis_split']['broad']} of {T} ({S['analysis_split']['broad_pct']}%)**",
        "That is 100% minus one bucket",
        f"--family reentrancy        # {FAM['reentrancy']} rows",
        f"--family accounting        # {FAM['accounting']} rows",
        f"**{FAM['panic']} rows** are `panic`-family findings",
        f"`async_ordering` accounts for **{FAM['async_ordering']} rows**",
        f"**{S['public_record_preventable']} of {T} ({S['public_record_preventable_pct']}%) could have been stopped",
        f"**n = {T}, one researcher.**",
    ],
    "docs/gates.md": [],
    "SECURITY.md": [],
    "CONTRIBUTING.md": [],
}

NUMBER = re.compile(
    r"\b\d+(?:\.\d+)?%|\b\d+ of \d+\b|\b\d+ (?:rows?|families|rejections|duplicates?|submissions|logged)\b|\bn ?= ?\d+"
)


def prose(name: str) -> str:
    text = (ROOT / name).read_text(encoding="utf-8")
    return re.sub(r"<!-- BEGIN:generated -->.*?<!-- END:generated -->", "", text, flags=re.S)


@pytest.mark.parametrize("name", sorted(CLAIMS))
def test_every_registered_claim_is_current(name):
    text = prose(name)
    for phrase in CLAIMS[name]:
        assert phrase in text, f"{name} no longer says {phrase!r} - update the prose or the registry"


@pytest.mark.parametrize("name", sorted(CLAIMS))
def test_every_number_in_the_prose_is_registered(name):
    text = prose(name)
    covered: list[tuple[int, int]] = []
    for phrase in CLAIMS[name]:
        start = text.find(phrase)
        while start != -1:
            covered.append((start, start + len(phrase)))
            start = text.find(phrase, start + 1)
    for m in NUMBER.finditer(text):
        if not any(a <= m.start() and m.end() <= b for a, b in covered):
            line = text[: m.start()].count("\n") + 1
            pytest.fail(f"{name}:{line}: {m.group()!r} is not pinned to summary.json - add it to CLAIMS")


def test_registry_covers_every_prose_file():
    for path in [*sorted((ROOT / "docs").glob("*.md")), ROOT / "README.md", ROOT / "data" / "SCHEMA.md"]:
        assert path.relative_to(ROOT).as_posix() in CLAIMS, path
