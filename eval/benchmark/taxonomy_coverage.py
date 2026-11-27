#!/usr/bin/env python3
"""PA-B2 taxonomy cue coverage over the six DEV corpora.

Walks every `.md` finding in each corpus's judging cache, classifies it with
`priorart taxonomy classify --json`, and counts:

- findings: the number of files classified;
- with a root cause: at least one rule in `mechanism.root_causes`;
- abstained: `mechanism.abstain_reason` is set;
- per root-cause class counts.

This is coverage, not accuracy. A class match is a cue hit, not a judged root
cause; no labels exist for this corpus at class level and none are implied.

Deterministic except for filesystem paths. No network.

Usage:
  eval/benchmark/taxonomy_coverage.py [--bin PRIORART] [--cache DIR] [--out FILE] [--batch N]
"""
from __future__ import annotations

import argparse
import json
import subprocess
import sys
from collections import Counter
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent

CORPORA = ["tapioca", "pooltogether", "maia-c4", "panoptic-c4", "party-c4", "wildcat-c4"]


def judging_dirs(cache: Path) -> list[tuple[str, Path]]:
    """Resolve each corpus's judging cache exactly as the PA-B3 driver does."""
    out: list[tuple[str, Path]] = []
    for name in CORPORA:
        corpus_dir = HERE if name == "tapioca" else HERE / name
        lock_path = corpus_dir / "cache.lock.json"
        if not lock_path.is_file():
            lock_path = HERE / "cache.lock.json"
        lock = json.loads(lock_path.read_text(encoding="utf-8"))
        judging = [k for k, v in lock["sources"].items() if v.get("role") == "judging"]
        if len(judging) != 1:
            raise SystemExit(f"{name}: expected one judging source, found {judging}")
        corpus_cache = cache if name == "tapioca" else cache / name
        path = corpus_cache / judging[0]
        if not path.is_dir():
            raise SystemExit(f"{name}: judging cache missing: {path}")
        out.append((name, path))
    return out


def classify(binary: Path, files: list[str], batch: int) -> list[dict]:
    results: list[dict] = []
    for start in range(0, len(files), batch):
        chunk = files[start:start + batch]
        proc = subprocess.run(
            [str(binary), "taxonomy", "classify", *chunk, "--json"],
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
        )
        if proc.returncode != 0:
            raise SystemExit(
                f"taxonomy classify failed ({proc.returncode}) on batch {start}: "
                f"{proc.stderr[:2000]}"
            )
        payload = json.loads(proc.stdout)
        results.extend(payload["results"])
    return results


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--bin", default=str(ROOT / "target" / "release" / "priorart.exe"))
    parser.add_argument("--cache", default=str(ROOT / "eval" / "cache" / "benchmark"))
    parser.add_argument("--out", default=str(ROOT / "target" / "taxonomy-coverage.json"))
    parser.add_argument("--batch", type=int, default=200)
    args = parser.parse_args(argv[1:])

    binary = Path(args.bin).resolve()
    if not binary.is_file():
        raise SystemExit(f"binary not found: {binary}")
    cache = Path(args.cache).resolve()

    per_corpus: dict[str, dict] = {}
    totals: dict[str, Counter] = {"classes": Counter()}
    files_total = with_root = abstained = 0
    batches: list[tuple[str, list[str]]] = []
    for name, directory in judging_dirs(cache):
        files = sorted(str(p) for p in directory.rglob("*.md"))
        batches.append((name, files))
        files_total += len(files)

    for name, files in batches:
        results = classify(binary, files, args.batch)
        if len(results) != len(files):
            raise SystemExit(f"{name}: {len(results)} results for {len(files)} files")
        corpus_root = with_root_c = abstained_c = 0
        classes: Counter = Counter()
        for row in results:
            mechanism = row.get("mechanism") or {}
            roots = mechanism.get("root_causes") or []
            corpus_root += 1
            if roots:
                with_root_c += 1
                for root in roots:
                    classes[root["class"]] += 1
            if mechanism.get("abstain_reason"):
                abstained_c += 1
        per_corpus[name] = {
            "files": corpus_root,
            "with_root_cause": with_root_c,
            "with_root_cause_rate": round(with_root_c / corpus_root, 4) if corpus_root else None,
            "abstained": abstained_c,
            "abstain_rate": round(abstained_c / corpus_root, 4) if corpus_root else None,
            "root_cause_classes": dict(sorted(classes.items())),
        }
        with_root += with_root_c
        abstained += abstained_c
        totals["classes"].update(classes)
        print(
            f"{name}: files={corpus_root} with_root_cause={with_root_c} "
            f"({per_corpus[name]['with_root_cause_rate']}) abstained={abstained_c} "
            f"({per_corpus[name]['abstain_rate']})",
            file=sys.stderr,
            flush=True,
        )

    summary = {
        "schema": "priorart.taxonomy-coverage/v1",
        "binary": str(binary),
        "cache": str(cache),
        "findings": files_total,
        "with_root_cause": with_root,
        "with_root_cause_rate": round(with_root / files_total, 4) if files_total else None,
        "abstained": abstained,
        "abstain_rate": round(abstained / files_total, 4) if files_total else None,
        "root_cause_class_counts": dict(sorted(totals["classes"].items())),
        "per_corpus": per_corpus,
        "note": "Coverage, not accuracy: a class match is a cue hit, not a judged root cause.",
    }
    out = Path(args.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(summary, indent=1, sort_keys=True) + "\n", encoding="utf-8", newline="\n")
    print(
        f"findings={files_total} with_root_cause={with_root} "
        f"({summary['with_root_cause_rate']}) abstained={abstained} "
        f"({summary['abstain_rate']}) classes_matched={len(summary['root_cause_class_counts'])}"
    )
    print(f"OUT {out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
