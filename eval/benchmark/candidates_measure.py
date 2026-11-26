#!/usr/bin/env python3
"""PA-B3 candidate-recall measurement (DEV corpora + Dup-Bench train only).

Reads the six committed DEV corpora and the Dup-Bench train split, runs the
`priorart candidates` command against them, and writes per-corpus JSON plus a
summary table. It never opens validation or FINAL contests.

Dup-Bench mode: the corpus is pointer-only, so structural anchors resolve only
when the contest's audited codebase is checked out at its pin. Point
`--dupbench-sources` at a directory produced by
`eval/dupbench/clone_codebases.py`; each contest with a checkout at its pinned
commit gets a `priorart anchors index` and the query runs against it. A
contest without a public repository, or with a checkout at the wrong commit,
abstains and is recorded as such — it is never guessed.

Deterministic except for filesystem paths. No network.

Usage:
  eval/benchmark/candidates_measure.py --mode dev [--corpus wildcat-c4]
  eval/benchmark/candidates_measure.py --mode dupbench --dupbench-sources DIR
  eval/benchmark/candidates_measure.py --mode all
"""
from __future__ import annotations

import argparse
import json
import math
import subprocess
import sys
from collections import Counter
from pathlib import Path

import yaml

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
BIN = ROOT / "target" / "release" / "priorart.exe"
BENCH = HERE
CACHE = ROOT / "eval" / "cache" / "benchmark"
SOURCES = ROOT.parent / "worktrees" / "pa-b1" / "target" / "anchors-sources"
DUPBENCH_LABELS = ROOT.parent / "worktrees" / "research-pa" / ".corpus" / "dupbench" / "labels"
DUPBENCH = ROOT / "eval" / "dupbench"
DUPBENCH_SOURCES: Path | None = None
OUT = ROOT / "target" / "candidates-measure"

Z = 1.959964

CORPORA = [
    ("tapioca", "sherlock-audit-2024-02-tapioca"),
    ("pooltogether", "sherlock-audit-2024-05-pooltogether"),
    ("maia-c4", "code-423n4-2023-09-maia"),
    ("panoptic-c4", "code-423n4-2024-04-panoptic"),
    ("party-c4", "code-423n4-2023-10-party"),
    ("wildcat-c4", "code-423n4-2024-08-wildcat"),
]


def wilson(k: int, n: int) -> list[float] | None:
    if n == 0:
        return None
    p = k / n
    denom = 1 + Z * Z / n
    centre = (p + Z * Z / (2 * n)) / denom
    half = Z * math.sqrt(p * (1 - p) / n + Z * Z / (4 * n * n)) / denom
    return [round(max(0.0, centre - half), 4), round(min(1.0, centre + half), 4)]


def quantiles(values: list[float]) -> dict:
    if not values:
        return {"n": 0, "min": None, "p25": None, "median": None, "p75": None, "p90": None, "max": None, "mean": None}
    values = sorted(values)

    def q(p: float) -> float:
        idx = min(len(values) - 1, max(0, int(round((len(values) - 1) * p))))
        return values[idx]

    return {
        "n": len(values),
        "min": values[0],
        "p25": q(0.25),
        "median": q(0.5),
        "p75": q(0.75),
        "p90": q(0.9),
        "max": values[-1],
        "mean": round(sum(values) / len(values), 4),
    }


def sid(finding_id: str) -> str:
    """One deterministic file stem for a finding id."""
    stem = finding_id.replace(":", "-").replace("/", "-")
    return stem[:-3] if stem.lower().endswith(".md") else stem


def run(cmd: list[str]) -> subprocess.CompletedProcess:
    proc = subprocess.run(cmd, capture_output=True, text=True, encoding="utf-8", errors="replace")
    if proc.returncode != 0:
        raise SystemExit(
            f"command failed ({proc.returncode}): {' '.join(cmd)}\nstderr: {proc.stderr[:2000]}"
        )
    return proc


def load_manifest(corpus_dir: Path) -> dict:
    return yaml.safe_load((corpus_dir / "manifest.yaml").read_text(encoding="utf-8"))


def lock_roles(corpus_dir: Path) -> dict:
    lock_path = corpus_dir / "cache.lock.json"
    if not lock_path.is_file():
        lock_path = BENCH / "cache.lock.json"
    lock = json.loads(lock_path.read_text(encoding="utf-8"))
    return {v.get("role"): (k, v) for k, v in lock["sources"].items()}


