"""Tests for the published dataset in data/.

Run:  python -m pytest tests/ -q

These assert what would otherwise drift silently: a row that left the allowlist,
a column that grew a target dimension, a withheld row that leaked a field, a
derived column that contradicts the columns it is derived from, summary.json or
the README quoting numbers the data no longer supports.

Expected values are read from data/summary.json, which stats.py --check proves is
current. The one hard-coded fact is the SHA-256 of rejections.csv: a data change
has to be deliberate, so it has to touch this file.
"""

from __future__ import annotations

import csv
import hashlib
import json
import re
import subprocess
import sys
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DATA = ROOT / "data"
CSV_PATH = DATA / "rejections.csv"
JSON_PATH = DATA / "rejections.json"
SUMMARY_PATH = DATA / "summary.json"
README = ROOT / "README.md"

sys.path.insert(0, str(ROOT / "scripts"))

import build_public_export as build  # noqa: E402
import vocab  # noqa: E402

#: SHA-256 of data/rejections.csv for this release. Update it in the same commit
#: as a data change, and say in the CHANGELOG which cells moved and why.
CSV_SHA256 = "fb95c686164285b9ea764d1d759741250962f77ff7215a1f38527373d6f049ea"


def rows() -> list[dict]:
    with CSV_PATH.open(encoding="utf-8", newline="") as fh:
        return list(csv.DictReader(fh))


def summary() -> dict:
    return json.loads(SUMMARY_PATH.read_text(encoding="utf-8"))


# ---------------------------------------------------------------- identity


def test_data_files_exist():
    for p in (CSV_PATH, JSON_PATH, SUMMARY_PATH, DATA / "provenance.json"):
        assert p.exists(), f"missing {p}"


def test_csv_digest_is_pinned():
    digest = hashlib.sha256(CSV_PATH.read_bytes()).hexdigest()
    assert digest == CSV_SHA256, (
        "data/rejections.csv changed. If that was deliberate, update CSV_SHA256 and the CHANGELOG."
    )


def test_row_count_matches_summary():
    assert len(rows()) == summary()["total"]


def test_published_data_passes_every_invariant():
    removed = json.loads((DATA / "provenance.json").read_text(encoding="utf-8"))["removed_ids"]
    assert build.validate_published(rows(), removed) == []


def test_check_mode_passes():
    assert build.check_existing(DATA) == 0


def test_json_matches_csv_cell_for_cell():
    json_rows = json.loads(JSON_PATH.read_text(encoding="utf-8"))
    assert json_rows == build.to_json_rows(rows())


def test_provenance_records_the_private_inputs():
    prov = json.loads((DATA / "provenance.json").read_text(encoding="utf-8"))
    assert build.validate_provenance(prov) == []
    for key in ("source_sha256", "classifier_sha256", "ledger_sha256"):
        assert prov[key], f"a release must record {key}"


# ---------------------------------------------------------------- schema


def test_expected_columns_exactly():
    assert list(rows()[0].keys()) == list(vocab.COLUMNS)


def test_no_target_dimension_and_no_free_text():
    assert not set(rows()[0].keys()) & vocab.FORBIDDEN_COLUMNS


def test_family_ids_are_opaque_and_dense():
    ids = [r["family_id"] for r in rows() if r["family_id"] != vocab.WITHHELD]
    assert ids[0] == "F-001"
    numeric = sorted(int(f[2:]) for f in set(ids))
    assert numeric == list(range(1, len(numeric) + 1))


def test_below_payout_floor_rows_exist_and_are_withheld():
    withheld = [r for r in rows() if r["triage_class"] == vocab.WITHHELD_TRIAGE]
    assert len(withheld) == summary()["withheld_rows"] > 0
    for r in withheld:
        assert r["mechanism_class"] == vocab.WITHHELD_MECHANISM
        assert all(r[f] == vocab.WITHHELD for f in vocab.WITHHELD_FIELDS)


# ---------------------------------------------------------------- totals


def test_every_distribution_matches_summary():
    s = summary()
    data = rows()
    for column, key in (
        ("triage_class", "triage"),
        ("earliest_gate", "gate"),
        ("bug_family", "bug_family"),
        ("mechanism_class", "mechanism_class"),
        ("duplicate_source", "duplicate_source"),
        ("preventable_by", "preventable_by"),
    ):
        assert dict(Counter(r[column] for r in data)) == s[key], column


def test_summary_keeps_every_0_5_key():
    # khomdev.org and other readers fetch these keys; renaming one breaks them.
    keys = {
        "total", "prehunt_catchable", "prehunt_catchable_pct", "withheld_rows", "effective_n",
        "repeat_rows", "repeated_families", "largest_family", "triage", "gate", "bug_family",
        "mechanism_class", "duplicate_source", "preventable_by", "analysis_split",
    }
    assert keys <= set(summary())


