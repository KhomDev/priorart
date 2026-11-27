#!/usr/bin/env python3
"""Builds the human duplicate-judging sheets for the V1 pairs (approved rubric: JUDGING_RUBRIC.md).

Item = a query finding and the matcher's top candidate from the same contest. The pool is
(a) every query on which the shipped alarm fired (the 48 firings of round-1-pairs.json) and
(b) 48 controls: duplicate queries that did not abstain and on which the alarm stayed quiet,
drawn by seed. Nothing else is read. The labeller sees two findings and nothing more: no contest,
no issue number, no score, no alarm decision, no judge grouping.

It re-runs the same queries as pairs_run.py (same binary, ranker and alarm, prose anchors off), so
the alarm decisions must reproduce the committed counts (it refuses otherwise).

Outputs (all outside the repository; they hold third-party issue text):
  <out>/items.json        per item: contest, query issue, candidate issue, role, alarm, judge truth (the KEY)
  <out>/batches/<id>/PAIR_SHEET.csv/.md   one sheet per labeller (L-slot id), shuffled per labeller
  <out>/plan.json         batch plan: anchors, batches, labeller slots (no identities)
Usage:
  python -I eval/dupbench/pairs-swap/judging_sheet.py --bin B --pair-labels DIR --result eval/rounds/round-1-pairs.json --out DIR
"""
from __future__ import annotations

import argparse, csv, hashlib, io, json, random, sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
DB = HERE.parent
sys.path.insert(0, str(DB))
import build_ranker_data as brd  # noqa: E402
import pairs_run as pr  # noqa: E402

CONTROLS, SEED_CONTROLS = 48, "p2r-v1-priorart-controls-2026-11-27"
SEED_ANCHORS, SEED_BATCHES = "p2r-v1-anchors-2026-11-27", "p2r-v1-priorart-batches-2026-11-27"
ANCHORS, BATCHES = 3, 4
CAP = 7000


def h(*parts: str) -> str:
    return hashlib.sha256("|".join(parts).encode()).hexdigest()


def collect(slug, entry, labels, issues, args, work) -> list[dict]:
    checkout = brd.find_checkout(slug, args.source_dirs)
    assert checkout is not None and brd.git_head(checkout).lower() == entry["commit"].lower(), slug
    primaries = sorted((str(n) for n in labels["primaries"]), key=int)
    duplicates = labels.get("duplicates", {})
    primary_texts = {n: brd.issue_text(issues.get(int(n))) for n in primaries}
    primary_texts = {n: t for n, t in primary_texts.items() if t}
    index_path, status = brd.index_for_contest(slug, entry, checkout, work, [], args.bin)
    assert index_path is not None, (slug, status)
    priors = brd.write_corpus(slug, primary_texts, work)
    prepared = work / "prepared" / f"{slug}.json"
    prepared.parent.mkdir(parents=True, exist_ok=True)
    brd.run([str(args.bin), "candidates", "--corpus", str(priors), "--index", str(index_path),
             "--prepare", str(prepared), "--json"], f"{slug}: prepare")
    queries = []
    for primary in sorted(duplicates, key=int):
        for dup in sorted(duplicates[primary], key=int):
            if primary in primary_texts and brd.issue_text(issues.get(int(dup))):
                queries.append(("dup", primary, str(dup)))
    for primary in primaries:
        if primary in primary_texts:
            queries.append(("primary", None, primary))
    qdir = brd.clear_queries(slug, work)
    out = []
    for role, expected, issue in queries:
        text = brd.issue_text(issues.get(int(issue)))
        qpath = brd.write_query(qdir, f"{role}-{issue}", text)
        proc = brd.run([str(args.bin), "candidates", str(qpath), "--corpus", str(prepared), "--index", str(index_path),
                        "--exclude", issue, "--top-k", str(pr.TOP), "--ranker", str(args.ranker), "--alarm", str(args.alarm),
                        "--json"], f"{slug}: {role} {issue}")
        rep = json.loads(proc.stdout)
        abstained = bool(rep.get("abstention"))
        cands = [x["prior_id"] for x in rep.get("candidates", [])]
        alarm = rep.get("alarm") or {}
        fires = bool(alarm.get("fires")) and not abstained
        top = alarm.get("top_candidate") if fires else (cands[0] if cands else None)
        out.append({"contest": slug, "role": role, "issue": issue, "expected_primary": expected,
                    "abstained": abstained, "alarm_fires": fires, "top_candidate": top,
                    "top_is_judged_primary": (top == expected) if role == "dup" else False})
    return out


