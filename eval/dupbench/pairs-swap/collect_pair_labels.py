#!/usr/bin/env python3
"""Collects the judge grouping for the V1 pair contests, the Dup-Bench way.

For every pair in pairs_v1_swapped.json whose contest is not a Dup-Bench contest
it fetches the findings repository's issue pages with `gh api`
(`issues?state=all&per_page=100&page=N`, at least 7 s between calls, resumable)
into `<out>/<slug>/page-N.json`, then writes `<out>/labels.json` with the same
rules as build_manifest.py:

* primaries: issues labelled `selected for report` that also carry a
  `2 (med risk)` or `3 (high risk)` label;
* duplicates: issues labelled `duplicate-N` whose N is a `selected for report` issue;
* invalid: issues labelled `invalid` or `out of scope` (a count only).

The labels are the Code4rena judge's, not a human or AI rater's: nothing here
judges anything. Issue text stays in `<out>` (never committed); `pair-labels.lock.json`
holds the page sha256s and counts. `--rebuild` recomputes labels.json from the
pages without the network.

Usage:
  python -I eval/dupbench/pairs-swap/collect_pair_labels.py --out DIR [--rebuild]
"""
from __future__ import annotations

import argparse, hashlib, json, subprocess, sys, time
from pathlib import Path

HERE = Path(__file__).resolve().parent
DB = HERE.parent


def sha(p: Path) -> str:
    return hashlib.sha256(p.read_bytes()).hexdigest()


def names(it: dict) -> list[str]:
    return [l["name"] for l in it.get("labels", [])]


def fetch(repo: str, d: Path) -> None:
    d.mkdir(parents=True, exist_ok=True)
    page = 1
    while True:
        out = d / f"page-{page}.json"
        if out.is_file():
            if len(json.loads(out.read_text(encoding="utf-8"))) < 100:
                return
            page += 1
            continue
        time.sleep(7)
        r = subprocess.run(["gh", "api", f"repos/{repo}/issues?state=all&per_page=100&page={page}"],
                           capture_output=True, text=True, encoding="utf-8", errors="replace")
        if r.returncode != 0:
            raise SystemExit(f"{repo} page {page}: {r.stderr.strip()}")
        out.write_text(r.stdout, encoding="utf-8")
        n = len(json.loads(r.stdout))
        print(f"{repo} page {page}: {n}", flush=True)
        if n < 100:
            return
        page += 1


def grouping(d: Path) -> tuple[dict, dict]:
    issues = []
    for p in sorted(d.glob("page-*.json"), key=lambda p: int(p.stem.split("-")[1])):
        issues.extend(json.loads(p.read_text(encoding="utf-8")))
    issues = [i for i in issues if "pull_request" not in i]
    by = {str(i["number"]): i for i in issues}
    prim = [n for n, i in by.items() if "selected for report" in [x.lower() for x in names(i)]]
    prim_hm = [n for n in prim if any(x.lower() in ("2 (med risk)", "3 (high risk)") for x in names(by[n]))]
    dup = {n: [] for n in prim}
    other = 0
    for i in issues:
        for nm in names(i):
            if nm.lower().startswith("duplicate-"):
                t = nm.split("-", 1)[1]
                if t in dup:
                    dup[t].append(str(i["number"]))
                else:
                    other += 1
                break
    invalid = sum(1 for i in issues if any(x.lower() in ("invalid", "out of scope") for x in names(i)))
    ndup = sum(len(v) for v in dup.values())
    lab = {"primaries": sorted(prim_hm, key=int),
           "duplicates": {k: sorted(v, key=int) for k, v in dup.items() if v},
           "counts": {"issues": len(issues), "paid_unique": len(prim_hm), "duplicates": ndup, "invalid": invalid,
                      "duplicates_pointing_at_non_primary": other, "selected_for_report_any_severity": len(prim)}}
    return lab, {p.name: sha(p) for p in sorted(d.glob("page-*.json"))}


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", required=True)
    ap.add_argument("--pairs", default=str(HERE / "pairs_v1_swapped.json"))
    ap.add_argument("--rebuild", action="store_true")
    a = ap.parse_args()
    out = Path(a.out)
    manifest = json.loads((DB / "manifest.json").read_text(encoding="utf-8"))
    dupbench = {c["slug"] for c in manifest["contests"]}
    doc = {"version": 1, "schema": "priorart.dupbench.labels/v1",
           "note": "Judge grouping (Code4rena GitHub labels) for the V1 pair contests. No text.", "contests": {}}
    lock = {"version": 1, "contests": {}}
    for pair in json.loads(Path(a.pairs).read_text(encoding="utf-8"))["pairs"]:
        slug, repo = pair["contest"]["slug"], pair["contest"]["repo"]
        if slug in dupbench:
            print(f"{slug}: Dup-Bench contest, skipped (labels are in labels.json of eval/dupbench)")
            continue
        if not a.rebuild:
            fetch(repo, out / slug)
        lab, pages = grouping(out / slug)
        doc["contests"][slug] = lab
        lock["contests"][slug] = {"repo": repo, "pins_findings_commit": pair["contest"]["commit"], "pages": pages}
        print(slug, lab["counts"])
    (out / "labels.json").write_text(json.dumps(doc, indent=1, sort_keys=True) + "\n", encoding="utf-8", newline="\n")
    (out / "pair-labels.lock.json").write_text(json.dumps(lock, indent=1, sort_keys=True) + "\n", encoding="utf-8", newline="\n")
    print("labels.json sha256", sha(out / "labels.json"))
    return 0


if __name__ == "__main__":
    sys.exit(main())