def cache_dir_for(name: str) -> Path:
    return CACHE if name == "tapioca" else CACHE / name


def corpus_dir_for(name: str) -> Path:
    return BENCH if name == "tapioca" else BENCH / name


def build_index(corpus: str, snapshot_rel: str, roles: dict) -> Path:
    known_id, known = roles["known-issues"]
    snapshot = SOURCES / corpus / snapshot_rel
    head = run(["git", "-C", str(snapshot), "rev-parse", "HEAD"]).stdout.strip()
    if head.lower() != known["commit"].lower():
        raise SystemExit(
            f"{corpus}: source HEAD {head} is not the pinned commit {known['commit']}"
        )
    out = OUT / "index" / f"{corpus}.json"
    out.parent.mkdir(parents=True, exist_ok=True)
    with out.open("w", encoding="utf-8", newline="\n") as handle:
        proc = subprocess.run(
            [
                str(BIN),
                "anchors",
                "index",
                str(snapshot),
                "--repo-id",
                known["repo"],
                "--commit",
                known["commit"],
                "--json",
            ],
            stdout=handle,
            stderr=subprocess.PIPE,
            text=True,
            encoding="utf-8",
            errors="replace",
        )
    if proc.returncode != 0:
        raise SystemExit(f"{corpus}: index failed: {proc.stderr[:2000]}")
    return out


def write_prior_corpus(name: str, findings: list[dict], cache: Path, judging_id: str) -> tuple[Path, dict]:
    priors = OUT / "priors" / name
    priors.mkdir(parents=True, exist_ok=True)
    by_stem: dict[str, str] = {}
    stems: dict[str, str] = {}
    for finding in findings:
        stem = sid(finding["id"])
        if stem in stems:
            raise SystemExit(f"{name}: id collision {finding['id']!r} and {stems[stem]!r} -> {stem}")
        stems[stem] = finding["id"]
        by_stem[finding["id"]] = stem
        text = (cache / judging_id / finding["pointer"]["path"]).read_text(
            encoding="utf-8", errors="replace"
        )
        (priors / f"{stem}.md").write_text(text, encoding="utf-8", newline="\n")
    return priors, by_stem


