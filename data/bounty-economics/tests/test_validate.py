#!/usr/bin/env python3
"""Tests for the ledger validator.

Run:
    python -m pytest tests/ -q
    python tests/test_validate.py          # no pytest required

These pin the behaviours that matter and keep the taxonomies in sync with the
validator. A validator that silently accepts an uncited floor is worse than no
validator, because you will trust the entry.
"""

from __future__ import annotations

import json
import sys
from datetime import date
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "scripts"))

from validate import (  # noqa: E402
    CLAUSE_FILE,
    TIER_FILE,
    load_taxonomies,
    main,
    validate_entry,
)

TIERS, CLAUSES = load_taxonomies()

EXAMPLE_LEDGER = ROOT / "examples" / "ledger.example.yaml"


def entry(**over: object) -> dict:
    """A minimal valid ledger entry, with overrides."""
    base = {
        "id": "test-program",
        "tier_shape": "medium_and_above",
        "floor_status": "documented",
        "floor_severity": "medium",
        "source_url": "https://real.example.org/rewards",
        "retrieved": "2026-10-01",
    }
    base.update(over)
    return base


def check(e: dict, example_mode: bool = False) -> tuple[list[str], list[str]]:
    return validate_entry(e, 0, TIERS, CLAUSES, example_mode=example_mode)


# ---------------------------------------------------------------- taxonomies


def test_taxonomy_files_exist():
    assert TIER_FILE.exists(), f"missing {TIER_FILE}"
    assert CLAUSE_FILE.exists(), f"missing {CLAUSE_FILE}"


def test_five_tier_shapes():
    assert len(TIERS) == 5, f"expected 5 tier shapes, got {sorted(TIERS)}"


def test_no_any_shape():
    # `any` was removed: a program documenting no severity floor is
    # low_and_above, because informational is acknowledged rather than paid.
    assert "any" not in TIERS, "the `any` shape must not come back"


def test_low_and_above_is_the_widest_shape():
    widest = max(TIERS.values(), key=lambda s: len(s.get("pays") or []))
    assert widest["id"] == "low_and_above", widest["id"]
    assert widest["floor"] == "low"


def test_no_shape_has_an_informational_floor():
    for tid, spec in TIERS.items():
        assert spec.get("floor") != "informational", (
            f"{tid} declares an informational floor, which no program pays"
        )


def test_fourteen_clause_classes():
    assert len(CLAUSES) == 14, f"expected 14 clause classes, got {len(CLAUSES)}"


def test_unknown_is_the_only_empty_paying_shape():
    for tid, spec in TIERS.items():
        pays = spec.get("pays") or []
        if tid == "unknown":
            assert pays == [], "unknown must pay nothing (it is not permissive)"
        else:
            assert pays, f"{tid} must declare what it pays"


def test_every_clause_class_has_an_escape_field():
    for cid, spec in CLAUSES.items():
        assert spec.get("kills"), f"{cid} does not say what it kills"
        # `escape` may be None (known_issue has no escape) but the key should exist.
        assert "escape" in spec, f"{cid} is missing the 'escape' key"


# ---------------------------------------------------------------- citation


def test_valid_entry_passes():
    errors, _ = check(entry())
    assert errors == [], errors


def test_missing_source_url_fails():
    e = entry()
    del e["source_url"]
    errors, _ = check(e)
    assert any("source_url" in x for x in errors), errors


def test_missing_retrieved_fails():
    e = entry()
    del e["retrieved"]
    errors, _ = check(e)
    assert any("retrieved" in x for x in errors), errors


def test_unquoted_yaml_date_is_accepted_but_warned_about():
    # YAML resolves an unquoted 2026-10-01 to a date object. The ledger format
    # declares a string, so the validator accepts it (str()) but says so, because
    # the JSON Schema would reject it.
    errors, warnings = check(entry(retrieved=date(2026, 10, 1)))
    assert errors == [], errors
    assert any("unquoted" in w for w in warnings), warnings


