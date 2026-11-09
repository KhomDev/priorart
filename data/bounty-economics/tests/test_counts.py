#!/usr/bin/env python3
"""Count consistency, and drift between the prose, the data and the schema.

Three things quote the same counts: the README, the taxonomy files, and the
worked example. They were not consistent — the README advertised "six fictional
entries, one per tier shape" for a file that has five — so these tests exist to
make the next drift fail loudly instead of quietly.
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

import pytest
import yaml

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "scripts"))

from validate import (  # noqa: E402
    CLAUSE_FILE,
    FLOOR_SEVERITIES,
    FLOOR_STATUSES,
    SCHEMA_VERSION,
    TIER_FILE,
    load_taxonomies,
)

TIERS, CLAUSES = load_taxonomies()

README = (ROOT / "README.md").read_text(encoding="utf-8")
SCHEMA = json.loads((ROOT / "schemas" / "ledger.schema.json").read_text(encoding="utf-8"))
EXAMPLE_SHAPES = ROOT / "examples" / "tier-shapes.example.yaml"
EXAMPLE_LEDGER = ROOT / "examples" / "ledger.example.yaml"

DATA_FILES = (TIER_FILE, CLAUSE_FILE, ROOT / "schemas" / "ledger.schema.json")


def example_entries() -> list[dict]:
    data = yaml.safe_load(EXAMPLE_SHAPES.read_text(encoding="utf-8"))
    assert isinstance(data, list), f"{EXAMPLE_SHAPES.name} must be a list of entries"
    return data


# ---------------------------------------------------------------- the example


def test_the_worked_example_has_one_entry_per_shape():
    shapes = [e["tier_shape"] for e in example_entries()]
    assert len(shapes) == len(TIERS) == 5, f"expected 5 entries, got {shapes}"
    assert sorted(shapes) == sorted(TIERS), shapes
    assert len(set(shapes)) == len(shapes), f"one entry per shape, got {shapes}"


def test_the_example_entry_count_matches_the_shape_count():
    assert len(example_entries()) == len(TIERS)


def test_the_example_contains_no_removed_any_shape():
    for e in example_entries():
        assert e["tier_shape"] != "any", "the removed `any` shape must not reappear"


def test_every_shipped_example_entry_records_a_floor_status():
    for path in (EXAMPLE_SHAPES, EXAMPLE_LEDGER):
        for e in yaml.safe_load(path.read_text(encoding="utf-8")):
            assert "floor_status" in e, f"{path.name}: {e['id']} has no floor_status"
            assert e["floor_status"] in FLOOR_STATUSES, e["floor_status"]


def test_the_shipped_examples_quote_their_dates():
    # An unquoted YAML date is a date object, and the JSON Schema declares a
    # string. The examples must set the example.
    for path in (EXAMPLE_SHAPES, EXAMPLE_LEDGER):
        for e in yaml.safe_load(path.read_text(encoding="utf-8")):
            assert isinstance(e["retrieved"], str), (
                f"{path.name}: {e['id']} has retrieved as {type(e['retrieved']).__name__}, "
                f"not a quoted string"
            )


def test_the_unstated_example_keeps_tier_shape_unknown():
    # The whole point of the field: an unstated floor is not a classification.
    for e in example_entries():
        if e.get("floor_status") == "unstated":
            assert e["tier_shape"] == "unknown", e
            assert e.get("floor_severity") is None, e


# ---------------------------------------------------------------- the README


def test_readme_quotes_the_real_counts():
    assert f"the {len(TIERS)} tier shapes" in README, "README must quote the tier-shape count"
    assert f"the {len(CLAUSES)} exclusion-clause classes" in README, (
        "README must quote the clause-class count"
    )


def test_readme_does_not_promise_more_entries_than_the_example_has():
    assert "six fictional" not in README.lower(), (
        "the README used to advertise six entries for a five-entry file"
    )


def test_readme_documents_the_floor_status_field():
    assert "floor_status" in README, "the unstated-floor rule must be documented"
    assert "unstated" in README


def test_readme_documents_the_version_matrix():
    assert "schema version" in README.lower()
    assert "viability-gate" in README


# The exact claims that were wrong. Named individually so a re-introduction is
# unambiguous rather than a vibe.
RETRACTED_CLAIMS = (
    "single largest category of wasted submissions industry-wide",
    "no floor statement at all",
    "never occurred in practice",
    "six fictional",
    "Also where a program lands when it documents **no floor at all**",
)


def test_the_retracted_claims_are_gone_from_the_readme():
    for claim in RETRACTED_CLAIMS:
        assert claim not in README, f"retracted claim is back: {claim!r}"


def test_the_taxonomies_make_no_unsupported_frequency_claim():
    for path in (TIER_FILE, CLAUSE_FILE):
        text = path.read_text(encoding="utf-8")
        for claim in ("never occurred in practice", "never occurs in the wild"):
            assert claim not in text, f"{path.name} still claims: {claim!r}"


def test_the_schema_makes_no_unsupported_frequency_claim():
    text = json.dumps(SCHEMA)
    assert "never occurred in practice" not in text


def test_readme_does_not_equate_an_unstated_floor_with_low_and_above():
    assert "If no floor is stated, the floor is `low`" not in README, (
        "an unstated floor is not evidence that Low pays"
    )


# ---------------------------------------------------------------- the schema


def test_the_schema_id_is_versioned():
    schema_id = SCHEMA["$id"]
    assert re.search(r"/v\d+/", schema_id), schema_id
    assert schema_id.endswith("/ledger.schema.json"), schema_id
    assert schema_id.startswith("https://github.com/KhomDev/bounty-economics/"), schema_id


def test_the_schema_declares_the_current_version():
    assert SCHEMA_VERSION == 1
    assert SCHEMA["items"]["properties"]["schema_version"]["const"] == SCHEMA_VERSION


def test_the_schema_tier_enum_matches_the_taxonomy():
    enum = SCHEMA["items"]["properties"]["tier_shape"]["enum"]
    assert enum == list(TIERS), "the schema enum and data/tier-shapes.yaml have drifted"


def test_the_schema_clause_enum_matches_the_taxonomy():
    enum = SCHEMA["items"]["properties"]["exclusions"]["items"]["properties"]["class"]["enum"]
    assert enum == list(CLAUSES), "the schema enum and data/clause-classes.yaml have drifted"


def test_the_schema_floor_status_enum_matches_the_validator():
    enum = SCHEMA["items"]["properties"]["floor_status"]["enum"]
    assert enum == list(FLOOR_STATUSES), enum


def test_the_schema_floor_severity_enum_matches_the_validator():
    enum = SCHEMA["items"]["properties"]["floor_severity"]["enum"]
    assert set(enum) == FLOOR_SEVERITIES, enum


def test_the_schema_required_fields_are_the_documented_ones():
    assert SCHEMA["items"]["required"] == ["id", "tier_shape", "source_url", "retrieved"]


def test_the_schema_is_closed_to_unknown_fields():
    # A field the schema does not know is a field nobody validates.
    assert SCHEMA["items"]["additionalProperties"] is False


def test_the_shipped_examples_validate_against_the_json_schema():
    # The validator is the reference implementation; the JSON Schema is the
    # published contract. Both have to accept the examples, or one of them is
    # lying about the format.
    jsonschema = pytest.importorskip("jsonschema")
    for path in sorted((ROOT / "examples").glob("*.yaml")):
        instance = yaml.safe_load(path.read_text(encoding="utf-8"))
        jsonschema.validate(instance=instance, schema=SCHEMA)


def test_the_json_schema_rejects_an_unknown_field():
    # A negative test, so the previous one cannot pass by accident.
    jsonschema = pytest.importorskip("jsonschema")
    bad = [
        {
            "id": "x",
            "tier_shape": "unknown",
            "source_url": "https://real.example.org/x",
            "retrieved": "2026-10-01",
            "smuggled": True,
        }
    ]
    with pytest.raises(jsonschema.ValidationError):
        jsonschema.validate(instance=bad, schema=SCHEMA)


def test_the_json_schema_rejects_a_floor_status_outside_the_enum():
    jsonschema = pytest.importorskip("jsonschema")
    bad = [
        {
            "id": "x",
            "tier_shape": "unknown",
            "floor_status": "probably",
            "source_url": "https://real.example.org/x",
            "retrieved": "2026-10-01",
        }
    ]
    with pytest.raises(jsonschema.ValidationError):
        jsonschema.validate(instance=bad, schema=SCHEMA)
