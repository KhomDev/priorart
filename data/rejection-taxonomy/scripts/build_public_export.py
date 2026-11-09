#!/usr/bin/env python3
"""Build data/ from a vocabulary-only classified file, and verify data/ without one.

NEVER hand-edit data/. Run this script.

TWO STAGES
----------
The dataset is derived from a private rejection log in two stages, and only the
second is in this repository.

1. **Private classifier** (not published). Reads the private log - protocol
   names, report titles, sponsor correspondence - and assigns each row a value
   from each closed vocabulary in `scripts/vocab.py`. Its rules necessarily quote
   private text, which is why it is not here. It keys rows by a stable hash, so
   inserting or reordering rows in the private log cannot shift a classification
   onto the wrong row. Its output is a CSV of vocabulary values only:

       id, family, bug_family, mechanism_class, triage_class, earliest_gate,
       root_cause_class, dup_signal

   plus a provenance file with the SHA-256 of the private source, the classifier
   and its row ledger.

2. **This script** (public). Validates every value against the allowlist,
   applies the withholding rule, derives family_id, repeat_of_own,
   duplicate_source and preventable_by from vocabulary values alone (the rules
   are in `vocab.py`, readable and tested), checks the invariants, and writes
   data/. It never sees private text, so none can reach the output.

Usage:
    # build (needs the private classifier's output)
    python scripts/build_public_export.py --classified classified.PRIVATE.csv \
        --provenance provenance.PRIVATE.json --out data/

    # the same on the synthetic fixture, which CI does
    python scripts/build_public_export.py --classified tests/fixtures/classified.example.csv \
        --provenance tests/fixtures/provenance.example.json --out /tmp/rt

    # verify data/ with no private input
    python scripts/build_public_export.py --check --out data/

Exit codes: 0 ok, 1 validation failed (nothing written), 2 cannot run.
"""

from __future__ import annotations

import argparse
import csv
import json
import re
import sys
from collections import Counter
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import vocab  # noqa: E402
from stats import write_summary  # noqa: E402
from vocab import (  # noqa: E402
    ALLOWED,
    COLUMNS,
    DUP_SIGNALS,
    FORBIDDEN_COLUMNS,
    PUBLIC_DUP_SIGNALS,
    PUBLIC_RECORD_GATES,
    WITHHELD,
    WITHHELD_FIELDS,
    WITHHELD_MECHANISM,
    WITHHELD_TRIAGE,
)

INTERMEDIATE_COLUMNS: tuple[str, ...] = (
    "id", "family", "bug_family", "mechanism_class", "triage_class",
    "earliest_gate", "root_cause_class", "dup_signal",
)
PROVENANCE_KEYS: tuple[str, ...] = ("source_sha256", "classifier_sha256", "ledger_sha256", "removed_ids")

ROW_ID_RE = re.compile(r"^R-\d{3}$")
FAMILY_ID_RE = re.compile(r"^F-\d{3}$")
SHA256_RE = re.compile(r"^[0-9a-f]{64}$")

#: Belt and braces on top of the allowlist: shapes that identify a target or an
#: internal artifact. With every cell drawn from a vocabulary none can occur, so
#: a hit means the allowlist itself was widened by mistake.
BANNED_PATTERNS: tuple[str, ...] = (
    r"https?://",
    r"\b[0-9a-f]{7,40}\b",  # commit hashes
    r"\b20\d{2}-\d{2}-\d{2}\b",  # dates
    r"/",  # paths
)


def rid(n: int) -> str:
    return f"R-{n:03d}"


def id_number(row_id: str) -> int:
    return int(row_id[2:])


# --------------------------------------------------------------------------
# Reading and validating the intermediate
# --------------------------------------------------------------------------


def read_classified(path: Path) -> list[dict]:
    with path.open(encoding="utf-8", newline="") as fh:
        reader = csv.DictReader(fh)
        if tuple(reader.fieldnames or ()) != INTERMEDIATE_COLUMNS:
            raise SystemExit(
                f"ERROR: {path} columns are {reader.fieldnames}, expected {list(INTERMEDIATE_COLUMNS)}"
            )
        return list(reader)


def read_provenance(path: Path | None) -> dict:
    if path is None:
        return {"source_sha256": None, "classifier_sha256": None, "ledger_sha256": None, "removed_ids": []}
    data = json.loads(path.read_text(encoding="utf-8"))
    problems = validate_provenance(data)
    if problems:
        raise SystemExit("ERROR: provenance invalid:\n  " + "\n  ".join(problems))
    return data