def test_bad_date_format_fails():
    errors, _ = check(entry(retrieved="01/10/2026"))
    assert any("YYYY-MM-DD" in x for x in errors), errors


def test_placeholder_host_fails_in_a_real_ledger():
    errors, _ = check(entry(source_url="https://example.invalid/rewards"))
    assert any("placeholder" in x for x in errors), errors


def test_placeholder_host_only_warns_in_example_mode():
    errors, warnings = check(
        entry(source_url="https://example.invalid/rewards"), example_mode=True
    )
    assert errors == [], errors
    assert any("placeholder" in w for w in warnings), warnings


# ---------------------------------------------------------------- tier shape


def test_unknown_tier_shape_warns_not_permissive():
    _, warnings = check(entry(tier_shape="unknown", floor_severity=None))
    assert any("NOT permissive" in w for w in warnings), warnings


def test_critical_only_warns():
    _, warnings = check(entry(tier_shape="critical_only", floor_severity="critical"))
    assert any("pays only" in w for w in warnings), warnings


def test_bad_tier_shape_fails():
    errors, _ = check(entry(tier_shape="pays_everything"))
    assert any("tier_shape" in x for x in errors), errors


def test_removed_any_shape_gives_a_migration_not_a_bare_error():
    # Someone with an old ledger should be told the fix, not just that the value
    # is wrong. The message must name the replacement.
    errors, _ = check(entry(tier_shape="any", floor_severity="low"))
    assert errors, "the removed `any` value must fail validation"
    joined = " ".join(errors)
    assert "low_and_above" in joined, f"error should name the replacement: {errors}"


def test_informational_floor_is_rejected():
    # No program pays informational in practice, so it is not a real floor.
    errors, _ = check(entry(tier_shape="low_and_above", floor_severity="informational"))
    assert any("informational" in x for x in errors), errors
    assert any("low" in x for x in errors), f"error should suggest 'low': {errors}"


def test_low_and_above_accepts_floor_low():
    errors, _ = check(entry(tier_shape="low_and_above", floor_severity="low"))
    assert errors == [], errors


def test_floor_conflict_is_caught():
    # tier_shape implies medium, floor_severity says high.
    _, warnings = check(entry(tier_shape="medium_and_above", floor_severity="high"))
    assert any("implies floor" in w for w in warnings), warnings


# ---------------------------------------------------------------- floor_status
#
# The defect these pin: "the page states no floor" transcribed as "Low pays".
# An unstated floor is not evidence about the program; it is evidence the page
# has not been read, so it must keep tier_shape at 'unknown'.


def test_unstated_floor_beside_low_and_above_is_an_error():
    errors, _ = check(
        entry(tier_shape="low_and_above", floor_severity=None, floor_status="unstated")
    )
    assert errors, "an unstated floor beside low_and_above must not validate"
    joined = " ".join(errors)
    assert "not evidence that Low pays" in joined, joined


def test_unstated_floor_with_unknown_shape_warns():
    errors, warnings = check(
        entry(tier_shape="unknown", floor_severity=None, floor_status="unstated")
    )
    assert errors == [], errors
    assert any("unstated" in w for w in warnings), warnings


def test_unstated_floor_may_not_carry_a_severity():
    errors, _ = check(
        entry(tier_shape="unknown", floor_severity="low", floor_status="unstated")
    )
    assert any("unstated" in x for x in errors), errors


def test_documented_floor_requires_a_severity():
    errors, _ = check(
        entry(tier_shape="unknown", floor_severity=None, floor_status="documented")
    )
    assert any("documented" in x for x in errors), errors


def test_documented_floor_conflicts_with_unknown_shape():
    errors, _ = check(
        entry(tier_shape="unknown", floor_severity="low", floor_status="documented")
    )
    assert any("unknown" in x for x in errors), errors


def test_conflicting_floor_warns():
    errors, warnings = check(
        entry(tier_shape="unknown", floor_severity=None, floor_status="conflicting")
    )
    assert errors == [], errors
    assert any("disagree" in w for w in warnings), warnings


