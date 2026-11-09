"""The public build, end to end, on a synthetic fixture - plus every way --check must fail.

The fixture (tests/fixtures/classified.example.csv) is invented. It exists so the
whole build runs in CI with no private input, and each of its 14 rows pins one
derivation rule.
"""

from __future__ import annotations

import csv
import json
import shutil
import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "scripts"))

import build_public_export as build  # noqa: E402
import vocab  # noqa: E402

FIXTURES = ROOT / "tests" / "fixtures"
CLASSIFIED = FIXTURES / "classified.example.csv"
PROVENANCE = FIXTURES / "provenance.example.json"

#: id -> (family_id, repeat_of_own, duplicate_source, preventable_by)
EXPECTED = {
    "R-001": ("F-001", "false", "known_issue", "public_record"),  # public signal
    "R-002": ("F-002", "false", "not_duplicate", "public_record"),  # public-record gate
    "R-003": ("withheld", "withheld", "withheld", "withheld"),  # below the payout floor
    "R-004": ("F-003", "false", "not_duplicate", "none"),  # needed the finding
    "R-005": ("F-003", "true", "own_prior", "own_log"),  # repeat, log names no source
    "R-006": ("F-004", "false", "concurrent_submission", "none"),  # nobody could pre-empt
    "R-007": ("F-005", "false", "not_duplicate", "none"),
    "R-008": ("F-006", "false", "known_issue", "public_record"),  # known issue off the D1 gate
    "R-009": ("F-007", "false", "not_duplicate", "public_record"),  # D1 gate, not a duplicate
    "R-010": ("F-008", "false", "not_duplicate", "unknown"),  # no gate recognised
    "R-011": ("F-003", "true", "not_duplicate", "public_record"),  # repeat, but G3 is cheaper
    "R-012": ("F-009", "false", "unknown", "unknown"),  # the log does not say
    "R-013": ("F-010", "false", "not_duplicate", "none"),
    "R-014": ("F-011", "false", "own_prior", "own_log"),  # the log names an own earlier submission
}


def run_build(out: Path, *extra: str, classified: Path = CLASSIFIED) -> int:
    return build.main(["--classified", str(classified), "--provenance", str(PROVENANCE), "--out", str(out), *extra])


def read(out: Path) -> list[dict]:
    with (out / "rejections.csv").open(encoding="utf-8", newline="") as fh:
        return list(csv.DictReader(fh))


def write_rows(out: Path, rows: list[dict]) -> None:
    with (out / "rejections.csv").open("w", encoding="utf-8", newline="") as fh:
        w = csv.DictWriter(fh, fieldnames=list(rows[0].keys()), lineterminator="\n")
        w.writeheader()
        w.writerows(rows)
    (out / "rejections.json").write_text(json.dumps(build.to_json_rows(rows), indent=2) + "\n", encoding="utf-8")


@pytest.fixture
def built(tmp_path: Path) -> Path:
    assert run_build(tmp_path) == 0
    return tmp_path


# ------------------------------------------------------------------ happy path


def test_fixture_builds_with_the_expected_derivations(built):
    rows = {r["id"]: r for r in read(built)}
    assert list(rows) == list(EXPECTED)
    for rid, (fid, repeat, dup, prev) in EXPECTED.items():
        r = rows[rid]
        assert (r["family_id"], r["repeat_of_own"], r["duplicate_source"], r["preventable_by"]) == (
            fid,
            repeat,
            dup,
            prev,
        ), rid


def test_withheld_row_ships_only_the_five_fields(built):
    r = {r["id"]: r for r in read(built)}["R-003"]
    assert r["mechanism_class"] == vocab.WITHHELD_MECHANISM
    assert (r["bug_family"], r["triage_class"], r["earliest_gate"], r["root_cause_class"]) == (
        "fees",
        "below_payout_floor",
        "G1_severity_floor",
        "below_severity_floor",
    )
    for field in vocab.WITHHELD_FIELDS:
        assert r[field] == vocab.WITHHELD