def validate_provenance(data: dict) -> list[str]:
    problems = []
    if sorted(data) != sorted(PROVENANCE_KEYS):
        problems.append(f"keys are {sorted(data)}, expected {sorted(PROVENANCE_KEYS)}")
        return problems
    for key in ("source_sha256", "classifier_sha256", "ledger_sha256"):
        if data[key] is not None and not SHA256_RE.match(str(data[key])):
            problems.append(f"{key} is not a SHA-256 hex digest")
    removed = data["removed_ids"]
    if not isinstance(removed, list) or not all(isinstance(r, str) and ROW_ID_RE.match(r) for r in removed):
        problems.append("removed_ids must be a list of R-### ids")
    return problems


def validate_classified(rows: list[dict], removed: list[str]) -> list[str]:
    """Every problem with the intermediate. Empty means it can be published."""
    problems: list[str] = []
    ids = [r["id"] for r in rows]
    for r in rows:
        if not ROW_ID_RE.match(r["id"]):
            problems.append(f"{r['id']!r}: bad row id")
    if problems:
        return problems
    if len(set(ids)) != len(ids):
        problems.append("duplicate row ids")
    if ids != sorted(ids, key=id_number):
        problems.append("rows are not in id order")
    present = set(ids) | set(removed)
    if set(ids) & set(removed):
        problems.append(f"ids both published and removed: {sorted(set(ids) & set(removed))}")
    top = max((id_number(i) for i in present), default=0)
    gaps = [rid(n) for n in range(1, top + 1) if rid(n) not in present]
    if gaps:
        problems.append(f"ids are not dense; missing {gaps[:10]} (a removed row must be listed in removed_ids)")

    by_id = {r["id"]: r for r in rows}
    for r in rows:
        for column in ("bug_family", "mechanism_class", "triage_class", "earliest_gate", "root_cause_class"):
            if r[column] not in ALLOWED[column]:
                problems.append(f"{r['id']}: {column} {r[column]!r} is not in the vocabulary")
        if r["dup_signal"] not in DUP_SIGNALS:
            problems.append(f"{r['id']}: dup_signal {r['dup_signal']!r} is not in the vocabulary")
        if r["mechanism_class"] == WITHHELD_MECHANISM and r["triage_class"] != WITHHELD_TRIAGE:
            problems.append(f"{r['id']}: {WITHHELD_MECHANISM} on a row that is not {WITHHELD_TRIAGE}")
        root = by_id.get(r["family"])
        if root is None:
            problems.append(f"{r['id']}: family {r['family']!r} is not a row in this file")
            continue
        if root["family"] != root["id"]:
            problems.append(f"{r['id']}: family root {root['id']} is itself in family {root['family']}")
        if id_number(root["id"]) > id_number(r["id"]):
            problems.append(f"{r['id']}: family root {root['id']} comes after it")
        if r["triage_class"] != WITHHELD_TRIAGE and root["triage_class"] == WITHHELD_TRIAGE:
            problems.append(f"{r['id']}: family root {root['id']} is a withheld row")
        # A family is one bug: one bug family and one mechanism class.
        if r["triage_class"] != WITHHELD_TRIAGE:
            for column in ("bug_family", "mechanism_class"):
                if r[column] != root[column]:
                    problems.append(
                        f"{r['id']}: {column} {r[column]} differs from its family root {root['id']} ({root[column]})"
                    )
    return problems


# --------------------------------------------------------------------------
# Deriving the published rows
# --------------------------------------------------------------------------


def build_rows(rows: list[dict]) -> list[dict]:
    """Classified rows -> published rows. Assumes validate_classified passed."""
    published = [r for r in rows if r["triage_class"] != WITHHELD_TRIAGE]
    # Family ids in first-seen order over the PUBLISHED rows: a withheld row
    # consumes no id, so the sequence has no tell-tale gap.
    family_id: dict[str, str] = {}
    for r in published:
        if r["family"] not in family_id:
            family_id[r["family"]] = f"F-{len(family_id) + 1:03d}"

    out: list[dict] = []
    for r in rows:
        if r["triage_class"] == WITHHELD_TRIAGE:
            row = dict.fromkeys(COLUMNS, WITHHELD)
            row.update(
                id=r["id"],
                bug_family=r["bug_family"],
                mechanism_class=WITHHELD_MECHANISM,
                triage_class=r["triage_class"],
                earliest_gate=r["earliest_gate"],
                root_cause_class=r["root_cause_class"],
            )
            out.append(row)
            continue
        repeat = r["family"] != r["id"]
        out.append(
            {
                "id": r["id"],
                "family_id": family_id[r["family"]],
                "bug_family": r["bug_family"],
                "mechanism_class": r["mechanism_class"],
                "triage_class": r["triage_class"],
                "earliest_gate": r["earliest_gate"],
                "root_cause_class": r["root_cause_class"],
                "repeat_of_own": "true" if repeat else "false",
                "duplicate_source": vocab.derive_duplicate_source(r["triage_class"], r["dup_signal"], repeat),
                "preventable_by": vocab.derive_preventable_by(r["earliest_gate"], r["dup_signal"], repeat),
            }
        )
    return out