def test_missing_floor_status_warns():
    # Optional in the schema for backwards compatibility, but a ledger that omits
    # it cannot distinguish a floor that was read from one that was assumed.
    e = entry()
    del e["floor_status"]
    errors, warnings = check(e)
    assert errors == [], errors
    assert any("floor_status is missing" in w for w in warnings), warnings


def test_bad_floor_status_fails():
    errors, _ = check(entry(floor_status="probably"))
    assert any("floor_status" in x for x in errors), errors


def test_floor_status_is_optional_for_old_ledgers():
    # A ledger written before floor_status existed still validates, with a
    # warning rather than a hard failure: the fix is to add the field.
    e = entry()
    del e["floor_status"]
    errors, _ = check(e)
    assert errors == [], errors


# ---------------------------------------------------------------- schema_version


def test_schema_version_one_is_accepted():
    errors, _ = check(entry(schema_version=1))
    assert errors == [], errors


def test_unknown_schema_version_is_rejected():
    errors, _ = check(entry(schema_version=99))
    assert any("schema_version" in x for x in errors), errors


# ---------------------------------------------------------------- exclusions


def test_bad_clause_class_fails():
    e = entry(exclusions=[{"class": "not_a_class", "clause": "x"}])
    errors, _ = check(e)
    assert any("not_a_class" in x for x in errors), errors


def test_exclusion_without_clause_fails():
    e = entry(exclusions=[{"class": "privileged_role"}])
    errors, _ = check(e)
    assert any("verbatim" in x for x in errors), errors


def test_invalid_regex_fails():
    e = entry(
        exclusions=[{"class": "privileged_role", "clause": "x", "pattern": "(unclosed"}]
    )
    errors, _ = check(e)
    assert any("invalid regex" in x for x in errors), errors


def test_valid_exclusion_passes():
    e = entry(
        exclusions=[
            {
                "class": "privileged_role",
                "clause": "Attacks requiring access to privileged addresses",
                "pattern": "(owner|admin|capability)",
            }
        ]
    )
    errors, _ = check(e)
    assert errors == [], errors


# ---------------------------------------------------------------- prior audits


def test_prior_audit_without_covers_warns():
    e = entry(prior_audits=[{"firm": "Example Firm"}])
    _, warnings = check(e)
    assert any("covers" in w for w in warnings), warnings


def test_prior_audit_without_firm_fails():
    e = entry(prior_audits=[{"covers": ["sources/Vault.sol"]}])
    errors, _ = check(e)
    assert any("firm" in x for x in errors), errors


# ---------------------------------------------------------------- --format json


def test_json_format_reports_a_valid_ledger(capsys):
    code = main(["--format", "json", str(EXAMPLE_LEDGER)])
    payload = json.loads(capsys.readouterr().out)
    assert code == 0, payload
    assert payload["ok"] is True
    assert payload["schema_version"] == 1
    assert payload["entries"] == 1
    assert payload["errors"] == []
    assert payload["warnings"], "the shipped example warns about its placeholder host"


def test_json_format_reports_a_failing_ledger(capsys, tmp_path):
    bad = tmp_path / "my-ledger.yaml"
    bad.write_text(
        "- id: real-program\n"
        "  tier_shape: critical_only\n"
        "  floor_status: documented\n"
        "  floor_severity: critical\n"
        "  source_url: https://example.invalid/rewards\n"
        "  retrieved: 2026-10-01\n",
        encoding="utf-8",
    )
    code = main(["--format", "json", str(bad)])
    payload = json.loads(capsys.readouterr().out)
    assert code == 1
    assert payload["ok"] is False
    assert payload["errors"], "a placeholder host must be an error outside example mode"


def test_json_format_reports_a_missing_file(capsys):
    code = main(["--format", "json", "does-not-exist.yaml"])
    payload = json.loads(capsys.readouterr().out)
    assert code == 2
    assert payload["ok"] is False
    assert "not found" in payload["error"]