def measure_corpus(name: str, snapshot_rel: str) -> dict:
    corpus_dir = corpus_dir_for(name)
    manifest = load_manifest(corpus_dir)
    roles = lock_roles(corpus_dir)
    judging_id, _ = roles["judging"]
    cache = cache_dir_for(name)
    index_path = build_index(name, snapshot_rel, roles)
    priors, by_stem = write_prior_corpus(name, manifest["findings"], cache, judging_id)
    prepared = OUT / "prepared" / f"{name}.json"
    prepared.parent.mkdir(parents=True, exist_ok=True)
    run(
        [
            str(BIN),
            "candidates",
            "--corpus",
            str(priors),
            "--index",
            str(index_path),
            "--prepare",
            str(prepared),
            "--json",
        ]
    )

    duplicate_rows = [d for d in manifest.get("duplicates", []) if d.get("expected")]
    rows: list[dict] = []
    for i, duplicate in enumerate(duplicate_rows):
        if i % 20 == 0:
            print(f"{name}: {i}/{len(duplicate_rows)}", file=sys.stderr, flush=True)
        pointer = cache / judging_id / duplicate["pointer"]["path"]
        text = pointer.read_text(encoding="utf-8", errors="replace")
        query = OUT / "queries" / name / f"{sid(duplicate['id'])}.md"
        query.parent.mkdir(parents=True, exist_ok=True)
        query.write_text(text, encoding="utf-8", newline="\n")
        expected = sid(duplicate["expected"])
        proc = run(
            [
                str(BIN),
                "candidates",
                str(query),
                "--corpus",
                str(prepared),
                "--index",
                str(index_path),
                "--exclude",
                sid(duplicate["id"]),
                "--top-k",
                "1000",
                "--json",
            ]
        )
        report = json.loads(proc.stdout)
        candidate_ids = [c["prior_id"] for c in report["candidates"]]
        rows.append(
            {
                "id": duplicate["id"],
                "expected": expected,
                "hit": expected in candidate_ids,
                "candidates": candidate_ids,
                "abstention": (report.get("abstention") or {}).get("reason"),
                "finding_anchors": report["anchors"]["resolved"],
                "finding_mechanism": report["mechanism"]["root_causes"],
                "features": [
                    {
                        "function_shared": len(c["features"]["anchor_function_overlap"]["shared"]),
                        "state_shared": len(c["features"]["anchor_state_overlap"]["shared"]),
                        "distance": c["features"]["call_graph_distance"],
                        "root_causes": len(c["features"]["mechanism_match"]["shared_root_causes"]),
                        "impacts": len(c["features"]["impact_match"]["shared_impacts"]),
                        "identifier_specific": c["features"]["identifier_score"]["specific_symbols"],
                        "rules": [r["rule"] for r in c["reasons"]],
                    }
                    for c in report["candidates"]
                ],
            }
        )

    n = len(rows)
    hits = sum(1 for r in rows if r["hit"])
    abstained = sum(1 for r in rows if r["abstention"])
    reasons = Counter(r["abstention"] for r in rows if r["abstention"])
    sizes = [len(r["candidates"]) for r in rows]
    feature_rows = [f for r in rows for f in r["features"]]
    rule_counts: Counter = Counter()
    for f in feature_rows:
        for rule in f["rules"]:
            rule_counts[rule] += 1
    result = {
        "corpus": name,
        "source_repo": roles["known-issues"][1]["repo"],
        "source_commit": roles["known-issues"][1]["commit"],
        "findings_total": len(manifest["findings"]),
        "duplicates_total": len(duplicate_rows),
        "representatives_in_corpus": n,
        "n": n,
        "hits": hits,
        "recall": round(hits / n, 4) if n else None,
        "wilson95": wilson(hits, n),
        "abstained": abstained,
        "abstain_rate": round(abstained / n, 4) if n else None,
        "abstain_reasons": dict(sorted(reasons.items())),
        "candidate_set_sizes": quantiles([float(s) for s in sizes]),
        "candidate_pairs": len(feature_rows),
        "rule_counts": dict(sorted(rule_counts.items())),
        "features": {
            "function_shared": quantiles([float(f["function_shared"]) for f in feature_rows]),
            "state_shared": quantiles([float(f["state_shared"]) for f in feature_rows]),
            "distance_non_null": sum(1 for f in feature_rows if f["distance"] is not None),
            "distance": quantiles([float(f["distance"]) for f in feature_rows if f["distance"] is not None]),
            "root_causes": quantiles([float(f["root_causes"]) for f in feature_rows]),
            "impacts": quantiles([float(f["impacts"]) for f in feature_rows]),
            "identifier_specific": quantiles([float(f["identifier_specific"]) for f in feature_rows]),
        },
        "rows": rows,
    }
    out = OUT / "results" / f"dev-{name}.json"
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(result, indent=1) + "\n", encoding="utf-8", newline="\n")
    print(
        f"{name}: n={n} hits={hits} recall={result['recall']} "
        f"abstain={abstained} median_candidates={result['candidate_set_sizes']['median']}",
        file=sys.stderr,
        flush=True,
    )
    return result


def load_issue_pages(contest: str) -> dict[int, dict]:
    issues: dict[int, dict] = {}
    folder = DUPBENCH_LABELS / contest
    for page in sorted(folder.glob("page-*.json")):
        payload = json.loads(page.read_text(encoding="utf-8"))
        for item in payload:
            number = item.get("number")
            if isinstance(number, int):
                issues[number] = item
    return issues


def issue_text(issue: dict | None) -> str | None:
    if not issue:
        return None
    body = issue.get("body") or ""
    title = issue.get("title") or ""
    if not body.strip() and not title.strip():
        return None
    return f"# {title}\n\n{body}\n"