# --------------------------------------------------------------------------
# Validating published rows (used by the build AND by --check)
# --------------------------------------------------------------------------


def validate_published(rows: list[dict], removed: list[str] | None = None) -> list[str]:
    problems: list[str] = []
    if not rows:
        return ["no rows"]
    if list(rows[0].keys()) != list(COLUMNS):
        problems.append(f"columns are {list(rows[0].keys())}, expected {list(COLUMNS)}")
    leaked = FORBIDDEN_COLUMNS & set(rows[0].keys())
    if leaked:
        problems.append(f"schema exposes forbidden columns: {sorted(leaked)}")
    if problems:
        return problems

    ids = [r["id"] for r in rows]
    if not all(ROW_ID_RE.match(i) for i in ids):
        return ["bad row id(s)"]
    if len(set(ids)) != len(ids) or ids != sorted(ids, key=id_number):
        problems.append("row ids are duplicated or out of order")
    present = set(ids) | set(removed or [])
    top = max(id_number(i) for i in present)
    gaps = [rid(n) for n in range(1, top + 1) if rid(n) not in present]
    if gaps:
        problems.append(f"row ids are not dense; missing {gaps[:10]} and not listed as removed")

    families_seen: dict[str, str] = {}
    next_family = 1
    for r in rows:
        tag = r["id"]
        for column, allowed in ALLOWED.items():
            if r[column] not in allowed:
                problems.append(f"{tag}: {column} {r[column]!r} is not in the allowed vocabulary")
        blob = json.dumps(r)
        for pattern in BANNED_PATTERNS:
            if re.search(pattern, blob, re.IGNORECASE):
                problems.append(f"{tag}: banned pattern {pattern}")

        if r["triage_class"] == WITHHELD_TRIAGE:
            if r["mechanism_class"] != WITHHELD_MECHANISM:
                problems.append(f"{tag}: withheld row must ship {WITHHELD_MECHANISM}")
            for field in WITHHELD_FIELDS:
                if r[field] != WITHHELD:
                    problems.append(f"{tag}: withheld row leaks {field}={r[field]!r}")
            continue

        # Published rows carry no withheld value at all.
        for field in COLUMNS:
            if r[field] == WITHHELD or r[field] == WITHHELD_MECHANISM:
                problems.append(f"{tag}: {field} is {r[field]!r} on a row that is not withheld")
        if not FAMILY_ID_RE.match(r["family_id"]):
            problems.append(f"{tag}: bad family_id {r['family_id']!r}")
            continue

        # Family ids open in first-seen order; repeat_of_own marks every later member.
        fid = r["family_id"]
        if fid not in families_seen:
            if fid != f"F-{next_family:03d}":
                problems.append(f"{tag}: family {fid} opens out of order (expected F-{next_family:03d})")
            next_family += 1
            families_seen[fid] = tag
            if r["repeat_of_own"] != "false":
                problems.append(f"{tag}: first row of {fid} must have repeat_of_own=false")
        elif r["repeat_of_own"] != "true":
            problems.append(f"{tag}: later row of {fid} must have repeat_of_own=true")

        # duplicate_source is set exactly on duplicate rows.
        if (r["triage_class"] == "duplicate") == (r["duplicate_source"] == "not_duplicate"):
            problems.append(f"{tag}: duplicate_source {r['duplicate_source']} contradicts triage {r['triage_class']}")
        # preventable_by follows from the other columns wherever they determine it.
        repeat = r["repeat_of_own"] == "true"
        if r["duplicate_source"] in PUBLIC_DUP_SIGNALS or r["earliest_gate"] in PUBLIC_RECORD_GATES:
            if r["preventable_by"] != "public_record":
                problems.append(f"{tag}: a public-record signal or gate, but preventable_by={r['preventable_by']}")
        elif (repeat or r["duplicate_source"] == "own_prior") and r["preventable_by"] != "own_log":
            problems.append(f"{tag}: repeats the author's own family, but preventable_by={r['preventable_by']}")
        if r["duplicate_source"] == "unknown" and repeat:
            problems.append(f"{tag}: a duplicate that repeats the author's own family cannot be 'unknown'")

    # One bug per family: one bug_family and one mechanism_class.
    for fid in families_seen:
        members = [r for r in rows if r["family_id"] == fid]
        for column in ("bug_family", "mechanism_class"):
            if len({m[column] for m in members}) > 1:
                problems.append(f"{fid}: members disagree on {column}")
    return problems


# --------------------------------------------------------------------------
# Writing
# --------------------------------------------------------------------------