def test_json_taxonomy_is_machine_readable(capsys):
    code = main(["--taxonomy", "--format", "json"])
    payload = json.loads(capsys.readouterr().out)
    assert code == 0
    assert payload["tier_shapes"]["count"] == 5
    assert payload["clause_classes"]["count"] == 14
    assert [s["id"] for s in payload["tier_shapes"]["items"]] == list(TIERS)


def test_json_format_is_the_same_verdict_as_text(capsys):
    # The machine output must not be a second implementation with its own rules.
    text_code = main([str(EXAMPLE_LEDGER)])
    capsys.readouterr()
    json_code = main(["--format", "json", str(EXAMPLE_LEDGER)])
    capsys.readouterr()
    assert text_code == json_code == 0


def test_several_ledgers_in_one_run(capsys):
    # The README and CI invoke this with a shell glob
    # (python scripts/validate.py examples/*.yaml), so more than one path has to
    # be a supported invocation and not an argparse error.
    shapes = ROOT / "examples" / "tier-shapes.example.yaml"
    code = main([str(EXAMPLE_LEDGER), str(shapes)])
    out = capsys.readouterr().out
    assert code == 0
    assert "6 entries" in out, out
    assert str(shapes) in out


def test_several_ledgers_report_json_per_file(capsys):
    shapes = ROOT / "examples" / "tier-shapes.example.yaml"
    code = main(["--format", "json", str(EXAMPLE_LEDGER), str(shapes)])
    payload = json.loads(capsys.readouterr().out)
    assert code == 0
    assert payload["ok"] is True
    assert payload["entries"] == 6
    assert [r["entries"] for r in payload["ledgers"]] == [1, 5]
    assert "ledger" not in payload, "the single-ledger key must not be ambiguous"


def test_a_failure_in_one_of_several_ledgers_fails_the_run(capsys, tmp_path):
    shapes = ROOT / "examples" / "tier-shapes.example.yaml"
    bad = tmp_path / "my-ledger.yaml"
    bad.write_text(
        "- id: real-program\n"
        "  tier_shape: critical_only\n"
        "  floor_status: documented\n"
        "  floor_severity: critical\n"
        "  source_url: https://example.invalid/rewards\n"
        "  retrieved: 2026-10-01\n",
        encoding="utf-8",
    )
    code = main(["--format", "json", str(shapes), str(bad)])
    payload = json.loads(capsys.readouterr().out)
    assert code == 1
    assert payload["ok"] is False
    assert any(str(bad) in e for e in payload["errors"]), payload["errors"]


def test_strict_promotes_warnings_in_json_too(capsys):
    code = main(["--format", "json", "--strict", str(EXAMPLE_LEDGER)])
    payload = json.loads(capsys.readouterr().out)
    assert code == 1
    assert payload["ok"] is False
    assert payload["errors"] == []
    assert payload["warnings"]


# ---------------------------------------------------------------- runner


def main_runner() -> int:
    tests = [
        (n, f)
        for n, f in sorted(globals().items())
        if n.startswith("test_") and callable(f) and not n.startswith("test_json")
        and not n.startswith("test_strict")
    ]
    failed = 0
    for name, fn in tests:
        try:
            fn()
            print(f"  ok    {name}")
        except AssertionError as err:
            failed += 1
            print(f"  FAIL  {name}: {err}")
        except Exception as err:  # report the failure, do not abort the runner
            failed += 1
            print(f"  ERROR {name}: {type(err).__name__}: {err}")
    print(f"\n{len(tests) - failed}/{len(tests)} passed")
    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main_runner())


# ---------------------------------------------------------------- 0.5.0 audit fixes
#
# Each test below pins a defect reproduced with the 0.4.1 validator during the
# 2026-11-06 audit.


def test_an_unknown_key_is_an_error():
    # REPRODUCED: `exclusionz:` was accepted, so the exclusions silently vanished.
    errors, _ = check(entry(exclusionz=[]))
    assert any("exclusionz" in x and "schema" in x for x in errors), errors