def trim(text: str) -> str:
    return text if len(text) <= CAP else text[:CAP] + "\n[... truncated for the sheet ...]"


def check_alarm_counts(rows: list[dict], want: dict) -> list[dict]:
    """The alarm decisions must reproduce the committed V1 counts; returns the firing rows."""
    alarms = [r for r in rows if r["alarm_fires"]]
    true_alarms = sum(r["top_is_judged_primary"] for r in alarms if r["role"] == "dup")
    if len(alarms) != want["alarms"] or true_alarms != want["true_alarms"]:
        raise ValueError(f"alarm counts {len(alarms)} (true {true_alarms}) do not match the committed run {want}")
    return alarms


def select_controls(rows: list[dict], n: int = CONTROLS, seed: str = SEED_CONTROLS) -> list[dict]:
    """Duplicate queries that did not abstain, had a candidate and left the alarm quiet; first n by seeded hash."""
    pool = [r for r in rows if r["role"] == "dup" and not r["abstained"] and not r["alarm_fires"] and r["top_candidate"]]
    pool.sort(key=lambda r: h(seed, r["contest"], r["issue"]))
    return pool[:n]


def number_items(alarms: list[dict], controls: list[dict]) -> list[dict]:
    """Tag each row with its kind, order by seeded hash and number I001, I002, ... (no ordering by kind)."""
    items = [{**r, "kind": kind} for kind, rows in (("alarm", alarms), ("control", controls)) for r in rows]
    items.sort(key=lambda r: h(SEED_BATCHES, r["contest"], r["issue"]))
    for i, it in enumerate(items):
        it["item"] = f"I{i + 1:03d}"
    return items


def pick_anchors(items: list[dict], n: int = ANCHORS) -> set[str]:
    return {a["item"] for a in sorted(items, key=lambda r: h(SEED_ANCHORS, r["item"]))[:n]}


