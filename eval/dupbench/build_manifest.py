#!/usr/bin/env python3
"""Builds eval/dupbench/{manifest.json,labels.json,cache.lock.json} offline.

Reads the pinned cache (report.md + data/*.json + label pages) and computes,
per contest: the judge's unique paid findings (High/Medium report sections),
the judge's duplicate grouping (GitHub issues labelled `duplicate-N` whose N is
a `selected for report` issue), invalid counts where published, and the
temporal split fixed in eval/P2R_PREREGISTRATION.md (T = 2025-09-01,
T2 = 2026-05-01).

No finding body is written to the committed files: issue numbers, label names,
counts and sha256 only.

Usage:
  python -I eval/dupbench/build_manifest.py
  python -I eval/dupbench/build_manifest.py --check
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
HERE = ROOT / "eval" / "dupbench"
CACHE = ROOT / ".corpus" / "dupbench"
PINS = CACHE / "repos"
LABELS = CACHE / "labels"
T, T2 = "2024-03-01", "2024-09-01"

SEC = re.compile(r'^## \[\[([HM])-(\d+)\]\s*(.*?)\]\((https://github\.com/[^)]+?/issues/(\d+))\)', re.S | re.M)


def sha_file(p: Path) -> str:
    return hashlib.sha256(p.read_bytes()).hexdigest()


def split_of(date: str | None) -> str:
    if not date:
        return "none"
    if date < T:
        return "train"
    if date < T2:
        return "validation"
    return "FINAL"


def load_labels(slug: str) -> list[dict]:
    d = LABELS / slug
    if not d.is_dir():
        return []
    issues = []
    for p in sorted(d.glob("page-*.json")):
        issues.extend(json.loads(p.read_text(encoding="utf-8")))
    return issues


def label_names(it: dict) -> list[str]:
    return [l["name"] for l in it.get("labels", [])]


def build() -> dict:
    src = json.loads((HERE / "sources.json").read_text(encoding="utf-8"))
    rows = []
    labels_doc = {"version": 1, "schema": "priorart.dupbench.labels/v1",
                  "note": "Judge grouping only: primary issue numbers and duplicate issue numbers. No text.",
                  "contests": {}}
    lock = {"version": 1, "schema": "priorart.dupbench.cache/v1", "api": "https://api.github.com",
            "contests": {}}
    for c in src["repos"]:
        slug = c["slug"]
        pin_dir = PINS / f"c4-{slug}"
        rp = pin_dir / "report.md"
        if not rp.is_file():
            raise SystemExit(f"missing {rp}")
        report = rp.read_text(encoding="utf-8", errors="replace")
        secs = [(m.group(1), m.group(2), m.group(5)) for m in SEC.finditer(report)]
        data_files = sorted((pin_dir / "data").glob("*.json")) if (pin_dir / "data").is_dir() else []
        data_ids = set()
        for p in data_files:
            try:
                data_ids.add(str(json.loads(p.read_text(encoding="utf-8")).get("issueId")))
            except Exception:
                pass
        join_hits = sum(1 for _, _, iid in secs if iid in data_ids)
        issues = load_labels(slug)
        by_num = {str(it["number"]): it for it in issues}
        prim = [str(it["number"]) for it in issues
                if "selected for report" in [n.lower() for n in label_names(it)]]
        prim_hm = [n for n in prim if any(nm.lower() in ("2 (med risk)", "3 (high risk)") for nm in label_names(by_num[n]))]
        dup_map = {n: [] for n in prim}
        dup_other = 0
        for it in issues:
            for nm in label_names(it):
                low = nm.lower()
                if low.startswith("duplicate-"):
                    target = nm.split("-", 1)[1]
                    if target in dup_map:
                        dup_map[target].append(str(it["number"]))
                    else:
                        dup_other += 1
                    break
        invalid = sum(1 for it in issues
                      if any(n.lower() in ("invalid", "out of scope") for n in label_names(it)))
        unique = len(secs)
        duplicates = sum(len(v) for v in dup_map.values())
        split = split_of(c["report_date"])
        labeled = bool(issues)
        # Pins: report, data listing, every label page. The bodies are fetched
        # from the pinned commit; the listing hash pins names and count without
        # reading every body (23k small files).
        data_listing = "\n".join(sorted(p.name for p in data_files))
        label_pages = sorted((LABELS / slug).glob("page-*.json"))
        row = {
            "slug": slug,
            "repo": c["repo"],
            "kind": c["kind"],
            "commit": c["commit"],
            "report_date": c["report_date"],
            "split": split,
            "paid_unique": unique,
            "duplicates": duplicates,
            "invalid": invalid,
            "invalid_source": "issue labels in the findings repo",
            "hm_sections": c["hm_sections"],
            "curated_jsons": len(data_files),
            "label_issues": len(issues),
            "join_report_to_data": f"{join_hits}/{len(secs)}",
            "duplicates_pointing_at_qa_or_missing": dup_other,
            "labels_status": "collected" if labeled else "not-collected",
        }
        rows.append(row)
        labels_doc["contests"][slug] = {
            "split": split,
            "primaries": sorted(prim_hm, key=int),
            "duplicates": {k: sorted(v, key=int) for k, v in dup_map.items() if v},
            "counts": {"paid_unique": unique, "duplicates": duplicates, "invalid": invalid},
        }
        lock["contests"][slug] = {
            "repo": c["repo"], "commit": c["commit"],
            "files": {"report.md": sha_file(rp),
                      "data_listing_sha256": hashlib.sha256(data_listing.encode()).hexdigest(),
                      "data_files": len(data_files)},
            "label_pages": {p.name: sha_file(p) for p in label_pages},
        }
    labeled_rows = [r for r in rows if r["labels_status"] == "collected"]
    splits = {}
    for s in ("train", "validation", "FINAL", "none"):
        rs = [r for r in labeled_rows if r["split"] == s]
        splits[s] = {"contests": len(rs),
                     "paid_unique": sum(r["paid_unique"] for r in rs),
                     "duplicates": sum(r["duplicates"] for r in rs),
                     "invalid": sum(r["invalid"] for r in rs)}
    joined = sum(int(r["join_report_to_data"].split("/")[0]) for r in rows)
    sections = sum(int(r["join_report_to_data"].split("/")[1]) for r in rows)
    manifest = {
        "version": 1,
        "schema": "priorart.dupbench.manifest/v1",
        "T": T, "T2": T2,
        "note": "Pointers and counts only. report.md and data/ are pinned by cache.lock.json; labels are issue numbers from GitHub labels. No finding body is committed.",
        "contests": rows,
        "counts": {
            "contests": len(rows),
            "contests_labeled": len(labeled_rows),
            "span": [min(r["report_date"] for r in rows), max(r["report_date"] for r in rows)],
            "by_split": splits,
            "join_report_to_data": f"{joined}/{sections}",
            "contests_with_grouping": sum(1 for r in labeled_rows if r["duplicates"] > 0),
        },
    }
    return {"manifest": manifest, "labels": labels_doc, "lock": lock}


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true")
    args = ap.parse_args(argv[1:])
    docs = build()
    outs = {"manifest.json": docs["manifest"], "labels.json": docs["labels"], "cache.lock.json": docs["lock"]}
    for name, doc in outs.items():
        text = json.dumps(doc, indent=1, ensure_ascii=False) + "\n"
        p = HERE / name
        if args.check:
            if not p.is_file() or p.read_text(encoding="utf-8") != text:
                print(f"{name} does not match the builder and the cache", file=sys.stderr)
                return 1
        else:
            p.write_text(text, encoding="utf-8", newline="\n")
    m = docs["manifest"]
    print(json.dumps(m["counts"], indent=1))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