def test_an_unknown_nested_key_is_an_error():
    errors, _ = check(
        entry(exclusions=[{"class": "privileged_role", "clause": "c", "patern": "admin"}])
    )
    assert any("patern" in x for x in errors), errors


def test_a_wrong_type_is_an_error():
    errors, _ = check(entry(conflicts="one string, not a list"))
    assert any("conflicts" in x for x in errors), errors


def test_schema_errors_are_not_duplicated_for_a_missing_citation():
    e = entry()
    del e["source_url"]
    errors, _ = check(e)
    assert len([x for x in errors if "source_url" in x]) == 1, errors


@pytest.mark.parametrize(
    "pattern",
    ["(?<!un)privileged", "(?=admin)owner", "(owner)\\1", "(?P<r>owner)(?P=r)", "a++"],
)
def test_a_pattern_viability_gate_cannot_compile_is_an_error(pattern):
    # REPRODUCED: a lookbehind passed here and was "invalid regex" in vg's G2.
    errors, _ = check(
        entry(exclusions=[{"class": "privileged_role", "clause": "c", "pattern": pattern}])
    )
    assert any("exclusions[0].pattern" in x for x in errors), errors


def test_every_suggested_pattern_is_in_the_common_dialect():
    from validate import regex_problem

    for cid, spec in CLAUSES.items():
        pattern = spec.get("suggested_pattern")
        if pattern:
            assert regex_problem(pattern) is None, (cid, regex_problem(pattern))


@pytest.mark.parametrize(
    "url",
    [
        "https://github.com/acme/todo-list-protocol/blob/main/BOUNTY.md",
        "https://docs.real-program.org/todo/rewards",
        "https://myexample.com/rewards",
    ],
)
def test_a_real_url_containing_a_placeholder_word_is_not_a_placeholder(url):
    # REPRODUCED: "todo" anywhere in the URL was an error.
    errors, _ = check(entry(source_url=url))
    assert not any("placeholder" in x for x in errors), errors


@pytest.mark.parametrize(
    "url",
    [
        "https://example.invalid/rewards",
        "https://rewards.example.invalid/",
        "http://localhost:8000/rewards",
        "https://todo/rewards",
        "https://example.com/rewards",
    ],
)
def test_a_placeholder_host_is_caught_by_host(url):
    errors, _ = check(entry(source_url=url))
    assert any("placeholder" in x for x in errors), errors


def test_a_future_retrieved_date_is_an_error():
    future = (date.today().replace(year=date.today().year + 1)).isoformat()
    errors, _ = check(entry(retrieved=future))
    assert any("future" in x for x in errors), errors


def test_a_conflicting_floor_beside_a_settled_shape_is_an_error():
    # The README says a conflicting floor keeps tier_shape 'unknown'; vg's G1
    # fails it. The validator now agrees instead of only warning.
    errors, _ = check(entry(tier_shape="medium_and_above", floor_status="conflicting"))
    assert any("conflicting" in x for x in errors), errors


def test_the_any_migration_message_distinguishes_explicit_from_absent():
    errors, _ = check(entry(tier_shape="any"))
    joined = " ".join(errors)
    assert "low_and_above" in joined and "unstated" in joined, joined


def test_the_schema_and_the_readme_agree_on_an_explicit_none_floor():
    # REPRODUCED: the README said "SEVERITY FLOOR: NONE" is documented +
    # low_and_above while the schema said it is unknown + unstated.
    schema = json.loads((ROOT / "schemas" / "ledger.schema.json").read_text(encoding="utf-8"))
    shape_doc = schema["items"]["properties"]["tier_shape"]["description"]
    assert "EXPLICITLY states 'SEVERITY FLOOR: NONE' or 'any' is 'low_and_above'" in shape_doc
    readme = (ROOT / "README.md").read_text(encoding="utf-8")
    assert "| `SEVERITY FLOOR: NONE` or `SEVERITY FLOOR: any`, in those words | `documented` | `low_and_above` |" in readme
