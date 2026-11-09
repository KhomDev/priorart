#!/usr/bin/env python3
"""Cross-reference a viability-gate gate token to this repo's program-terms catalog.

The four repos share one gate vocabulary. viability-gate maps each anti-pattern
onto a gate token (a fired rule -> the gate it is a statement about). This helper
closes the loop for the two gates decided by the program's OWN TERMS: given a
gate token, it prints the catalog in this repo that tells you how to read the
program page so you never reach that gate.

    anti-pattern (viability-gate)  ->  gate token  ->  program-terms catalog (here)

The mapping is DATA in `data/gate-crossref.yaml`; this script only resolves it
against `data/tier-shapes.yaml` and `data/clause-classes.yaml`.

Usage:
    python scripts/crossref.py                 # print the whole cross-reference
    python scripts/crossref.py G2_exclusion_clause   # print the catalog for one gate
    python scripts/crossref.py --format json   # the full mapping as JSON
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

try:
    import yaml
except ImportError:  # pragma: no cover
    print("crossref: PyYAML is required (pip install pyyaml)", file=sys.stderr)
    raise SystemExit(2) from None

ROOT = Path(__file__).resolve().parent.parent
CROSSREF_FILE = ROOT / "data" / "gate-crossref.yaml"

sys.path.insert(0, str(ROOT / "scripts"))
from validate import load_taxonomies  # noqa: E402

# catalog name in gate-crossref.yaml -> (loader-key, data file)
CATALOGS = {
    "tier-shapes": "data/tier-shapes.yaml",
    "clause-classes": "data/clause-classes.yaml",
    "prior-audit-surface": "(prose only — see the README)",
}


def load_crossref() -> dict:
    if not CROSSREF_FILE.exists():
        print(f"crossref: missing {CROSSREF_FILE}", file=sys.stderr)
        raise SystemExit(2)
    return yaml.safe_load(CROSSREF_FILE.read_text(encoding="utf-8")) or {}


def catalog_entries(catalog: str) -> list[str]:
    """Return the ids a catalog publishes, for the entries listing."""
    tiers, clauses = load_taxonomies()
    if catalog == "tier-shapes":
        return list(tiers)
    if catalog == "clause-classes":
        return list(clauses)
    return []


def print_gate(gate: str, cross: dict) -> int:
    answered = cross.get("answered_here", {})
    not_terms = cross.get("not_program_terms", {})
    if gate in answered:
        info = answered[gate]
        catalog = info["catalog"]
        print(f"{gate}  ->  {catalog} ({CATALOGS.get(catalog, '?')})")
        print(f"  read: {info['read'].strip()}")
        ids = catalog_entries(catalog)
        if ids:
            print(f"  entries ({len(ids)}): {', '.join(ids)}")
        return 0
    if gate in not_terms:
        print(f"{gate}  ->  not answered by this repo's catalogs")
        print(f"  the signal lives in: {not_terms[gate]}")
        return 0
    print(f"crossref: unknown gate token {gate!r}", file=sys.stderr)
    known = sorted(set(answered) | set(not_terms))
    print("  known gates: " + ", ".join(known), file=sys.stderr)
    return 1


def print_all(cross: dict) -> int:
    answered = cross.get("answered_here", {})
    not_terms = cross.get("not_program_terms", {})
    print("Gates answered by this repo's program-terms catalogs:\n")
    for gate, info in answered.items():
        print(f"  {gate:<24} -> {info['catalog']}")
    print("\nGates whose signal is not in the program's terms (not answered here):\n")
    for gate, where in not_terms.items():
        print(f"  {gate:<24} -> {where}")
    return 0


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("gate", nargs="?", help="a gate token, e.g. G2_exclusion_clause")
    ap.add_argument("--format", choices=("text", "json"), default="text")
    args = ap.parse_args(argv)

    cross = load_crossref()
    if args.format == "json":
        print(json.dumps(cross, indent=2))
        return 0
    if args.gate:
        return print_gate(args.gate, cross)
    return print_all(cross)


if __name__ == "__main__":
    raise SystemExit(main())
