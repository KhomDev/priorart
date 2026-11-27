#!/usr/bin/env python3
"""priorart pairs runner (P2R-2, eval only): recall@5, FAR at the shipped alarm,
warning precision and abstention, per contest, for the six sealed pairs
(`eval/dupbench/pairs.json`, V1 only) or for Dup-Bench train contests (the DEV
proof run).

Method (eval/P2R2_ANALYSIS_PLAN.md section 2), with the shipped ranker
(`weights/v1.json`) and alarm (`weights/alarm-v1.json`), prose anchors OFF:

* the prior corpus is the contest's paid-unique primaries (judge grouping),
  prepared once per contest with `priorart candidates --prepare`;
* every judged duplicate is a query (expected: its judged primary); every
  primary is a query against the other primaries (expected: nothing);
* recall@5: duplicate queries whose judged primary is among the top five, over
  all duplicate queries attempted (abstentions count as misses and are also
  reported on their own);
* FAR: primary queries on which the alarm fires, over primary queries attempted;
* warning precision: among queries on which the alarm fires, the share whose top
  candidate is the query's judged primary (the alarm-v1.json definition: any
  alarm on a primary is false);
* abstention: abstained queries over queries attempted.

Wilson 95% intervals (z = 1.959964); a cell with n < 5 is reported as a count.

Refusals (checked before any contest is read):
* a contest in the Dup-Bench FINAL split is never run, in any mode;
* `--mode dev` runs only Dup-Bench **train** contests;
* `--mode pairs` requires `--round`, `--version` and `--binary-sha256`, and the
  sha256 must be the sha256 of `--bin` (a placeholder is refused). A pair whose
  contest is also a Dup-Bench train or validation contest is run but reported as
  in-sample, outside the V1 cells; it is never pooled with fresh pairs.

Deterministic: no timestamps, no absolute paths, sorted keys. `--check FILE`
recomputes and fails (exit 1) unless the result is byte-identical to FILE.
No network. No LLM. Third-party issue text stays in the gitignored cache/output.

Usage:
  python -I eval/dupbench/pairs_run.py --mode dev --contest 2023-01-drips --out target/pairs-dev.json
  python -I eval/dupbench/pairs_run.py --mode pairs --round 1 --version r1 \\
      --binary-sha256 <sha256 of --bin> --pair-labels DIR --out eval/rounds/round-1-pairs.json
"""
from __future__ import annotations

import argparse
import hashlib
import json
import math
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
sys.path.insert(0, str(HERE))
import build_ranker_data as brd  # noqa: E402  (shared, reviewed helpers)

Z = 1.959964
TOP = 5
SCHEMA = "priorart.pairs-run/v1"


def wilson(k: int, n: int) -> list[float] | None:
    if n == 0:
        return None
    p = k / n
    denom = 1 + Z * Z / n
    centre = (p + Z * Z / (2 * n)) / denom
    half = Z * math.sqrt(p * (1 - p) / n + Z * Z / (4 * n * n)) / denom
    return [round(max(0.0, centre - half), 4), round(min(1.0, centre + half), 4)]


def cell(k: int, n: int) -> dict:
    out = {"k": k, "n": n}
    if n >= 5:
        out["rate"] = round(k / n, 4)
        out["wilson95"] = wilson(k, n)
    else:
        out["count_only"] = True
    return out


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def dupbench_splits(manifest: dict) -> dict[str, str]:
    return {c["slug"]: c["split"] for c in manifest["contests"]}