def dupbench_index(slug: str, entry: dict | None) -> tuple[Path | None, str]:
    """Index a contest's audited codebase, or return a refusal reason.

    The only acceptable states are: no public repository pin (abstain),
    checkout absent (abstain), checkout at the wrong commit (abstain), or a
    checkout at the pinned commit (index).
    """
    if not entry or entry.get("status") != "repo+commit" or not entry.get("commit"):
        return None, "no-public-repo"
    if DUPBENCH_SOURCES is None:
        return None, "no-source-dir"
    checkout = DUPBENCH_SOURCES / slug
    if not checkout.is_dir():
        return None, "checkout-missing"
    head = subprocess.run(
        ["git", "-C", str(checkout), "rev-parse", "HEAD"],
        capture_output=True, text=True, encoding="utf-8", errors="replace",
    ).stdout.strip()
    if head.lower() != entry["commit"].lower():
        return None, "head-mismatch"
    repo_id = entry["repo"].rstrip("/").split("github.com/")[-1]
    out = OUT / "index" / f"dupbench-{slug}.json"
    out.parent.mkdir(parents=True, exist_ok=True)
    with out.open("w", encoding="utf-8", newline="\n") as handle:
        proc = subprocess.run(
            [
                str(BIN), "anchors", "index", str(checkout),
                "--repo-id", repo_id,
                "--commit", entry["commit"],
                "--json",
            ],
            stdout=handle,
            stderr=subprocess.PIPE,
            text=True,
            encoding="utf-8",
            errors="replace",
        )
    if proc.returncode != 0:
        return None, f"index-failed:{proc.stderr.strip().splitlines()[-1][:120]}"
    return out, "indexed"


def measure_dupbench_train() -> dict:
    manifest = json.loads((DUPBENCH / "manifest.json").read_text(encoding="utf-8"))
    labels = json.loads((DUPBENCH / "labels.json").read_text(encoding="utf-8"))
    codebases = json.loads((DUPBENCH / "codebases.json").read_text(encoding="utf-8"))["codebases"]
    train = [c for c in manifest["contests"] if c.get("split") == "train"]
    contests_out = []
    total_n = total_hits = total_abstained = 0
    total_reasons: Counter = Counter()
    all_sizes: list[float] = []
    indexed_contests = 0
    for contest in sorted(train, key=lambda c: c["slug"]):
        slug = contest["slug"]
        if slug not in labels["contests"]:
            contests_out.append({"contest": slug, "status": "labels-missing"})
            continue
        issues = load_issue_pages(slug)
        label = labels["contests"][slug]
        primaries = [str(n) for n in label.get("primaries", [])]
        duplicates = label.get("duplicates", {})
        primary_texts: dict[str, str] = {}
        for primary in primaries:
            text = issue_text(issues.get(int(primary)))
            if text:
                primary_texts[primary] = text
        priors = OUT / "dupbench-priors" / slug
        priors.mkdir(parents=True, exist_ok=True)
        for number, text in primary_texts.items():
            (priors / f"{number}.md").write_text(text, encoding="utf-8", newline="\n")
        if not primary_texts:
            contests_out.append({"contest": slug, "status": "no-primary-text"})
            continue
        index_path, source_status = dupbench_index(slug, codebases.get(slug))
        if index_path is not None:
            indexed_contests += 1
        index_args = ["--index", str(index_path)] if index_path else []
        # Prepare once. With an index the prior items carry anchors; without
        # one the mechanism and identifier profiles still exercise the path,
        # and the per-query result abstains with a reason.
        prepared = OUT / "dupbench-prepared" / f"{slug}.json"
        prepared.parent.mkdir(parents=True, exist_ok=True)
        run(
            [
                str(BIN),
                "candidates",
                "--corpus",
                str(priors),
                *index_args,
                "--prepare",
                str(prepared),
                "--json",
            ]
        )
        rows = []
        for primary, dup_numbers in duplicates.items():
            if primary not in primary_texts:
                continue
            for dup in dup_numbers:
                text = issue_text(issues.get(int(dup)))
                if not text:
                    continue
                query = OUT / "dupbench-queries" / slug / f"{dup}.md"
                query.parent.mkdir(parents=True, exist_ok=True)
                query.write_text(text, encoding="utf-8", newline="\n")
                proc = run(
                    [
                        str(BIN),
                        "candidates",
                        str(query),
                        "--corpus",
                        str(prepared),
                        *index_args,
                        "--exclude",
                        dup,
                        "--top-k",
                        "1000",
                        "--json",
                    ]
                )
                report = json.loads(proc.stdout)
                candidate_ids = [c["prior_id"] for c in report["candidates"]]
                rows.append(
                    {
                        "duplicate": dup,
                        "expected": primary,
                        "hit": primary in candidate_ids,
                        "candidates": candidate_ids,
                        "abstention": (report.get("abstention") or {}).get("reason"),
                    }
                )
        n = len(rows)
        hits = sum(1 for r in rows if r["hit"])
        abstained = sum(1 for r in rows if r["abstention"])
        reasons = Counter(r["abstention"] for r in rows if r["abstention"])
        total_n += n
        total_hits += hits
        total_abstained += abstained
        total_reasons.update(reasons)
        all_sizes.extend(len(r["candidates"]) for r in rows)
        contests_out.append(
            {
                "contest": slug,
                "status": "measured" if n else "no-duplicates-with-text",
                "source_status": source_status,
                "indexed": index_path is not None,
                "primaries": len(primaries),
                "primary_texts": len(primary_texts),
                "duplicates_total": sum(len(v) for v in duplicates.values()),
                "n": n,
                "hits": hits,
                "recall": round(hits / n, 4) if n else None,
                "wilson95": wilson(hits, n),
                "abstained": abstained,
                "abstain_reasons": dict(sorted(reasons.items())),
                "rows": rows,
            }
        )
        print(f"dupbench {slug}: n={n} hits={hits} source={source_status}", file=sys.stderr, flush=True)
    result = {
        "split": "train",
        "contests_total": len(train),
        "contests_indexed": indexed_contests,
        "contests": contests_out,
        "n": total_n,
        "hits": total_hits,
        "recall": round(total_hits / total_n, 4) if total_n else None,
        "wilson95": wilson(total_hits, total_n),
        "abstained": total_abstained,
        "abstain_rate": round(total_abstained / total_n, 4) if total_n else None,
        "abstain_reasons": dict(sorted(total_reasons.items())),
        "candidate_set_sizes": quantiles(all_sizes),
        "source_code_checkouts": indexed_contests > 0,
        "dupbench_sources": str(DUPBENCH_SOURCES) if DUPBENCH_SOURCES else None,
        "note": (
            "Train split only. Contests with a public repository pin and a checkout at that "
            "commit are indexed; every other contest abstains and is recorded with its reason."
        ),
    }
    out = OUT / "results" / "dupbench-train.json"
    out.write_text(json.dumps(result, indent=1) + "\n", encoding="utf-8", newline="\n")
    return result