def test_built_output_passes_check(built):
    assert build.check_existing(built) == 0


def test_build_is_deterministic_and_lf(tmp_path):
    a, b = tmp_path / "a", tmp_path / "b"
    assert run_build(a) == 0 and run_build(b) == 0
    for name in ("rejections.csv", "rejections.json", "summary.json", "provenance.json"):
        data = (a / name).read_bytes()
        assert data == (b / name).read_bytes(), name
        assert b"\r\n" not in data, f"{name} has CRLF"


def test_provenance_is_published_verbatim(built):
    assert json.loads((built / "provenance.json").read_text()) == json.loads(PROVENANCE.read_text())


def test_aggregates_only_drops_per_row_files_even_from_an_earlier_build(built):
    assert (built / "rejections.csv").exists()
    assert run_build(built, "--aggregates-only") == 0
    assert not (built / "rejections.csv").exists()
    assert not (built / "rejections.json").exists()
    summary = json.loads((built / "summary.json").read_text())
    assert summary["total"] == len(EXPECTED)


# ------------------------------------------------------------------ refusals


def mutate_classified(tmp_path: Path, edit) -> Path:
    with CLASSIFIED.open(encoding="utf-8", newline="") as fh:
        rows = list(csv.DictReader(fh))
    edit(rows)
    path = tmp_path / "classified.csv"
    with path.open("w", encoding="utf-8", newline="") as fh:
        w = csv.DictWriter(fh, fieldnames=list(build.INTERMEDIATE_COLUMNS), lineterminator="\n")
        w.writeheader()
        w.writerows(rows)
    return path


def by_id(rows, rid):
    return next(r for r in rows if r["id"] == rid)


@pytest.mark.parametrize(
    "edit",
    [
        pytest.param(lambda rows: by_id(rows, "R-002").update(mechanism_class="free text"), id="value-outside-vocabulary"),
        pytest.param(lambda rows: by_id(rows, "R-002").update(dup_signal="maybe"), id="bad-dup-signal"),
        pytest.param(lambda rows: rows.pop(5), id="id-gap"),
        pytest.param(lambda rows: by_id(rows, "R-005").update(family="R-011"), id="family-root-after-row"),
        pytest.param(lambda rows: by_id(rows, "R-005").update(bug_family="oracle"), id="family-incoherent"),
        pytest.param(lambda rows: by_id(rows, "R-005").update(family="R-099"), id="family-root-missing"),
        pytest.param(lambda rows: by_id(rows, "R-002").update(mechanism_class="withheld_valid"), id="withheld-mech-on-public-row"),
        pytest.param(lambda rows: rows.reverse(), id="out-of-order"),
    ],
)
def test_build_refuses_and_writes_nothing(tmp_path, edit):
    bad = mutate_classified(tmp_path, edit)
    out = tmp_path / "out"
    assert run_build(out, classified=bad) == 1
    assert not out.exists() or not any(out.iterdir())


def test_a_removed_row_is_allowed_only_when_listed(tmp_path):
    bad = mutate_classified(tmp_path, lambda rows: rows.pop(5))  # drops R-006
    rows = build.read_classified(bad)
    assert build.validate_classified(rows, removed=[])
    assert build.validate_classified(rows, removed=["R-006"]) == []


@pytest.mark.parametrize(
    "edit, message",
    [
        (lambda rows: rows[1].update(preventable_by="none"), "public-record"),
        (lambda rows: rows[4].update(preventable_by="unknown"), "own family"),
        (lambda rows: rows[4].update(repeat_of_own="false"), "repeat_of_own=true"),
        (lambda rows: rows[0].update(duplicate_source="not_duplicate"), "contradicts triage"),
        (lambda rows: rows[2].update(family_id="F-099"), "leaks family_id"),
        (lambda rows: rows[3].update(duplicate_source="withheld"), "not withheld"),
        (lambda rows: rows[1].update(bug_family="chain_x"), "allowed vocabulary"),
        (lambda rows: rows[1].update(family_id="F-009"), "opens out of order"),
    ],
)
def test_check_catches_a_tampered_export(built, edit, message):
    rows = read(built)
    edit(rows)
    write_rows(built, rows)
    problems = build.validate_published(rows)
    assert any(message in p for p in problems), problems
    assert build.check_existing(built) == 1