def run_contest(slug: str, entry: dict, labels: dict, issues: dict, args, out_dir: Path) -> dict:
    """One contest: returns counts, or a skip reason. Mirrors build_ranker_data's
    checkout checks so an incomplete checkout is never measured."""
    checkout = brd.find_checkout(slug, args.source_dirs)
    if checkout is None:
        return {"skip": "checkout-missing"}
    if brd.git_head(checkout).lower() != entry["commit"].lower():
        return {"skip": "head-mismatch"}
    if brd.git_deleted_files(checkout).strip() or brd.git_tracked_count(checkout) <= 0:
        return {"skip": "checkout-incomplete"}
    primaries = sorted((str(n) for n in labels.get("primaries", [])), key=int)
    duplicates = labels.get("duplicates", {})
    primary_texts = {}
    for number in primaries:
        text = brd.issue_text(issues.get(int(number)))
        if text:
            primary_texts[number] = text
    if not primary_texts:
        return {"skip": "no-primary-text"}
    index_path, status = brd.index_for_contest(slug, entry, checkout, out_dir, args.reuse_index_dirs, args.bin)
    if index_path is None:
        return {"skip": status}
    priors = brd.write_corpus(slug, primary_texts, out_dir)
    prepared = out_dir / "prepared" / f"{slug}.json"
    prepared.parent.mkdir(parents=True, exist_ok=True)
    brd.run([str(args.bin), "candidates", "--corpus", str(priors), "--index", str(index_path),
             "--prepare", str(prepared), "--json"], f"{slug}: prepare")

    c = {"dup_attempted": 0, "dup_hit5": 0, "dup_abstained": 0, "dup_text_missing": 0,
         "dup_primary_not_in_corpus": 0, "primary_attempted": 0, "primary_abstained": 0,
         "primary_text_missing": 0, "primary_alarms": 0, "alarms": 0, "true_alarms": 0,
         "alarm_status": None}
    queries = []
    for primary in sorted(duplicates, key=int):
        for dup in sorted(duplicates[primary], key=int):
            if primary not in primary_texts:
                c["dup_primary_not_in_corpus"] += 1
                continue
            text = brd.issue_text(issues.get(int(dup)))
            if text is None:
                c["dup_text_missing"] += 1
                continue
            queries.append(("dup", primary, str(dup), text))
    for primary in primaries:
        text = primary_texts.get(primary)
        if text is None:
            c["primary_text_missing"] += 1
            continue
        queries.append(("primary", None, primary, text))

    qdir = brd.clear_queries(slug, out_dir)
    for role, expected, issue, text in queries:
        qpath = brd.write_query(qdir, f"{role}-{issue}", text)
        proc = brd.run([str(args.bin), "candidates", str(qpath), "--corpus", str(prepared),
                        "--index", str(index_path), "--exclude", issue, "--top-k", str(TOP),
                        "--ranker", str(args.ranker), "--alarm", str(args.alarm), "--json"],
                       f"{slug}: {role} {issue}")
        rep = json.loads(proc.stdout)
        abstained = bool(rep.get("abstention"))
        top_ids = [x["prior_id"] for x in rep.get("candidates", [])][:TOP]
        alarm = rep.get("alarm") or {}
        if c["alarm_status"] is None and alarm:
            c["alarm_status"] = alarm.get("status")
        fires = bool(alarm.get("fires")) and not abstained
        if role == "dup":
            c["dup_attempted"] += 1
            c["dup_abstained"] += abstained
            c["dup_hit5"] += (not abstained) and expected in top_ids
        else:
            c["primary_attempted"] += 1
            c["primary_abstained"] += abstained
            c["primary_alarms"] += fires
        if fires:
            c["alarms"] += 1
            c["true_alarms"] += role == "dup" and alarm.get("top_candidate") == expected
    return c


def cells(c: dict) -> dict:
    attempted = c["dup_attempted"] + c["primary_attempted"]
    return {
        "recall_at_5": cell(c["dup_hit5"], c["dup_attempted"]),
        "far_at_alarm": cell(c["primary_alarms"], c["primary_attempted"]),
        "warning_precision": cell(c["true_alarms"], c["alarms"]),
        "abstention": cell(c["dup_abstained"] + c["primary_abstained"], attempted),
    }