def split_batches(items: list[dict], anchor_ids: set[str], n_batches: int = BATCHES) -> dict[str, list[str]]:
    """Everything except the anchors, split as evenly as possible; smaller batches first (23,23,23,24 for 93 items)."""
    rest = [it["item"] for it in items if it["item"] not in anchor_ids]
    sizes = sorted(len(rest) // n_batches + (1 if i < len(rest) % n_batches else 0) for i in range(n_batches))
    batches, pos = {}, 0
    for b, n in enumerate(sizes, 1):
        batches[f"P{b}"] = rest[pos:pos + n]
        pos += n
    return batches


def shuffled_rows(lid: str, batch_ids: list[str], anchor_ids: set[str]) -> list[str]:
    """The batch plus every anchor, in an order that depends on the labeller slot only."""
    row_ids = list(batch_ids) + sorted(anchor_ids)
    random.Random(f"{SEED_BATCHES}/{lid}").shuffle(row_ids)
    return row_ids


def render_sheet(lid: str, row_ids: list[str], by_item: dict[str, dict]) -> tuple[str, str, dict[str, str]]:
    """CSV and markdown sheet for one labeller slot, plus the row-to-item key. The sheet carries only the two texts."""
    buf = io.StringIO()
    w = csv.writer(buf, lineterminator="\n")
    w.writerow(["row", "finding_1", "finding_2", "label (yes/no/unclear)", "reason (one line, required for unclear)"])
    md = [f"# Duplicate judging sheet {lid}", "",
          "For each row: do the two findings describe the same underlying vulnerability (same root cause in the same code path), "
          "so that a judge would group them as duplicates? Answer yes, no or unclear. Rubric: JUDGING_RUBRIC.md. Do not search for the contest's "
          "published report; label alone.", ""]
    keymap = {}
    for n, iid in enumerate(row_ids, 1):
        it = by_item[iid]
        rid = f"R{n:02d}"
        w.writerow([rid, it["_q"], it["_c"], "", ""])
        md += [f"## {rid}", "", "### Finding 1", "", it["_q"], "", "### Finding 2", "", it["_c"], "", "Label: yes / no / unclear", ""]
        keymap[rid] = iid
    return buf.getvalue(), "\n".join(md) + "\n", keymap

def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--bin", required=True); ap.add_argument("--pair-labels", required=True)
    ap.add_argument("--result", required=True); ap.add_argument("--out", required=True)
    ap.add_argument("--pairs", default=str(HERE / "pairs_v1_swapped.json"))
    ap.add_argument("--ranker", default=str(DB / "weights" / "v1.json")); ap.add_argument("--alarm", default=str(DB / "weights" / "alarm-v1.json"))
    ap.add_argument("--source-dir", action="append", default=[]); ap.add_argument("--work", required=True)
    args = ap.parse_args()
    args.bin, args.ranker, args.alarm = Path(args.bin), Path(args.ranker), Path(args.alarm)
    args.source_dirs = [Path(p) for p in args.source_dir]
    out, work = Path(args.out), Path(args.work)
    result = json.loads(Path(args.result).read_text(encoding="utf-8"))
    assert pr.sha256_file(args.bin) == result["binary_sha256"], "binary differs from the V1 run"
    pl = Path(args.pair_labels)
    labels = json.loads((pl / "labels.json").read_text(encoding="utf-8"))["contests"]
    allq, texts = [], {}
    for pair in json.loads(Path(args.pairs).read_text(encoding="utf-8"))["pairs"]:
        slug = pair["contest"]["slug"]
        entry = {"repo": pair["codebase"]["repo"], "commit": pair["codebase"]["commit"]}
        issues = brd.load_issue_pages(pl, slug)
        rows = collect(slug, entry, labels[slug], issues, args, work)
        for r in rows:
            r["_q"] = trim(brd.issue_text(issues.get(int(r["issue"]))) or "")
            r["_c"] = trim(brd.issue_text(issues.get(int(r["top_candidate"]))) or "") if r["top_candidate"] else ""
        allq += rows
    alarms = check_alarm_counts(allq, result["pooled_by_group"]["v1-fresh"]["counts"])
    controls = select_controls(allq)
    items = number_items(alarms, controls)
    anchor_ids = pick_anchors(items)
    batches = split_batches(items, anchor_ids)
    by_item = {it["item"]: it for it in items}
    out.mkdir(parents=True, exist_ok=True)
    plan = {"seeds": {"controls": SEED_CONTROLS, "anchors": SEED_ANCHORS, "batches": SEED_BATCHES},
            "anchors": sorted(anchor_ids), "batches": batches, "labellers": {}}
    for b, ids in batches.items():
        for role in ("A", "B"):
            lid = f"{b}-{role}"
            row_ids = shuffled_rows(lid, ids, anchor_ids)
            csv_text, md_text, keymap = render_sheet(lid, row_ids, by_item)
            d = out / "batches" / lid; d.mkdir(parents=True, exist_ok=True)
            (d / "PAIR_SHEET.csv").write_text(csv_text, encoding="utf-8", newline="\n")
            (d / "PAIR_SHEET.md").write_text(md_text, encoding="utf-8", newline="\n")
            (d / "rowkey.json").write_text(json.dumps(keymap, indent=1, sort_keys=True) + "\n", encoding="utf-8", newline="\n")
            plan["labellers"][lid] = {"batch": b, "role": role, "rows": len(row_ids)}
    key = [{k: v for k, v in it.items() if not k.startswith("_")} for it in items]
    (out / "items.json").write_text(json.dumps(key, indent=1, sort_keys=True) + "\n", encoding="utf-8", newline="\n")
    (out / "plan.json").write_text(json.dumps(plan, indent=1, sort_keys=True) + "\n", encoding="utf-8", newline="\n")
    print(f"{len(items)} items ({len(alarms)} alarm firings, {len(controls)} controls), anchors {sorted(anchor_ids)}, batches "
          f"{ {b: len(v) for b, v in batches.items()} }, {len(plan['labellers'])} sheets")
    return 0


if __name__ == "__main__":
    sys.exit(main())