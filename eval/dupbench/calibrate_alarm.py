#!/usr/bin/env python3
"""PA-B5: isotonic calibration of the PA-B4 ranker and the shipped alarm threshold.

Reads the PA-B4 ranker dataset (`build_ranker_data.py` output) and the committed
weights (`weights/v1.json`), scores every **validation** query's candidates with
the committed weights (the same formula as `src/candidates/ranker.rs`), and fits
an isotonic (pool-adjacent-violators) map from the query's **top candidate
score** to the probability that the alarm is a true shared root cause.

Definitions (written into the output file verbatim):

- A *query* is one paid-unique primary (`query_role == "primary"`) or one judged
  duplicate (`query_role == "duplicate"`) that did not abstain. Abstained
  queries produce no candidate and therefore no alarm; they are excluded.
- The *top candidate* is the candidate with the highest ranker score; ties keep
  the default candidate order (the order the dataset records), exactly as the
  Rust stable sort does.
- The *alarm* fires when the calibrated probability of the top score is at or
  above the threshold. It names the top candidate.
- The alarm is a *true shared root cause* iff the query is a judged duplicate
  and the top candidate is its judged primary (pair `label == 1`). An alarm on
  a paid-unique primary is always false: by PA-B4's FAR definition a
  paid-unique finding shares no root cause with any other primary. An alarm on
  a duplicate that names a different primary is false.
- Precision at threshold t = k / n over the validation queries whose calibrated
  top score is >= t, where k counts the true alarms. The Wilson 95% interval
  uses z = 1.959964, as in `train_ranker.py`.
- The shipped threshold is the lowest calibrated value t (one per isotonic
  block) whose precision Wilson lower bound is >= 0.80. If no block qualifies,
  the alarm is off (`threshold: null`).

The validation split is used for calibration only. The train split is never
read for calibration. FINAL is refused: the script stops if any dataset row is
labelled FINAL or names a contest the manifest assigns to FINAL, and it stops
if a contest's dataset split disagrees with the manifest.

Pure standard library, deterministic, offline.

Usage:
  python -I eval/dupbench/calibrate_alarm.py --dataset <ranker-data>/dataset.jsonl
  python -I eval/dupbench/calibrate_alarm.py --dataset ... --check   # verify the committed file
  python -I eval/dupbench/calibrate_alarm.py --self-test             # synthetic tests, no data
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

SCHEMA = "priorart.alarm-calibration/v1"
WEIGHTS_SCHEMA = "priorart.ranker-weights/v1"
Z = 1.959964
PRECISION_FLOOR = 0.80
CALL_GRAPH_MISSING = 3.0

DEFINITION = {
    "query": (
        "one paid-unique primary or one judged duplicate of the Dup-Bench validation "
        "split that did not abstain; abstained queries produce no alarm and are excluded"
    ),
    "top_candidate": (
        "the candidate with the highest PA-B4 ranker score (weights v1); ties keep the "
        "default candidate order"
    ),
    "alarm": "fires when the calibrated probability of the top score is >= the threshold",
    "true_alarm": (
        "the query is a judged duplicate and its top candidate is its judged primary "
        "(pair label 1); any alarm on a paid-unique primary is false (PA-B4 FAR "
        "definition); an alarm naming a different primary is false"
    ),
    "precision": (
        "k / n over validation queries whose calibrated top score is >= threshold; "
        "k = true alarms; Wilson 95% interval with z = 1.959964"
    ),
    "threshold_rule": (
        "the lowest isotonic block value whose precision Wilson lower bound is >= 0.80; "
        "none qualifies -> the alarm is off"
    ),
    "calibration_map": (
        "a step function over the raw top score: a score s maps to the value of the last "
        "block whose score_lo <= s; a score below the first block's score_lo maps to the "
        "first block's value"
    ),
}


class Refused(SystemExit):
    """The data would put a FINAL contest into calibration."""


def wilson(k: int, n: int) -> list[float] | None:
    if n == 0:
        return None
    p = k / n
    denom = 1 + Z * Z / n
    centre = (p + Z * Z / (2 * n)) / denom
    half = Z * math.sqrt(p * (1 - p) / n + Z * Z / (4 * n * n)) / denom
    return [max(0.0, centre - half), min(1.0, centre + half)]


def sigmoid(z: float) -> float:
    # Same clamp and formula as `ranker.rs`.
    z = max(-700.0, min(700.0, z))
    return 1.0 / (1.0 + math.exp(-z))


def raw_value(name: str, missing: float | None, f: dict) -> float:
    if name == "call_graph_distance":
        cg = f["call_graph_distance"]
        if cg is None:
            return CALL_GRAPH_MISSING if missing is None else float(missing)
        return float(cg)
    if name == "call_graph_distance_missing":
        return 1.0 if f["call_graph_distance"] is None else 0.0
    value = f[name]
    if value is None:
        return 0.0 if missing is None else float(missing)
    if isinstance(value, bool):
        return 1.0 if value else 0.0
    return float(value)


def score(weights: dict, features: dict) -> float:
    """The PA-B4 score, term by term in the same order as `Ranker::score`."""
    z = float(weights["intercept"])
    for spec in weights["features"]:
        value = raw_value(spec["name"], spec.get("missing_value"), features)
        if spec["transform"] == "log1p":
            value = math.log1p(value)
        z += spec["coefficient"] * (value - spec["mean"]) / spec["scale"]
    return sigmoid(z)


def final_contests(manifest: dict) -> set[str]:
    return {c["slug"] for c in manifest["contests"] if c.get("split") == "FINAL"}


def guard_rows(rows: list[dict], manifest: dict) -> None:
    """Refuse any FINAL row or any split that disagrees with the manifest."""
    finals = final_contests(manifest)
    splits = {c["slug"]: c.get("split") for c in manifest["contests"]}
    for row in rows:
        contest = row.get("contest")
        if row.get("split") == "FINAL" or contest in finals:
            raise Refused(
                f"refusing: dataset row for contest {contest!r} is FINAL; "
                "FINAL never enters calibration"
            )
        if contest in splits and splits[contest] != row.get("split"):
            raise Refused(
                f"refusing: contest {contest!r} is {row.get('split')!r} in the dataset but "
                f"{splits[contest]!r} in the manifest"
            )


def query_points(rows: list[dict], weights: dict, split: str) -> list[dict]:
    """One point per non-abstained query of `split`: top score and truth."""
    pairs: dict[str, list[dict]] = {}
    queries: list[dict] = []
    for row in rows:
        if row.get("split") != split:
            continue
        if row["kind"] == "query":
            queries.append(row)
        elif row["kind"] == "pair":
            pairs.setdefault(row["query"], []).append(row)
    points = []
    for q in queries:
        if q["abstention"]:
            continue
        cands = pairs.get(q["query"], [])
        if not cands:
            continue
        best = None
        best_score = -math.inf
        for pair in cands:  # recorded order = default order; strict > keeps the first on ties
            s = score(weights, pair["features"])
            if s > best_score:
                best, best_score = pair, s
        truth = 1 if (q["query_role"] == "duplicate" and best["label"] == 1) else 0
        points.append({
            "query": q["query"],
            "contest": q["contest"],
            "role": q["query_role"],
            "top_score": best_score,
            "top_candidate": best["candidate"],
            "true_alarm": truth,
        })
    points.sort(key=lambda p: (p["top_score"], p["query"]))
    return points


def isotonic(points: list[dict]) -> list[dict]:
    """Pool-adjacent-violators on (top_score, true_alarm), equal scores pooled first."""
    blocks: list[dict] = []
    for p in points:
        if blocks and blocks[-1]["score_hi"] == p["top_score"]:
            b = blocks[-1]
            b["n"] += 1
            b["k"] += p["true_alarm"]
        else:
            blocks.append({"score_lo": p["top_score"], "score_hi": p["top_score"],
                           "n": 1, "k": p["true_alarm"]})
        while len(blocks) >= 2 and blocks[-2]["k"] * blocks[-1]["n"] >= blocks[-1]["k"] * blocks[-2]["n"]:
            # The previous block's rate is >= this one's: a violation (or a tie,
            # which is pooled so block values are strictly increasing).
            last = blocks.pop()
            prev = blocks[-1]
            prev["score_hi"] = last["score_hi"]
            prev["n"] += last["n"]
            prev["k"] += last["k"]
    for b in blocks:
        b["value"] = b["k"] / b["n"]
    return blocks


def calibrated(blocks: list[dict], s: float) -> float:
    value = blocks[0]["value"]
    for b in blocks:
        if b["score_lo"] <= s:
            value = b["value"]
        else:
            break
    return value


def threshold_table(blocks: list[dict]) -> list[dict]:
    table = []
    for i, b in enumerate(blocks):
        n = sum(x["n"] for x in blocks[i:])
        k = sum(x["k"] for x in blocks[i:])
        w = wilson(k, n)
        table.append({
            "bin": i + 1,
            "score_lo": b["score_lo"],
            "score_hi": b["score_hi"],
            "bin_n": b["n"],
            "bin_k": b["k"],
            "calibrated": b["value"],
            "tail_n": n,
            "tail_k": k,
            "tail_precision": k / n,
            "tail_wilson95": w,
            "bin_wilson95": wilson(b["k"], b["n"]),
        })
    return table


def choose(table: list[dict]) -> dict | None:
    for row in table:
        if row["tail_wilson95"][0] >= PRECISION_FLOOR:
            return row
    return None


def load_rows(path: Path) -> list[dict]:
    with path.open(encoding="utf-8") as handle:
        return [json.loads(line) for line in handle if line.strip()]


def build(rows: list[dict], weights: dict, manifest: dict, dataset_sha: str,
          weights_sha: str) -> dict:
    if weights.get("schema") != WEIGHTS_SCHEMA:
        raise SystemExit(f"weights schema is {weights.get('schema')!r}; expected {WEIGHTS_SCHEMA!r}")
    guard_rows(rows, manifest)
    points = query_points(rows, weights, "validation")
    if not points:
        raise SystemExit("no validation query points; refusing to calibrate")
    contests = sorted({p["contest"] for p in points})
    finals = final_contests(manifest)
    assert not (set(contests) & finals), "a FINAL contest reached calibration"
    blocks = isotonic(points)
    for a, b in zip(blocks, blocks[1:]):
        assert a["value"] < b["value"] and a["score_hi"] < b["score_lo"], "isotonic map is not monotone"
    table = threshold_table(blocks)
    chosen = choose(table)
    roles = {r: sum(1 for p in points if p["role"] == r) for r in ("primary", "duplicate")}
    return {
        "schema": SCHEMA,
        "version": 1,
        "created_by": "eval/dupbench/calibrate_alarm.py",
        "ranker_weights": "eval/dupbench/weights/v1.json",
        "ranker_weights_sha256": weights_sha,
        "dataset_sha256": dataset_sha,
        "split": "validation",
        "validation_contests": contests,
        "final_contests_refused": len(finals),
        "definition": DEFINITION,
        "precision_floor": PRECISION_FLOOR,
        "queries": {"total": len(points), **roles,
                    "true_alarms": sum(p["true_alarm"] for p in points)},
        "blocks": [
            {"score_lo": b["score_lo"], "score_hi": b["score_hi"], "n": b["n"], "k": b["k"],
             "value": b["value"]}
            for b in blocks
        ],
        "table": table,
        "threshold": None if chosen is None else chosen["calibrated"],
        "threshold_score_lo": None if chosen is None else chosen["score_lo"],
        "threshold_bin": None if chosen is None else chosen["bin"],
        "status": "off" if chosen is None else "on",
    }


def render(doc: dict) -> str:
    return json.dumps(doc, indent=1) + "\n"


def print_table(doc: dict) -> None:
    print(f"validation queries={doc['queries']}")
    print(f"{'bin':>3} {'score_lo':>10} {'score_hi':>10} {'n':>5} {'k':>4} {'calib':>7} "
          f"{'tail_n':>6} {'tail_k':>6} {'prec':>7} {'wilson95':>17}")
    for row in doc["table"]:
        w = row["tail_wilson95"]
        print(f"{row['bin']:>3} {row['score_lo']:>10.6f} {row['score_hi']:>10.6f} "
              f"{row['bin_n']:>5} {row['bin_k']:>4} {row['calibrated']:>7.4f} "
              f"{row['tail_n']:>6} {row['tail_k']:>6} {row['tail_precision']:>7.4f} "
              f"[{w[0]:.4f}, {w[1]:.4f}]")
    print(f"status={doc['status']} threshold={doc['threshold']} "
          f"threshold_score_lo={doc['threshold_score_lo']} bin={doc['threshold_bin']}")


def self_test() -> int:
    """Synthetic checks: monotone map, threshold rule, FINAL refusal. No data read."""
    pts = [{"query": f"q{i}", "contest": "c", "role": "duplicate", "top_score": s,
            "top_candidate": "x", "true_alarm": y}
           for i, (s, y) in enumerate([(0.1, 0), (0.2, 1), (0.3, 0), (0.4, 0), (0.5, 1),
                                        (0.6, 1), (0.6, 0), (0.9, 1)])]
    pts.sort(key=lambda p: (p["top_score"], p["query"]))
    blocks = isotonic(pts)
    values = [b["value"] for b in blocks]
    assert values == sorted(values) and len(set(values)) == len(values), values
    assert sum(b["n"] for b in blocks) == len(pts)
    assert calibrated(blocks, 0.0) == blocks[0]["value"]
    assert calibrated(blocks, 1.0) == blocks[-1]["value"]
    # Threshold rule: 40 true alarms at the top clear the floor; 3 do not.
    strong = [{"query": f"s{i}", "contest": "c", "role": "duplicate", "top_score": 0.99,
               "top_candidate": "x", "true_alarm": 1} for i in range(40)]
    table = threshold_table(isotonic(sorted(pts + strong, key=lambda p: (p["top_score"], p["query"]))))
    chosen = choose(table)
    assert chosen is not None and chosen["tail_wilson95"][0] >= PRECISION_FLOOR
    weak = [{"query": f"w{i}", "contest": "c", "role": "duplicate", "top_score": 0.99,
             "top_candidate": "x", "true_alarm": 1} for i in range(3)]
    assert choose(threshold_table(isotonic(sorted(pts + weak, key=lambda p: (p["top_score"], p["query"]))))) is None
    # FINAL refusal: a FINAL-labelled row, and a row whose contest the manifest
    # assigns to FINAL under another label.
    manifest = {"contests": [{"slug": "v", "split": "validation"}, {"slug": "f", "split": "FINAL"}]}
    for bad in ({"kind": "query", "split": "FINAL", "contest": "v"},
                {"kind": "query", "split": "validation", "contest": "f"},
                {"kind": "query", "split": "train", "contest": "v"}):
        try:
            guard_rows([bad], manifest)
        except Refused:
            continue
        raise AssertionError(f"guard accepted {bad}")
    guard_rows([{"kind": "query", "split": "validation", "contest": "v"}], manifest)
    print("self-test ok")
    return 0


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--dataset", default=str(ROOT / "target" / "ranker-data" / "dataset.jsonl"))
    parser.add_argument("--weights", default=str(HERE / "weights" / "v1.json"))
    parser.add_argument("--manifest", default=str(HERE / "manifest.json"))
    parser.add_argument("--out", default=str(HERE / "weights" / "alarm-v1.json"))
    parser.add_argument("--check", action="store_true",
                        help="recompute and compare with --out byte for byte; write nothing")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args(argv[1:])
    if args.self_test:
        return self_test()
    dataset = Path(args.dataset)
    if not dataset.is_file():
        raise SystemExit(f"dataset not found: {dataset}")
    weights_path = Path(args.weights)
    weights = json.loads(weights_path.read_text(encoding="utf-8"))
    manifest = json.loads(Path(args.manifest).read_text(encoding="utf-8"))
    dataset_sha = hashlib.sha256(dataset.read_bytes()).hexdigest()
    if weights.get("dataset_sha256") and weights["dataset_sha256"] != dataset_sha:
        raise SystemExit(
            f"dataset sha256 {dataset_sha} is not the one the weights were trained on "
            f"({weights['dataset_sha256']}); refusing to calibrate a different dataset"
        )
    weights_sha = hashlib.sha256(weights_path.read_bytes()).hexdigest()
    doc = build(load_rows(dataset), weights, manifest, dataset_sha, weights_sha)
    text = render(doc)
    print_table(doc)
    out = Path(args.out)
    if args.check:
        current = out.read_text(encoding="utf-8") if out.is_file() else ""
        if current != text:
            print(f"CHECK FAILED: {out} differs from the recomputed calibration")
            return 1
        print(f"check ok: {out}")
        return 0
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(text, encoding="utf-8", newline="\n")
    print(f"calibration: {out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