def pool(rows: list[dict]) -> dict:
    keys = ["dup_attempted", "dup_hit5", "dup_abstained", "primary_attempted",
            "primary_abstained", "primary_alarms", "alarms", "true_alarms"]
    tot = {k: sum(r["counts"][k] for r in rows) for k in keys}
    return {"contests": [r["contest"] for r in rows], "counts": tot, "cells": cells(tot)}


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--mode", required=True, choices=["dev", "pairs"])
    ap.add_argument("--contest", action="append", default=[], help="dev mode: a Dup-Bench train slug (repeat)")
    ap.add_argument("--round"); ap.add_argument("--version"); ap.add_argument("--binary-sha256")
    ap.add_argument("--pairs", default=str(HERE / "pairs.json"))
    ap.add_argument("--manifest", default=str(HERE / "manifest.json"))
    ap.add_argument("--labels", default=str(HERE / "labels.json"))
    ap.add_argument("--codebases", default=str(HERE / "codebases.json"))
    ap.add_argument("--labels-dir", default=str(brd.default_labels_dir()))
    ap.add_argument("--pair-labels", help="pairs mode: DIR with labels.json and <slug>/page-*.json for pair contests outside Dup-Bench")
    ap.add_argument("--bin", default=str(ROOT / "target" / "release" / ("priorart.exe" if sys.platform == "win32" else "priorart")))
    ap.add_argument("--ranker", default=str(HERE / "weights" / "v1.json"))
    ap.add_argument("--alarm", default=str(HERE / "weights" / "alarm-v1.json"))
    ap.add_argument("--source-dir", action="append", default=[])
    ap.add_argument("--reuse-index-dir", action="append", default=[])
    ap.add_argument("--work", default=str(ROOT / "target" / "pairs-run"))
    ap.add_argument("--out", required=True)
    ap.add_argument("--check", metavar="FILE", help="recompute and require byte-identical output to FILE")
    args = ap.parse_args(argv)
    args.bin = Path(args.bin); args.ranker = Path(args.ranker); args.alarm = Path(args.alarm)
    args.source_dirs = [Path(p) for p in args.source_dir] or brd.default_source_dirs()
    args.reuse_index_dirs = [Path(p) for p in args.reuse_index_dir] or brd.default_reuse_index_dirs()

    manifest = brd.load_json(Path(args.manifest))
    splits = dupbench_splits(manifest)
    labels_all = brd.load_json(Path(args.labels))["contests"]
    codebases = brd.load_json(Path(args.codebases))
    cb = codebases["codebases"]  # slug -> pin

    plan = []  # (contest, cell_group, entry, labels, labels_dir)
    refused = []
    if args.mode == "dev":
        if not args.contest:
            ap.error("--mode dev needs at least one --contest (a Dup-Bench train slug)")
        for slug in args.contest:
            split = splits.get(slug)
            if split != "train":
                ap.error(f"{slug}: dev mode runs only Dup-Bench train contests (this one is {split or 'not in Dup-Bench'})")
            entry = cb.get(slug)
            if not entry or not entry.get("commit"):
                ap.error(f"{slug}: no codebase pin")
            plan.append((slug, "dev-train", {"repo": entry["repo"], "commit": entry["commit"]},
                         labels_all.get(slug, {}), Path(args.labels_dir)))
    else:
        for need in ("round", "version", "binary_sha256"):
            if not getattr(args, need):
                ap.error(f"--mode pairs needs --{need.replace('_', '-')}")
        actual = sha256_file(args.bin)
        if args.binary_sha256.lower() != actual:
            ap.error(f"--binary-sha256 {args.binary_sha256!r} is not the sha256 of {args.bin.name} ({actual}); placeholders are refused")
        pair_labels = brd.load_json(Path(args.pair_labels) / "labels.json")["contests"] if args.pair_labels else {}
        for pair in brd.load_json(Path(args.pairs))["pairs"]:
            slug = pair["contest"]["slug"]
            split = splits.get(slug)
            if split == "FINAL":
                refused.append({"pair": pair["pair_id"], "contest": slug, "reason": "dup-bench-final-contest: running it would open a FINAL contest"})
                continue
            entry = {"repo": pair["codebase"]["repo"], "commit": pair["codebase"]["commit"]}
            if split in ("train", "validation"):
                plan.append((slug, f"in-sample-dupbench-{split}", entry, labels_all.get(slug, {}), Path(args.labels_dir)))
            else:
                lab = pair_labels.get(slug)
                if lab is None:
                    refused.append({"pair": pair["pair_id"], "contest": slug, "reason": "pair-labels-missing"})
                    continue
                plan.append((slug, "v1-fresh", entry, lab, Path(args.pair_labels)))

    work = Path(args.work)
    rows = []
    for slug, group, entry, labels, ldir in plan:
        issues = brd.load_issue_pages(ldir, slug)
        c = run_contest(slug, entry, labels, issues, args, work)
        row = {"contest": slug, "group": group, "split_dupbench": splits.get(slug)}
        if "skip" in c:
            row["skip"] = c["skip"]
        else:
            row["counts"] = c
            row["cells"] = cells(c)
        rows.append(row)
        print(f"{group} {slug}: {row.get('skip') or row['cells']['recall_at_5']}", file=sys.stderr, flush=True)

    groups = sorted({r["group"] for r in rows if "counts" in r})
    result = {
        "schema": SCHEMA,
        "mode": args.mode,
        "round": args.round, "version": args.version,
        "binary_sha256": sha256_file(args.bin),
        "ranker_sha256": sha256_file(args.ranker),
        "alarm_sha256": sha256_file(args.alarm),
        "prose_anchors": False,
        "top": TOP,
        "pairs_sha256": sha256_file(Path(args.pairs)),
        "refused": refused,
        "contests": rows,
        # Groups are never pooled with each other: fresh V1 pairs, in-sample
        # Dup-Bench overlaps and DEV contests are reported apart.
        "pooled_by_group": {g: pool([r for r in rows if r["group"] == g and "counts" in r]) for g in groups},
    }
    text = json.dumps(result, indent=2, sort_keys=True) + "\n"
    if args.check:
        old = Path(args.check).read_text(encoding="utf-8")
        if old != text:
            print(f"--check: output differs from {args.check}", file=sys.stderr)
            return 1
        print(f"--check: byte-identical to {args.check}", file=sys.stderr)
        return 0
    out = Path(args.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(text, encoding="utf-8", newline="\n")
    print(f"wrote {args.out}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