def test_check_catches_a_forbidden_column(built):
    rows = read(built)
    for r in rows:
        r["chain"] = "x"
    assert any("forbidden" in p for p in build.validate_published(rows))


def test_check_catches_json_drift(built):
    data = json.loads((built / "rejections.json").read_text())
    data[0]["mechanism_class"] = "replay_scope"
    (built / "rejections.json").write_text(json.dumps(data), encoding="utf-8")
    assert build.check_existing(built) == 1


def test_check_requires_provenance(built):
    (built / "provenance.json").unlink()
    assert build.check_existing(built) == 1


def test_check_on_an_empty_csv_fails(tmp_path):
    (tmp_path / "rejections.csv").write_text(",".join(vocab.COLUMNS) + "\n", encoding="utf-8")
    assert build.check_existing(tmp_path) == 1


def test_check_does_not_create_the_out_directory(tmp_path):
    missing = tmp_path / "nope"
    assert build.main(["--check", "--out", str(missing)]) == 2
    assert not missing.exists()


@pytest.mark.parametrize(
    "data",
    [
        {"source_sha256": "abc", "classifier_sha256": None, "ledger_sha256": None, "removed_ids": []},
        {"source_sha256": None, "classifier_sha256": None, "ledger_sha256": None, "removed_ids": ["7"]},
        {"source_sha256": None},
    ],
)
def test_provenance_validation(data):
    assert build.validate_provenance(data)


# ------------------------------------------------------------------ the rules themselves


@pytest.mark.parametrize(
    "triage, signal, repeat, expected",
    [
        ("informational", "none", False, "not_duplicate"),
        ("duplicate", "known_issue", True, "known_issue"),
        ("duplicate", "unknown", True, "own_prior"),
        ("duplicate", "unknown", False, "unknown"),
        ("duplicate", "concurrent_submission", False, "concurrent_submission"),
    ],
)
def test_derive_duplicate_source(triage, signal, repeat, expected):
    assert vocab.derive_duplicate_source(triage, signal, repeat) == expected


@pytest.mark.parametrize(
    "gate, signal, repeat, expected",
    [
        ("design_review", "known_issue", False, "public_record"),
        ("G4_prior_audit", "none", True, "public_record"),
        ("P17_no_attacker_win", "none", True, "own_log"),
        ("D1_duplicate_check", "own_prior", False, "own_log"),
        ("D1_duplicate_check", "concurrent_submission", False, "none"),
        ("D1_duplicate_check", "unknown", False, "unknown"),
        ("UNCLASSIFIED", "none", False, "unknown"),
        ("REALISM", "none", False, "none"),
    ],
)
def test_derive_preventable_by(gate, signal, repeat, expected):
    assert vocab.derive_preventable_by(gate, signal, repeat) == expected


def test_no_text_classifier_ships_in_this_repository():
    """The rules that read private text belong to the private stage. If one of their
    tables reappears in a public script, the 0.5.1 disclosure is back."""
    names = ("MECHANISM_RULES", "ROW_BUG_FAMILY", "FAMILY_GROUPS", "ROW_OWNER_OVERRIDE", "DUPLICATE_SOURCE_MAP", "ROOT_CAUSE_MAP")
    for path in (ROOT / "scripts").glob("*.py"):
        text = path.read_text(encoding="utf-8")
        for name in names:
            assert f"{name}:" not in text and f"{name} =" not in text, f"{path.name} defines {name}"


def test_fixture_copy_is_independent(tmp_path):
    # Guard for the fixture itself: it must stay a valid build input.
    shutil.copy(CLASSIFIED, tmp_path / "c.csv")
    assert build.validate_classified(build.read_classified(tmp_path / "c.csv"), []) == []