def test_prehunt_figure_is_its_definition():
    s = summary()
    assert s["prehunt_catchable"] == sum(1 for r in rows() if r["earliest_gate"] in vocab.PREHUNT_GATES)


def test_public_record_figure_is_its_definition():
    s = summary()
    assert s["public_record_preventable"] == s["preventable_by"]["public_record"]


def test_every_repeat_was_preventable_by_a_record():
    """The 0.5.1 bug: rows repeating the author's own family were labelled unknown."""
    for r in rows():
        if r["repeat_of_own"] == "true":
            assert r["preventable_by"] in {"public_record", "own_log"}, r["id"]


def test_analysis_split_definitions():
    s = summary()
    t = s["triage"]
    assert s["analysis_split"]["strict"] == t.get("spam", 0)
    assert s["analysis_split"]["broad"] == t.get("spam", 0) + t.get("informational", 0) + t.get(
        "intentional_design", 0
    )


# ---------------------------------------------------------------- README / SCHEMA


def test_readme_csv_examples_match_the_data():
    by_id = {r["id"]: r for r in rows()}
    checked = 0
    for line in README.read_text(encoding="utf-8").splitlines():
        if not re.match(r"^R-\d{3},", line.strip()):
            continue
        parsed = dict(zip(vocab.COLUMNS, next(csv.reader([line.strip()])), strict=True))
        assert parsed == by_id[parsed["id"]], parsed["id"]
        checked += 1
    assert checked >= 1


def test_schema_doc_tables_match_the_summary():
    s = summary()
    text = (DATA / "SCHEMA.md").read_text(encoding="utf-8")

    def section(heading: str, stop: str) -> str:
        start = text.index(heading)
        return text[start : text.index(stop, start)]

    for key, (heading, stop) in {
        "duplicate_source": ("### duplicate_source", "### preventable_by"),
        "preventable_by": ("### preventable_by", "### root_cause_class"),
    }.items():
        block = section(heading, stop)
        for value, count in s[key].items():
            row = re.search(rf"^\| `?{re.escape(value)}`? \|[^|]*\| (\d+) \|", block, re.M)
            assert row, f"SCHEMA.md has no row for {key} {value}"
            assert int(row.group(1)) == count, f"SCHEMA.md {key} {value}={row.group(1)}, data {count}"


def test_documented_vocabulary_sizes_match_the_code():
    assert len(vocab.BUG_FAMILIES) == 19
    assert len(vocab.MECHANISM_CLASSES) == 32
    assert len(vocab.VALID_MECHANISM) == 34
    assert len(vocab.TRIAGE_CLASSES) == 6
    assert len(vocab.ROOT_CAUSES) == 20
    assert len(vocab.VALID_GATES) == 17
    readme = README.read_text(encoding="utf-8")
    schema = (DATA / "SCHEMA.md").read_text(encoding="utf-8")
    for phrase in ("19 families", "32 classes, plus 2 sentinels", "16 gates, plus the", "20 reasons", "6 outcomes"):
        assert phrase in readme, phrase
    assert "34 members" in schema and "17 members" in schema
    used_mech = len({r["mechanism_class"] for r in rows()})
    used_gate = len({r["earliest_gate"] for r in rows()})
    assert f"**{used_mech} are used**" in schema
    assert f"**{used_gate} are\nused**" in schema or f"**{used_gate} are used**" in schema


def test_readme_generated_block_is_current():
    result = subprocess.run(
        [sys.executable, str(ROOT / "scripts" / "stats.py"), "--check"], capture_output=True, text=True, cwd=ROOT
    )
    assert result.returncode == 0, result.stdout + result.stderr


def test_readme_states_the_dataset_has_no_positive_class():
    assert "no positive class" in README.read_text(encoding="utf-8").lower()


def test_retracted_claims_stay_retracted():
    """Phrases that were published, found false, and removed. Each must not return."""
    banned = {
        "before reading a line of code": "most gates need the finding in hand",
        "before you open an editor": "most gates need the finding in hand",
        "before opening an editor": "most gates need the finding in hand",
        "runnable before reading any code": "most gates need the finding in hand",
        "catchable by six pre-hunt gates": "the 0.2.0 retraction; use preventable_by",
        "preventable by pre-hunt gates alone": "the 0.2.0 retraction",
        "largest sub-cause": "privileged-role is 1 of the out-of-scope rows",
        ".[].mechanism'": "the column is mechanism_class",
    }
    for path in [README, ROOT / "CITATION.cff", DATA / "SCHEMA.md", *sorted((ROOT / "docs").glob("*.md"))]:
        text = path.read_text(encoding="utf-8")
        for phrase, why in banned.items():
            assert phrase not in text, f"{path.name} contains {phrase!r}: {why}"
