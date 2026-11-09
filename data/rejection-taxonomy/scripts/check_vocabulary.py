#!/usr/bin/env python3
"""Check that the gate and anti-pattern ids published here mean what they mean in
viability-gate, at a pinned viability-gate tag.

P2 once meant "privileged-role-gated" in viability-gate and "duplicate check"
here. That is the class of bug this check exists for: one id, two meanings.
Every id this repository shares with viability-gate must exist there with the
title recorded below.

Known exception: `P18_design_tradeoff`. viability-gate's P18 is "Known
characteristic documented in a prior audit". No row uses P18 here, so nothing
published is wrong today; the value is retired at the next breaking release. The
check fails the moment a row uses it.

The sibling checkout is found in --viability-gate, else $VIABILITY_GATE_DIR,
else ../viability-gate. CI checks out PINNED_VG_TAG there and passes --require;
without --require a missing sibling is a SKIP.

Usage:
    python scripts/check_vocabulary.py [--viability-gate PATH] [--require]
    python scripts/check_vocabulary.py --print-pinned-tag
"""

from __future__ import annotations

import argparse
import csv
import os
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(Path(__file__).resolve().parent))

from vocab import VALID_GATES  # noqa: E402

#: The ONE place the viability-gate version is pinned. Bump it deliberately.
PINNED_VG_TAG = "v0.5.0"

#: published gate value -> (viability-gate id, viability-gate title)
SHARED_IDS: dict[str, tuple[str, str]] = {
    "D1_duplicate_check": ("D1", "Known-issues / duplicate"),
    "G1_severity_floor": ("G1", "Severity floor"),
    "G2_exclusion_clause": ("G2", "Exclusion clauses"),
    "G3_scope_tree": ("G3", "Scope tree"),
    "G4_prior_audit": ("G4", "Prior audits"),
    "G5_test_intent": ("G5", "Test-suite intent"),
    "P7_best_practice": ("P7", "Best-practice or convention critique"),
    "P12_no_persisted_impact": ("P12", "Panic or revert without persisted impact"),
    "P16_below_floor": ("P16", "Severity mismatch with the program payout floor"),
    "P17_no_attacker_win": ("P17", "No attacker-win scenario"),
    "P19_headline_mismatch": ("P19", "Headline impact contradicted by PoC evidence (SPAM line)"),
    "P20_cooperating_party": ("P20", "Requires a cooperating party"),
}

#: Published values whose id collides with a different meaning in viability-gate.
KNOWN_COLLISIONS: dict[str, tuple[str, str]] = {
    "P18_design_tradeoff": ("P18", "Known characteristic documented in a prior audit"),
}


def sibling_dir(arg: str | None) -> Path:
    if arg:
        return Path(arg)
    if os.environ.get("VIABILITY_GATE_DIR"):
        return Path(os.environ["VIABILITY_GATE_DIR"])
    return ROOT.parent / "viability-gate"


def load_titles(vg: Path) -> dict[str, str]:
    import yaml  # noqa: PLC0415

    titles: dict[str, str] = {}
    for name, key in (("gates.yaml", "gates"), ("anti-patterns.yaml", "rules")):
        doc = yaml.safe_load((vg / "rules" / name).read_text(encoding="utf-8")) or {}
        for entry in doc.get(key) or []:
            titles[str(entry["id"])] = str(entry.get("title", ""))
    return titles


def published_counts() -> dict[str, int]:
    with (ROOT / "data" / "rejections.csv").open(encoding="utf-8", newline="") as fh:
        counts: dict[str, int] = {}
        for row in csv.DictReader(fh):
            counts[row["earliest_gate"]] = counts.get(row["earliest_gate"], 0) + 1
    return counts


def check(titles: dict[str, str], counts: dict[str, int]) -> list[str]:
    problems: list[str] = []
    gate_like = {g for g in VALID_GATES if g[:2] in {"D1", "G1", "G2", "G3", "G4", "G5"} or g.startswith("P")}
    unmapped = gate_like - set(SHARED_IDS) - set(KNOWN_COLLISIONS)
    if unmapped:
        problems.append(f"vocabulary values with a viability-gate-style id but no mapping here: {sorted(unmapped)}")
    for value, (vid, title) in SHARED_IDS.items():
        if vid not in titles:
            problems.append(f"{value}: id {vid} does not exist in viability-gate")
        elif titles[vid] != title:
            problems.append(f"{value}: viability-gate {vid} is titled {titles[vid]!r}, expected {title!r}")
    for value, (vid, title) in KNOWN_COLLISIONS.items():
        if counts.get(value):
            problems.append(
                f"{value} is used by {counts[value]} row(s), but {vid} means {titles.get(vid, title)!r} "
                f"in viability-gate - rename the value before publishing such a row"
            )
    if "P2_duplicate_check" in counts:
        problems.append("P2_duplicate_check is published again; it was renamed to D1_duplicate_check")
    return problems


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--viability-gate", help="path to a viability-gate checkout at PINNED_VG_TAG")
    ap.add_argument("--require", action="store_true", help="fail, rather than skip, when it is missing")
    ap.add_argument("--print-pinned-tag", action="store_true")
    args = ap.parse_args(argv)

    if args.print_pinned_tag:
        print(PINNED_VG_TAG)
        return 0
    vg = sibling_dir(args.viability_gate)
    if not (vg / "rules" / "gates.yaml").exists():
        msg = f"viability-gate not found at {vg} (check out {PINNED_VG_TAG} there)"
        if args.require:
            print(f"ERROR: {msg}", file=sys.stderr)
            return 2
        print(f"SKIP: {msg}")
        return 0

    problems = check(load_titles(vg), published_counts())
    if problems:
        for p in problems:
            print(f"VOCAB  {p}", file=sys.stderr)
        return 1
    print(
        f"OK: {len(SHARED_IDS)} shared ids agree with viability-gate at {vg}; "
        f"{len(KNOWN_COLLISIONS)} known collision unused"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