def main() -> int:
    global BIN, CACHE, SOURCES, DUPBENCH_LABELS, DUPBENCH_SOURCES, OUT
    parser = argparse.ArgumentParser()
    parser.add_argument("--mode", choices=["dev", "dupbench", "all"], default="all")
    parser.add_argument("--corpus", action="append", default=[])
    parser.add_argument("--bin", default=str(BIN))
    parser.add_argument("--cache", default=str(CACHE))
    parser.add_argument("--sources", default=str(SOURCES))
    parser.add_argument("--dupbench-labels", default=str(DUPBENCH_LABELS))
    parser.add_argument("--dupbench-sources", default=None)
    parser.add_argument("--out", default=str(OUT))
    args = parser.parse_args()

    BIN = Path(args.bin).resolve()
    CACHE = Path(args.cache).resolve()
    SOURCES = Path(args.sources).resolve()
    DUPBENCH_LABELS = Path(args.dupbench_labels).resolve()
    DUPBENCH_SOURCES = Path(args.dupbench_sources).resolve() if args.dupbench_sources else None
    OUT = Path(args.out).resolve()

    if not BIN.is_file():
        raise SystemExit(f"binary not found: {BIN}")
    OUT.mkdir(parents=True, exist_ok=True)
    results = []
    if args.mode in ("dev", "all"):
        for name, snapshot in CORPORA:
            if args.corpus and name not in args.corpus:
                continue
            results.append(measure_corpus(name, snapshot))
    if args.mode in ("dupbench", "all"):
        results.append(measure_dupbench_train())
    summary = OUT / "results" / "summary.json"
    summary.parent.mkdir(parents=True, exist_ok=True)
    existing = []
    if summary.is_file():
        loaded = json.loads(summary.read_text(encoding="utf-8"))
        existing = loaded if isinstance(loaded, list) else list(loaded.values())
    merged = {r.get("corpus") or "dupbench-train": r for r in existing} if existing else {}
    for r in results:
        merged[r.get("corpus") or "dupbench-train"] = r
    summary.write_text(json.dumps(merged, indent=1) + "\n", encoding="utf-8", newline="\n")
    print(json.dumps(merged, indent=1))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
