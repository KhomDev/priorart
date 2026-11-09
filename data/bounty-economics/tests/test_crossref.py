#!/usr/bin/env python3
"""The gate cross-reference stays valid and composes with the catalogs.

`data/gate-crossref.yaml` maps the shared gate vocabulary onto this repo's
catalogs. These tests make the next drift fail loudly: a gate token that is not
in the shared vocabulary, a catalog reference that does not resolve, a gate
listed in both halves, or a helper that errors on a gate it claims to know.
"""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "scripts"))

from crossref import CATALOGS, catalog_entries, load_crossref  # noqa: E402
from validate import load_taxonomies  # noqa: E402

# The shared gate vocabulary (rejection-taxonomy/data/SCHEMA.md). Hardcoded on
# purpose: this repo has no gate-vocabulary file, and a change to the shared
# vocabulary SHOULD require a deliberate edit here.
VALID_GATES = {
    "D1_duplicate_check",
    "G1_severity_floor",
    "G2_exclusion_clause",
    "G3_scope_tree",
    "G4_prior_audit",
    "G5_test_intent",
    "design_review",
    "program_ceiling",
    "P7_best_practice",
    "P12_no_persisted_impact",
    "P16_below_floor",
    "P17_no_attacker_win",
    "P18_design_tradeoff",
    "P19_headline_mismatch",
    "P20_cooperating_party",
    "REALISM",
}


@pytest.fixture(scope="module")
def cross() -> dict:
    return load_crossref()


def test_crossref_is_versioned(cross):
    assert cross.get("version") == 1


def test_every_gate_is_in_the_shared_vocabulary(cross):
    gates = set(cross.get("answered_here", {})) | set(cross.get("not_program_terms", {}))
    unknown = sorted(gates - VALID_GATES)
    assert not unknown, f"gate-crossref names token(s) outside the shared vocabulary: {unknown}"


def test_no_gate_is_in_both_halves(cross):
    both = set(cross.get("answered_here", {})) & set(cross.get("not_program_terms", {}))
    assert not both, f"gate(s) listed as both answered and not-answered: {sorted(both)}"


def test_answered_gates_name_a_real_catalog(cross):
    for gate, info in cross.get("answered_here", {}).items():
        assert "catalog" in info and "read" in info, f"{gate} is missing catalog/read"
        assert info["catalog"] in CATALOGS, f"{gate} names unknown catalog {info['catalog']!r}"


def test_catalog_references_resolve_to_real_entries(cross):
    tiers, clauses = load_taxonomies()
    sizes = {"tier-shapes": len(tiers), "clause-classes": len(clauses)}
    for gate, info in cross.get("answered_here", {}).items():
        catalog = info["catalog"]
        if catalog in sizes:
            assert sizes[catalog] > 0, f"{gate} -> {catalog} resolves to no entries"
            assert catalog_entries(catalog), f"{gate} -> {catalog} returned no ids"


def test_the_two_core_mappings_are_present(cross):
    """The plan's load-bearing pair: G1 -> tier-shapes, G2 -> clause-classes."""
    answered = cross.get("answered_here", {})
    assert answered.get("G1_severity_floor", {}).get("catalog") == "tier-shapes"
    assert answered.get("G2_exclusion_clause", {}).get("catalog") == "clause-classes"


def test_helper_runs_for_every_known_gate(cross):
    script = ROOT / "scripts" / "crossref.py"
    gates = list(cross.get("answered_here", {})) + list(cross.get("not_program_terms", {}))
    for gate in gates:
        result = subprocess.run(
            [sys.executable, str(script), gate],
            capture_output=True,
            text=True,
            cwd=ROOT,
        )
        assert result.returncode == 0, f"crossref.py {gate} failed: {result.stderr}"

    # And an unknown gate is a non-zero exit, so a typo cannot pass silently.
    bad = subprocess.run(
        [sys.executable, str(script), "NOT_A_GATE"],
        capture_output=True,
        text=True,
        cwd=ROOT,
    )
    assert bad.returncode != 0
