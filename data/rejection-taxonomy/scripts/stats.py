#!/usr/bin/env python3
"""Compute every published aggregate from data/rejections.csv.

The tables and the sample-size sentences between `<!-- BEGIN:generated -->` and
`<!-- END:generated -->` in README.md are produced by this script, and `--check`
fails if the README or data/summary.json has drifted from the data. Numbers that
appear in prose elsewhere are checked by tests/test_docs_numbers.py.

Usage:
    python scripts/stats.py              # rewrite data/summary.json and the README block
    python scripts/stats.py --check      # exit 1 if either would change
    python scripts/stats.py --json       # print the aggregate object and exit
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

from vocab import PREHUNT_GATES, TRIAGE_LABELS, WITHHELD  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
DATA = ROOT / "data"
CSV_PATH = DATA / "rejections.csv"
SUMMARY_PATH = DATA / "summary.json"
README = ROOT / "README.md"

BEGIN = "<!-- BEGIN:generated -->"
END = "<!-- END:generated -->"

#: "Did the analysis hold up?" STRICT: the impact itself was fabricated. BROAD:
#: that, plus no security impact and intentional design - in all three the
#: researcher was wrong about the behaviour being a vulnerability.
STRICT_ANALYSIS = {"spam"}
BROAD_ANALYSIS = {"spam", "informational", "intentional_design"}

#: The columns whose combination is reported as a re-identification measure.
TUPLE_COLUMNS = ("bug_family", "mechanism_class", "triage_class", "earliest_gate", "root_cause_class")


def rows() -> list[dict]:
    with CSV_PATH.open(encoding="utf-8", newline="") as fh:
        return list(csv.DictReader(fh))


def pct(n: int, total: int) -> str:
    return f"{100.0 * n / total:.1f}%"


def round_pct(n: int, total: int) -> float:
    return round(100.0 * n / total, 1)


def compute(data: list[dict]) -> dict:
    total = len(data)
    triage = Counter(r["triage_class"] for r in data)
    gate = Counter(r["earliest_gate"] for r in data)
    family = Counter(r["bug_family"] for r in data)
    mechanism = Counter(r["mechanism_class"] for r in data)
    dup_source = Counter(r["duplicate_source"] for r in data)
    preventable = Counter(r["preventable_by"] for r in data)

    published = [r for r in data if r["family_id"] != WITHHELD]
    fam_sizes = Counter(r["family_id"] for r in published)
    repeated_families = {f: n for f, n in fam_sizes.items() if n > 1}
    repeats = [r for r in data if r["repeat_of_own"] == "true"]

    strict = sum(triage.get(k, 0) for k in STRICT_ANALYSIS)
    broad = sum(triage.get(k, 0) for k in BROAD_ANALYSIS)
    prehunt = sum(n for g, n in gate.items() if g in PREHUNT_GATES)
    public_record = preventable.get("public_record", 0)

    used_mech = [m for m in mechanism if m not in {"other_mechanism", "withheld_valid"}]
    tuples = Counter(tuple(r[c] for c in TUPLE_COLUMNS) for r in data)

    # Key order is part of the published format: the keys up to analysis_split
    # are the 0.5.x keys, unchanged; 0.6.0 appends after them.
    return {
        "total": total,
        "prehunt_catchable": prehunt,
        "prehunt_catchable_pct": round_pct(prehunt, total),
        "withheld_rows": sum(1 for r in data if r["family_id"] == WITHHELD),
        "effective_n": len(fam_sizes),
        "repeat_rows": len(repeats),
        "repeated_families": len(repeated_families),
        "largest_family": max(fam_sizes.values()) if fam_sizes else 0,
        "triage": dict(triage.most_common()),
        "gate": dict(gate.most_common()),
        "bug_family": dict(family.most_common()),
        "mechanism_class": dict(mechanism.most_common()),
        "duplicate_source": dict(dup_source.most_common()),
        "preventable_by": dict(preventable.most_common()),
        "analysis_split": {
            "strict": strict,
            "strict_pct": round_pct(strict, total),
            "broad": broad,
            "broad_pct": round_pct(broad, total),
            "old_numerator_pct": round_pct(triage.get("informational", 0), total),
            "old_remainder_pct": round_pct(total - triage.get("informational", 0), total),
        },
        "published_rows": len(published),
        "rows_in_repeated_families": sum(repeated_families.values()),
        "public_record_preventable": public_record,
        "public_record_preventable_pct": round_pct(public_record, total),
        "duplicate_unknown": sum(
            1 for r in data if r["triage_class"] == "duplicate" and r["duplicate_source"] == "unknown"
        ),
        "root_cause_other": sum(1 for r in data if r["root_cause_class"] == "other"),
        "mechanism_classes_used": len(used_mech),
        "singleton_mechanism_classes": sum(1 for m in used_mech if mechanism[m] == 1),
        "unique_category_tuples": sum(1 for n in tuples.values() if n == 1),
    }


def summary_text(agg: dict) -> str:
    return json.dumps(agg, indent=2) + "\n"


def write_summary(data: list[dict], path: Path) -> dict:
    """THE writer of summary.json. build_public_export.py and this script both call it."""
    agg = compute(data)
    path.write_text(summary_text(agg), encoding="utf-8", newline="\n")
    return agg


def render(agg: dict) -> str:
    total = agg["total"]
    prev = agg["preventable_by"]
    out: list[str] = []
    a = out.append

    a("### Headline numbers")
    a("")
    a("| Triage outcome | Count | Share |")
    a("|---|---:|---:|")
    for key, n in agg["triage"].items():
        a(f"| {TRIAGE_LABELS.get(key, key)} | {n} | {pct(n, total)} |")
    a(f"| **Total** | **{total}** | **100%** |")
    a("")
    a(
        f"**{agg['public_record_preventable']} of {total} ({agg['public_record_preventable_pct']}%) "
        f"could have been stopped from the public record**: the program's own pages, its published "
        f"audits and known issues, or its test suite. Another {prev.get('own_log', 0)} could have been "
        f"stopped only by the author's own submission log. Most of these checks need the finding in "
        f"hand: a clause, an audit or a known issue covers something only once you know what you found."
    )
    a("")
    a("### What could have prevented each rejection")
    a("")
    a("| `preventable_by` | Count | Share |")
    a("|---|---:|---:|")
    for key, n in prev.items():
        a(f"| `{key}` | {n} | {pct(n, total)} |")
    a("")
    a("### Where they died")
    a("")
    a("| Earliest gate | Count | Share |")
    a("|---|---:|---:|")
    for key, n in agg["gate"].items():
        a(f"| `{key}` | {n} | {pct(n, total)} |")
    a("")
    a(
        f"The duplicate check plus G1-G5 - the six gates viability-gate runs before the proof of "
        f"concept - are the earliest gate for {agg['prehunt_catchable']} rows "
        f"({agg['prehunt_catchable_pct']}%). That is not the preventable share: it includes duplicate "
        f"rows whose signal the log does not record, a concurrent duplicate nobody could have "
        f"pre-empted, and the withheld rows. Use `preventable_by` above."
    )
    a("")
    a("### What kind of bug each row was")
    a("")
    a("| `mechanism_class` | Count |")
    a("|---|---:|")
    for key, n in agg["mechanism_class"].items():
        a(f"| `{key}` | {n} |")
    a("")
    a("### Where the duplicate signal lived")
    a("")
    a("| `duplicate_source` | Count |")
    a("|---|---:|")
    for key, n in agg["duplicate_source"].items():
        a(f"| `{key}` | {n} |")
    a("")
    a("### Did the analysis hold up?")
    a("")
    s = agg["analysis_split"]
    a("| Definition | Numerator | Count | Share |")
    a("|---|---|---:|---:|")
    a(f"| **strict** - the impact was fabricated | `spam` | {s['strict']} | {s['strict_pct']}% |")
    a(
        f"| **broad** - no impact, intentional design, or fabricated | "
        f"`spam` + `informational` + `intentional_design` | {s['broad']} | {s['broad_pct']}% |"
    )
    a("")
    a(
        f"An earlier version of this document used `informational` alone "
        f"({s['old_numerator_pct']}%) as the numerator and called the remaining "
        f"{s['old_remainder_pct']}% 'targeting'. That is not a definition: it is 100% "
        f"minus one bucket, and it counts intentional-design rows as targeting failures "
        f"when they are analysis failures. Both honest definitions are above."
    )
    a("")
    a("### Sample size and identifiability")
    a("")
    a(
        f"- **Effective n: {agg['effective_n']} distinct bug families**, over the "
        f"{agg['published_rows']} rows that publish a family ({agg['withheld_rows']} rows are "
        f"withheld and carry none). {agg['repeat_rows']} rows repeat a family the author had already "
        f"submitted; together with their first submissions that is {agg['rows_in_repeated_families']} "
        f"rows in {agg['repeated_families']} families (the largest has {agg['largest_family']} rows). "
        f"A family whose first submission predates the log cannot be detected, so this is a lower "
        f"bound on self-duplication."
    )
    a(
        f"- **Unknowns are reported, not guessed.** {agg['duplicate_unknown']} of the "
        f"{agg['triage'].get('duplicate', 0)} duplicate rows record no duplicate source, "
        f"{prev.get('unknown', 0)} rows have `preventable_by = unknown`, and "
        f"{agg['root_cause_other']} rows have the residual root cause `other`."
    )
    a(
        f"- **Identifiability.** {agg['mechanism_classes_used']} of the 32 mechanism classes are used, "
        f"{agg['singleton_mechanism_classes']} of them by a single row, and "
        f"{agg['unique_category_tuples']} of {total} rows are the only row with their combination of "
        f"bug family, mechanism class, triage outcome, earliest gate and root cause. A reader who "
        f"already knows a specific submission could recognise it. See the disclosure model below."
    )
    return "\n".join(out)


def apply_readme(block: str, check: bool) -> bool:
    """Rewrite the generated block. Return True if the file changed (or would)."""
    raw = README.read_bytes()
    text = raw.decode("utf-8")
    if BEGIN not in text or END not in text:
        print(f"ERROR: {README} has no {BEGIN} / {END} markers", file=sys.stderr)
        raise SystemExit(2)
    pattern = re.compile(re.escape(BEGIN) + r".*?" + re.escape(END), re.DOTALL)
    replacement = f"{BEGIN}\n\n{block}\n\n{END}"
    updated = pattern.sub(lambda _: replacement, text.replace("\r\n", "\n"), count=1)
    if updated.encode("utf-8") == raw:
        return False
    if not check:
        README.write_text(updated, encoding="utf-8", newline="\n")
    return True


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--check", action="store_true", help="exit 1 if anything is stale")
    ap.add_argument("--json", action="store_true", help="print aggregates and exit")
    args = ap.parse_args(argv)

    data = rows()
    agg = compute(data)

    if args.json:
        print(summary_text(agg), end="")
        return 0

    # Compare BYTES: a CRLF checkout is a real difference from what is published,
    # and read_text's newline translation would hide it.
    old_bytes = SUMMARY_PATH.read_bytes() if SUMMARY_PATH.exists() else b""
    summary_stale = old_bytes != summary_text(agg).encode("utf-8")
    block = render(agg)
    readme_stale = apply_readme(block, check=True)

    if args.check:
        if summary_stale:
            print("STALE: data/summary.json does not match the data", file=sys.stderr)
        if readme_stale:
            print("STALE: README generated block does not match the data", file=sys.stderr)
        if summary_stale or readme_stale:
            print(
                "  (on Windows, check `git ls-files --eol data/summary.json`: a CRLF checkout "
                "also reads as stale)",
                file=sys.stderr,
            )
            return 1
        print(f"OK: summary.json and README are current ({len(data)} rows)")
        return 0

    write_summary(data, SUMMARY_PATH)
    apply_readme(block, check=False)
    print(f"wrote data/summary.json and the README generated block ({len(data)} rows)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
