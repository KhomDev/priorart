#!/usr/bin/env python3
"""Train the PA-B4 ranker and write the versioned weights table.

Pure Python standard library: L2-regularised logistic regression fitted with
Newton-Raphson / IRLS over a fixed design matrix, so the result is
deterministic (no random draws; the seed is recorded for provenance). Fit on
the Dup-Bench **train** split only. The regularisation strength and the count
transform are chosen on the **validation** split only, and the grid is
printed. FINAL is never read: the trainer refuses a dataset that contains a
FINAL row.

Output: `eval/dupbench/weights/v1.json` plus a validation decile calibration
table and the split measurements (recall@5, FAR, abstention) on stdout. The
measurement is reported, never turned into a verdict; the validation numbers
are model-selection data and are labelled as such.

Usage:
  python -I eval/dupbench/train_ranker.py
  python -I eval/dupbench/train_ranker.py --dataset target/ranker-data/dataset.jsonl \
      --out eval/dupbench/weights/v1.json
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent

SEED = 20261126
Z = 1.959964
# Documented missingness rule: a null call-graph distance is not a zero (zero
# means "same function", which always makes a shared-function pair) and is not
# comparable with a real distance. It gets the constant `MAX_CANDIDATE_DISTANCE
# + 1` and a companion missingness indicator, so the model can learn the
# "no path" effect instead of the value being silently imputed.
CALL_GRAPH_MISSING = 3.0

SCHEMA = "priorart.ranker-weights/v1"

# (name, kind, whether the log1p transform applies to this feature)
FEATURES = [
    ("anchor_function_overlap.shared", "count", True),
    ("anchor_function_overlap.ratio", "ratio", False),
    ("anchor_state_overlap.shared", "count", True),
    ("anchor_state_overlap.ratio", "ratio", False),
    ("call_graph_distance", "distance", False),
    ("call_graph_distance_missing", "derived", False),
    ("mechanism_match.shared_root_causes", "count", True),
    ("mechanism_match.same_contract", "binary", False),
    ("impact_match.shared_impacts", "count", True),
    ("identifier_score.specific_symbols", "count", True),
    ("identifier_score.shared_symbols", "count", True),
    ("identifier_score.rarity", "count", True),
    ("identifier_score.ranked", "binary", False),
]

LAMBDAS = [0.01, 0.1, 1.0, 10.0]
TRANSFORMS = ["identity", "log1p"]


def wilson(k: int, n: int) -> list[float] | None:
    if n == 0:
        return None
    p = k / n
    denom = 1 + Z * Z / n
    centre = (p + Z * Z / (2 * n)) / denom
    half = Z * math.sqrt(p * (1 - p) / n + Z * Z / (4 * n * n)) / denom
    return [round(max(0.0, centre - half), 4), round(min(1.0, centre + half), 4)]


def raw_vector(features: dict) -> list[float]:
    """The 13 raw model inputs from one recorded feature row."""
    cg = features["call_graph_distance"]
    return [
        float(features["anchor_function_overlap.shared"]),
        0.0 if features["anchor_function_overlap.ratio"] is None else float(features["anchor_function_overlap.ratio"]),
        float(features["anchor_state_overlap.shared"]),
        0.0 if features["anchor_state_overlap.ratio"] is None else float(features["anchor_state_overlap.ratio"]),
        CALL_GRAPH_MISSING if cg is None else float(cg),
        1.0 if cg is None else 0.0,
        float(features["mechanism_match.shared_root_causes"]),
        1.0 if features["mechanism_match.same_contract"] else 0.0,
        float(features["impact_match.shared_impacts"]),
        float(features["identifier_score.specific_symbols"]),
        float(features["identifier_score.shared_symbols"]),
        float(features["identifier_score.rarity"]),
        1.0 if features["identifier_score.ranked"] else 0.0,
    ]


def transform_vector(raw: list[float], transform: str) -> list[float]:
    out = []
    for (_, kind, loggable), value in zip(FEATURES, raw):
        if transform == "log1p" and loggable and kind == "count":
            out.append(math.log1p(value))
        else:
            out.append(value)
    return out


def fit_ridge(xs: list[list[float]], ys: list[int], lam: float, max_iter: int = 100) -> list[float]:
    """IRLS for ridge logistic regression. Returns [intercept, w1..wd]."""
    n = len(xs)
    d = len(xs[0])
    # Design matrix with a leading intercept column.
    rows = [[1.0] + row for row in xs]
    beta = [0.0] * (d + 1)
    p0 = (sum(ys) + 0.5) / (n + 1.0)
    beta[0] = math.log(p0 / (1.0 - p0))
    for _ in range(max_iter):
        probs = [sigmoid(dot(row, beta)) for row in rows]
        grad = [0.0] * (d + 1)
        for row, y, p in zip(rows, ys, probs):
            e = p - y
            for j in range(d + 1):
                grad[j] += e * row[j]
        for j in range(1, d + 1):
            grad[j] += lam * beta[j]
        hess = [[0.0] * (d + 1) for _ in range(d + 1)]
        for row, p in zip(rows, probs):
            w = max(p * (1.0 - p), 1e-12)
            wr = [w * v for v in row]
            for j in range(d + 1):
                wj = wr[j]
                if wj == 0.0:
                    continue
                hj = hess[j]
                for k in range(j, d + 1):
                    hj[k] += wj * row[k]
        for j in range(d + 1):
            for k in range(j):
                hess[j][k] = hess[k][j]
        for j in range(1, d + 1):
            hess[j][j] += lam
        delta = solve(hess, grad)
        if delta is None:
            raise SystemExit("ridge Hessian is singular; increase lambda")
        for j in range(d + 1):
            beta[j] -= delta[j]
        if max(abs(v) for v in delta) < 1e-10:
            break
    return beta


def dot(a: list[float], b: list[float]) -> float:
    return math.fsum(x * y for x, y in zip(a, b))


def solve(a: list[list[float]], b: list[float]) -> list[float] | None:
    """Gaussian elimination with partial pivoting; None when singular."""
    n = len(a)
    m = [row[:] + [b[i]] for i, row in enumerate(a)]
    for col in range(n):
        pivot = max(range(col, n), key=lambda r: abs(m[r][col]))
        if abs(m[pivot][col]) < 1e-14:
            return None
        m[col], m[pivot] = m[pivot], m[col]
        for r in range(col + 1, n):
            factor = m[r][col] / m[col][col]
            if factor == 0.0:
                continue
            for c in range(col, n + 1):
                m[r][c] -= factor * m[col][c]
    x = [0.0] * n
    for r in range(n - 1, -1, -1):
        acc = m[r][n] - math.fsum(m[r][c] * x[c] for c in range(r + 1, n))
        x[r] = acc / m[r][r]
    return x


def sigmoid(z: float) -> float:
    if z >= 0.0:
        return 1.0 / (1.0 + math.exp(-min(z, 700.0)))
    e = math.exp(max(z, -700.0))
    return e / (1.0 + e)


def log_loss(probs: list[float], ys: list[int]) -> float:
    total = 0.0
    for p, y in zip(probs, ys):
        p = min(max(p, 1e-15), 1.0 - 1e-15)
        total -= y * math.log(p) + (1 - y) * math.log(1.0 - p)
    return total / len(probs)


def auc(scores: list[float], ys: list[int]) -> float | None:
    """Rank AUC with average ranks for ties (pair rows only)."""
    positives = sum(ys)
    negatives = len(ys) - positives
    if positives == 0 or negatives == 0:
        return None
    order = sorted(range(len(scores)), key=lambda i: scores[i])
    ranks = [0.0] * len(scores)
    i = 0
    while i < len(order):
        j = i
        while j + 1 < len(order) and scores[order[j + 1]] == scores[order[i]]:
            j += 1
        avg = (i + j) / 2.0 + 1.0
        for k in range(i, j + 1):
            ranks[order[k]] = avg
        i = j + 1
    rank_sum = math.fsum(ranks[i] for i in range(len(scores)) if ys[i] == 1)
    return (rank_sum - positives * (positives + 1) / 2.0) / (positives * negatives)


class Spec:
    """The fitted transform for one feature."""

    def __init__(self, name: str, kind: str, loggable: bool, transform: str,
                 missing: float | None, mean: float, scale: float, coefficient: float):
        self.name = name
        self.kind = kind
        self.loggable = loggable
        self.transform = transform
        self.missing = missing
        self.mean = mean
        self.scale = scale
        self.coefficient = coefficient

    def to_json(self) -> dict:
        return {
            "name": self.name,
            "kind": self.kind,
            "transform": self.transform,
            "missing_value": self.missing,
            "mean": self.mean,
            "scale": self.scale,
            "coefficient": self.coefficient,
        }


def standardizer(xs: list[list[float]]) -> tuple[list[float], list[float]]:
    n = len(xs)
    d = len(xs[0])
    means = [math.fsum(row[j] for row in xs) / n for j in range(d)]
    scales = []
    for j in range(d):
        var = math.fsum((row[j] - means[j]) ** 2 for row in xs) / n
        scales.append(math.sqrt(var) if var > 0.0 else 1.0)
    return means, scales


def standardize(xs: list[list[float]], means: list[float], scales: list[float]) -> list[list[float]]:
    return [[(v - m) / s for v, m, s in zip(row, means, scales)] for row in xs]


def score_with(specs: list[Spec], intercept: float, features: dict) -> float:
    raw = raw_vector(features)
    z = intercept
    for spec, value in zip(specs, raw):
        if spec.transform == "log1p" and spec.loggable and spec.kind == "count":
            value = math.log1p(value)
        z += spec.coefficient * (value - spec.mean) / spec.scale
    return sigmoid(z)


def load_rows(path: Path) -> tuple[list[dict], list[dict], list[dict]]:
    queries, pairs, skips = [], [], []
    with path.open(encoding="utf-8") as handle:
        for line in handle:
            row = json.loads(line)
            if row.get("split") == "FINAL":
                raise SystemExit("the dataset contains a FINAL row; refusing to train on it")
            if row["kind"] == "pair":
                pairs.append(row)
            elif row["kind"] == "query":
                queries.append(row)
            else:
                skips.append(row)
    return queries, pairs, skips


def decile_table(scores: list[float], ys: list[int]) -> list[dict]:
    order = sorted(range(len(scores)), key=lambda i: (scores[i], i))
    n = len(order)
    table = []
    for b in range(10):
        lo, hi = b * n // 10, (b + 1) * n // 10
        idx = order[lo:hi]
        if not idx:
            table.append({"bucket": b + 1, "n": 0, "positives": 0, "rate": None,
                          "wilson95": None, "score_min": None, "score_max": None})
            continue
        pos = sum(ys[i] for i in idx)
        table.append({
            "bucket": b + 1,
            "n": len(idx),
            "positives": pos,
            "rate": round(pos / len(idx), 4),
            "wilson95": wilson(pos, len(idx)),
            "score_min": round(scores[idx[0]], 6),
            "score_max": round(scores[idx[-1]], 6),
        })
    return table


def monotone_within_sampling_error(table: list[dict]) -> tuple[bool, list[dict]]:
    """A bucket may not fall significantly below the one before it."""
    inversions = []
    defect = False
    prev = None
    for row in table:
        if row["n"] == 0:
            continue
        if prev is not None and row["rate"] < prev["rate"]:
            overlaps = prev["wilson95"][0] <= row["wilson95"][1]
            inversions.append({
                "lower_bucket": prev["bucket"],
                "higher_bucket": row["bucket"],
                "lower_rate": prev["rate"],
                "higher_rate": row["rate"],
                "within_sampling_error": overlaps,
            })
            if not overlaps:
                defect = True
        prev = row
    return defect, inversions


def measure(queries: list[dict], pairs: list[dict], specs: list[Spec], intercept: float,
            splits: list[str]) -> dict:
    by_query: dict[str, list[dict]] = {}
    for row in pairs:
        by_query.setdefault(row["query"], []).append(row)
    out = {}
    for split in splits:
        qs = [q for q in queries if q["split"] == split]
        dup = [q for q in qs if q["query_role"] == "duplicate"]
        prim = [q for q in qs if q["query_role"] == "primary"]
        base_hits = rank_hits = 0
        for q in dup:
            rows = by_query.get(q["query"], [])
            ranked = sorted(rows, key=lambda r: -score_with(specs, intercept, r["features"]))
            if any(r["label"] == 1 for r in rows[:5]):
                base_hits += 1
            if any(r["label"] == 1 for r in ranked[:5]):
                rank_hits += 1
        abstained = {
            role: sum(1 for q in qs if q["query_role"] == role and q["abstention"])
            for role in ("duplicate", "primary")
        }
        attempted = {"duplicate": len(dup), "primary": len(prim)}
        far = {}
        for threshold in (0.5, 0.7, 0.8, 0.9, 0.95):
            alarmed = 0
            for q in prim:
                if any(score_with(specs, intercept, r["features"]) >= threshold
                       for r in by_query.get(q["query"], [])):
                    alarmed += 1
            far[str(threshold)] = {
                "k": alarmed, "n": len(prim),
                "rate": round(alarmed / len(prim), 4) if prim else None,
                "wilson95": wilson(alarmed, len(prim)),
            }
        base_alarmed = sum(1 for q in prim if by_query.get(q["query"]))
        out[split] = {
            "queries_attempted": attempted,
            "abstention": {
                role: {
                    "k": abstained[role],
                    "n": attempted[role],
                    "rate": round(abstained[role] / attempted[role], 4) if attempted[role] else None,
                    "wilson95": wilson(abstained[role], attempted[role]),
                }
                for role in ("duplicate", "primary")
            },
            "recall_at_5_baseline": {
                "k": base_hits, "n": len(dup),
                "rate": round(base_hits / len(dup), 4) if dup else None,
                "wilson95": wilson(base_hits, len(dup)),
            },
            "recall_at_5_ranker": {
                "k": rank_hits, "n": len(dup),
                "rate": round(rank_hits / len(dup), 4) if dup else None,
                "wilson95": wilson(rank_hits, len(dup)),
            },
            "far_baseline_any_candidate": {
                "k": base_alarmed, "n": len(prim),
                "rate": round(base_alarmed / len(prim), 4) if prim else None,
                "wilson95": wilson(base_alarmed, len(prim)),
            },
            "far_ranker": far,
        }
    return out


def git_commit() -> str | None:
    proc = subprocess.run(
        ["git", "-C", str(ROOT), "rev-parse", "HEAD"],
        capture_output=True, text=True, encoding="utf-8", errors="replace",
    )
    return proc.stdout.strip() if proc.returncode == 0 else None


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--dataset", default=str(ROOT / "target" / "ranker-data" / "dataset.jsonl"))
    parser.add_argument("--out", default=str(HERE / "weights" / "v1.json"))
    parser.add_argument("--code-commit", default=None)
    args = parser.parse_args(argv[1:])

    dataset_path = Path(args.dataset)
    if not dataset_path.is_file():
        raise SystemExit(f"dataset not found: {dataset_path}")
    queries, pairs, skips = load_rows(dataset_path)
    train_rows = [r for r in pairs if r["split"] == "train"]
    val_rows = [r for r in pairs if r["split"] == "validation"]
    if not train_rows or not val_rows:
        raise SystemExit(
            f"need train and validation pair rows, got train={len(train_rows)} "
            f"validation={len(val_rows)}; refusing to fit"
        )
    train_pos = sum(r["label"] for r in train_rows)
    val_pos = sum(r["label"] for r in val_rows)
    print(f"train pairs={len(train_rows)} positives={train_pos} negatives={len(train_rows) - train_pos}")
    print(f"validation pairs={len(val_rows)} positives={val_pos} negatives={len(val_rows) - val_pos}")
    print(f"skipped records={len(skips)}", file=sys.stderr)

    if train_pos < 50:
        raise SystemExit(f"only {train_pos} training positives; too few to fit")

    ys_train = [r["label"] for r in train_rows]
    ys_val = [r["label"] for r in val_rows]
    raw_train = [raw_vector(r["features"]) for r in train_rows]
    raw_val = [raw_vector(r["features"]) for r in val_rows]

    grid = []
    best = None
    for transform in TRANSFORMS:
        xs_train_all = [transform_vector(raw, transform) for raw in raw_train]
        xs_val_all = [transform_vector(raw, transform) for raw in raw_val]
        means, scales = standardizer(xs_train_all)
        xs_train = standardize(xs_train_all, means, scales)
        xs_val = standardize(xs_val_all, means, scales)
        for lam in LAMBDAS:
            beta = fit_ridge(xs_train, ys_train, lam)
            val_probs = [sigmoid(dot([1.0] + row, beta)) for row in xs_val]
            train_probs = [sigmoid(dot([1.0] + row, beta)) for row in xs_train]
            entry = {
                "transform": transform,
                "lambda": lam,
                "train_log_loss": round(log_loss(train_probs, ys_train), 6),
                "validation_log_loss": round(log_loss(val_probs, ys_val), 6),
                "validation_auc": None if auc(val_probs, ys_val) is None else round(auc(val_probs, ys_val), 6),
            }
            grid.append(entry)
            key = (entry["validation_log_loss"], -lam)
            if best is None or key < best[0]:
                best = (key, entry, beta, means, scales, xs_train, xs_val)
            print(f"grid transform={transform} lambda={lam:<5} "
                  f"val_log_loss={entry['validation_log_loss']:.6f} "
                  f"val_auc={entry['validation_auc']}")
    _, chosen, beta, means, scales, xs_train, xs_val = best
    print(f"chosen transform={chosen['transform']} lambda={chosen['lambda']} "
          f"on validation log-loss (ties: more regularisation)")

    specs = []
    for (name, kind, loggable), mean, scale, coef in zip(FEATURES, means, scales, beta[1:]):
        transform = "log1p" if chosen["transform"] == "log1p" and loggable and kind == "count" else "identity"
        missing = None
        if name == "call_graph_distance":
            missing = CALL_GRAPH_MISSING
        elif name.endswith(".ratio"):
            missing = 0.0
        specs.append(Spec(name, kind, loggable, transform, missing, mean, scale, coef))
    intercept = beta[0]

    val_probs = [sigmoid(dot([1.0] + row, beta)) for row in xs_val]
    table = decile_table(val_probs, ys_val)
    defect, inversions = monotone_within_sampling_error(table)
    print("validation decile calibration (chosen model):")
    for row in table:
        print(f"  decile {row['bucket']:>2}  n={row['n']:>5}  positives={row['positives']:>4}  "
              f"rate={row['rate']}  wilson={row['wilson95']}  scores=[{row['score_min']}, {row['score_max']}]")
    if defect:
        print("CALIBRATION DEFECT: a higher decile falls below a lower one beyond sampling error")
        for inv in inversions:
            print(f"  inversion {inv}")
    else:
        print(f"calibration is non-decreasing within sampling error ({len(inversions)} near-inversions)")

    train_contests = sorted({r["contest"] for r in pairs if r["split"] == "train"})
    validation_contests = sorted({r["contest"] for r in pairs if r["split"] == "validation"})
    train_probs = [sigmoid(dot([1.0] + row, beta)) for row in xs_train]
    dataset_sha = hashlib.sha256(dataset_path.read_bytes()).hexdigest()
    measurement = measure(queries, pairs, specs, intercept, ["train", "validation"])
    weights = {
        "schema": SCHEMA,
        "version": 1,
        "created_by": "eval/dupbench/train_ranker.py",
        "seed": SEED,
        "code_commit": args.code_commit or git_commit(),
        "dataset_sha256": dataset_sha,
        "train_contests": train_contests,
        "validation_contests": validation_contests,
        "selection": {
            "criterion": "validation log-loss (ties: more regularisation)",
            "grid": grid,
            "chosen": {"transform": chosen["transform"], "lambda": chosen["lambda"]},
        },
        "features": [spec.to_json() for spec in specs],
        "intercept": intercept,
        "missingness_rule": (
            "A null call_graph_distance maps to missing_value 3.0 (MAX_CANDIDATE_DISTANCE 2 + 1) "
            "and sets call_graph_distance_missing to 1.0; a null overlap ratio maps to 0.0. "
            "The indicator is a model feature, never a silent zero."
        ),
        "in_sample": {
            "train_log_loss": round(log_loss(train_probs, ys_train), 6),
            "train_auc": None if auc(train_probs, ys_train) is None else round(auc(train_probs, ys_train), 6),
        },
        "validation": {
            "log_loss": chosen["validation_log_loss"],
            "auc": chosen["validation_auc"],
            "calibration_deciles": table,
            "calibration_defect": defect,
            "calibration_near_inversions": inversions,
        },
        "measurement": measurement,
    }
    out_path = Path(args.out)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(weights, indent=1) + "\n", encoding="utf-8", newline="\n")
    print(json.dumps(measurement, indent=1))
    print(f"weights: {out_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
