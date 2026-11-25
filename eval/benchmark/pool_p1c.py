#!/usr/bin/env python3
"""Pool the P1-C priorart measurements across corpora and add a
source-clustered bootstrap interval.

A corpus is a source cluster: flags and findings within one protocol are not
independent, so the pooled Wilson interval is too narrow. Resample the corpora
with replacement, 10,000 draws, fixed seed.

Usage:
  python eval/benchmark/pool_p1c.py [--json]
"""

from __future__ import annotations

import argparse
import json
import math
import random
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
HERE = ROOT / "eval" / "benchmark"
Z = 1.959964
DRAWS = 10_000
SEED = "priorart-bootstrap-p1c-2026-11-25"

CORPORA = [
    ("tapioca", "sherlock", "Tapioca"),
    ("pooltogether", "sherlock", "PoolTogether"),
    ("maia-c4", "code4rena", "Maia"),
    ("panoptic-c4", "code4rena", "Panoptic"),
    ("party-c4", "code4rena", "Party"),
    ("wildcat-c4", "code4rena", "Wildcat"),
]


def wilson(k: int, n: int) -> list[float] | None:
    if n == 0:
        return None
    p = k / n
    denom = 1 + Z * Z / n
    centre = (p + Z * Z / (2 * n)) / denom
    half = Z * math.sqrt(p * (1 - p) / n + Z * Z / (4 * n * n)) / denom
    return [round(max(0.0, centre - half), 4), round(min(1.0, centre + half), 4)]


def load(name: str) -> dict | None:
    path = HERE / name / "results.json"
    if not path.is_file():
        return None
    doc = json.loads(path.read_text(encoding="utf-8"))
    m = doc.get("measurements", {})
    far = m.get("false_alarm_rate", {})
    dup = m.get("duplicate_recall", {})
    kp = m.get("known_prior_recall", {})
    return {"far": (far.get("alarms"), far.get("n")),
            "dup": (dup.get("hits"), dup.get("n")),
            "kp": (kp.get("hits"), kp.get("n"))}


def pooled(values: list[tuple[str, int | None, int | None]]) -> dict:
    have = [(k, n) for _, k, n in values if n is not None]
    k = sum(x for x, _ in have)
    n = sum(y for _, y in have)
    rng = random.Random(SEED)
    rates = []
    for _ in range(DRAWS):
        if not have:
            break
        bk = bn = 0
        for _ in have:
            j = rng.randrange(len(have))
            bk += have[j][0]
            bn += have[j][1]
        if bn:
            rates.append(bk / bn)
    rates.sort()
    boot = ([round(rates[int(0.025 * (len(rates) - 1))], 4),
             round(rates[int(0.975 * (len(rates) - 1))], 4)] if rates else None)
    return {"k": k, "n": n, "rate": round(k / n, 4) if n else None,
            "wilson95": wilson(k, n), "bootstrap95": boot}


def main(argv: list[str]) -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--json", action="store_true")
    args = p.parse_args(argv[1:])
    rows = []
    for name, platform, label in CORPORA:
        m = load(name)
        if m is None:
            continue
        rows.append({"corpus": label, "platform": platform,
                     "far": pooled([(label, *m["far"])]),
                     "duplicate_recall": pooled([(label, *m["dup"])]),
                     "known_prior": pooled([(label, *m["kp"])])})
    pooled_all = {
        "far": pooled([(r["corpus"], r["far"]["k"], r["far"]["n"]) for r in rows]),
        "duplicate_recall": pooled([(r["corpus"], r["duplicate_recall"]["k"],
                                     r["duplicate_recall"]["n"]) for r in rows]),
        "known_prior": pooled([(r["corpus"], r["known_prior"]["k"],
                                r["known_prior"]["n"]) for r in rows]),
    }
    c4 = [r for r in rows if r["platform"] == "code4rena"]
    sherlock = [r for r in rows if r["platform"] == "sherlock"]
    by_platform = {
        "code4rena": {"far": pooled([(r["corpus"], r["far"]["k"], r["far"]["n"]) for r in c4]),
                       "duplicate_recall": pooled([(r["corpus"], r["duplicate_recall"]["k"],
                                                    r["duplicate_recall"]["n"]) for r in c4])},
        "sherlock": {"far": pooled([(r["corpus"], r["far"]["k"], r["far"]["n"]) for r in sherlock]),
                      "duplicate_recall": pooled([(r["corpus"], r["duplicate_recall"]["k"],
                                                   r["duplicate_recall"]["n"]) for r in sherlock])},
    }
    doc = {"schema": "priorart.pool-p1c/v1", "seed": SEED, "draws": DRAWS,
           "corpora": rows, "pooled": pooled_all, "by_platform": by_platform}
    if args.json:
        print(json.dumps(doc, indent=2))
    else:
        print("| Corpus | Platform | FAR | Duplicate recall@5 |")
        print("|---|---|---|---|")
        for r in rows:
            far, dup = r["far"], r["duplicate_recall"]
            print(f"| {r['corpus']} | {r['platform']} | "
                  f"{far['k']}/{far['n']} = {(far['rate'] or 0) * 100:.1f}% "
                  f"[{(far['wilson95'] or [0, 0])[0] * 100:.1f}-{(far['wilson95'] or [0, 0])[1] * 100:.1f}], "
                  f"boot [{(far['bootstrap95'] or [0, 0])[0] * 100:.1f}-{(far['bootstrap95'] or [0, 0])[1] * 100:.1f}] | "
                  f"{dup['k']}/{dup['n']} = {(dup['rate'] or 0) * 100:.1f}% "
                  f"[{(dup['wilson95'] or [0, 0])[0] * 100:.1f}-{(dup['wilson95'] or [0, 0])[1] * 100:.1f}] |")
        for name, block in [("pooled", pooled_all), *by_platform.items()]:
            far, dup = block["far"], block["duplicate_recall"]
            print(f"| {name} | | {far['k']}/{far['n']} = {(far['rate'] or 0) * 100:.1f}% "
                  f"[{(far['wilson95'] or [0, 0])[0] * 100:.1f}-{(far['wilson95'] or [0, 0])[1] * 100:.1f}], "
                  f"boot [{(far['bootstrap95'] or [0, 0])[0] * 100:.1f}-{(far['bootstrap95'] or [0, 0])[1] * 100:.1f}] | "
                  f"{dup['k']}/{dup['n']} = {(dup['rate'] or 0) * 100:.1f}% |")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(__import__("sys").argv))