def to_json_rows(rows: list[dict]) -> list[dict]:
    out = []
    for r in rows:
        obj = dict(r)
        if obj["repeat_of_own"] in {"true", "false"}:
            obj["repeat_of_own"] = obj["repeat_of_own"] == "true"
        out.append(obj)
    return out


def write_text_lf(path: Path, text: str) -> None:
    # newline="\n": Windows text mode would emit CRLF and the byte-exact checks
    # would then fail on the author's own machine.
    path.write_text(text, encoding="utf-8", newline="\n")


def write_outputs(rows: list[dict], provenance: dict, out: Path, aggregates_only: bool) -> None:
    out.mkdir(parents=True, exist_ok=True)
    csv_path, json_path = out / "rejections.csv", out / "rejections.json"
    if aggregates_only:
        # Drop the per-row tuples entirely, including any left from an earlier build.
        for stale in (csv_path, json_path):
            stale.unlink(missing_ok=True)
    else:
        with csv_path.open("w", encoding="utf-8", newline="") as fh:
            writer = csv.DictWriter(fh, fieldnames=list(COLUMNS), lineterminator="\n")
            writer.writeheader()
            writer.writerows(rows)
        write_text_lf(json_path, json.dumps(to_json_rows(rows), indent=2) + "\n")
    write_text_lf(out / "provenance.json", json.dumps(provenance, indent=2) + "\n")
    # summary.json has ONE writer, stats.write_summary, shared with scripts/stats.py.
    write_summary(rows, out / "summary.json")


# --------------------------------------------------------------------------
# --check
# --------------------------------------------------------------------------


def read_csv_rows(path: Path) -> list[dict]:
    with path.open(encoding="utf-8", newline="") as fh:
        return list(csv.DictReader(fh))


def check_existing(out: Path) -> int:
    """Re-validate data/ on disk, with no private input."""
    csv_path = out / "rejections.csv"
    if not csv_path.exists():
        print(f"ERROR: {csv_path} not found", file=sys.stderr)
        return 2
    rows = read_csv_rows(csv_path)
    problems: list[str] = []

    prov_path = out / "provenance.json"
    removed: list[str] = []
    if not prov_path.exists():
        problems.append("provenance.json is missing")
    else:
        prov = json.loads(prov_path.read_text(encoding="utf-8"))
        problems += [f"provenance: {p}" for p in validate_provenance(prov)]
        removed = prov.get("removed_ids") or []

    problems += validate_published(rows, removed)

    json_path = out / "rejections.json"
    if not json_path.exists():
        problems.append("rejections.json is missing")
    elif json.loads(json_path.read_text(encoding="utf-8")) != to_json_rows(rows):
        problems.append("rejections.json does not match rejections.csv cell for cell")

    if problems:
        print(f"FAIL: {len(problems)} problem(s)", file=sys.stderr)
        for p in problems[:60]:
            print(f"  {p}", file=sys.stderr)
        return 1
    print(f"OK: {len(rows)} rows, allowlist clean, invariants hold, no target dimension, no free text")
    return 0


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--classified", type=Path, help="the private classifier's vocabulary-only CSV")
    ap.add_argument("--provenance", type=Path, help="the private classifier's provenance JSON")
    ap.add_argument("--out", type=Path, default=Path("data"))
    ap.add_argument("--check", action="store_true", help="verify an existing export; needs no private input")
    ap.add_argument(
        "--aggregates-only",
        action="store_true",
        help=(
            "publish only summary.json and provenance.json, and delete any per-row files in --out. "
            "The headline numbers are unaffected."
        ),
    )
    args = ap.parse_args(argv)

    if args.check:
        return check_existing(args.out)
    if not args.classified:
        ap.error("--classified is required unless --check is given")

    provenance = read_provenance(args.provenance)
    classified = read_classified(args.classified)
    problems = validate_classified(classified, provenance["removed_ids"])
    if not problems:
        public = build_rows(classified)
        problems = validate_published(public, provenance["removed_ids"])
    if problems:
        print(f"\nREFUSING TO WRITE: {len(problems)} problem(s).\n", file=sys.stderr)
        for p in problems[:60]:
            print(f"  {p}", file=sys.stderr)
        print("\nNo files were written.", file=sys.stderr)
        return 1

    write_outputs(public, provenance, args.out, args.aggregates_only)
    triage = Counter(r["triage_class"] for r in public)
    print(f"wrote {len(public)} rows to {args.out}" + (" (aggregates only)" if args.aggregates_only else ""))
    for key, value in triage.most_common():
        print(f"  {key:<22} {value:>4}  ({value / len(public):.1%})")
    print("\n  NOTE: no protocol, chain, ecosystem, date, severity or free text is published.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
