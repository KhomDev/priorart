#!/usr/bin/env python3
"""Score the public benchmark with an offline priorart binary. Offline.

    python eval/benchmark/fetch.py          # once, network
    python eval/benchmark/fetch.py --manifest
    python eval/benchmark/run.py --priorart target/release/priorart [--split dev]
    python eval/benchmark/run.py --self-test      # fixture binary, no CLI needed
    python eval/benchmark/run.py --check          # recompute from results.json

Three measurements, defined in BENCHMARK.md:

known_prior_recall  for findings the judge excluded as a repeat of one prior
                    item, is that prior item in the top 5? (n is tiny; the
                    script prints the reason rather than a rate when n < 5)
false_alarm_rate    for judged paid and unique findings, how often does the
                    matcher return any prior item (by construction >= 2 strong
                    identifiers)? Wilson 95% interval.
duplicate_recall    supplementary: for a submission the judge grouped with a
                    representative, is the representative in the top 5?

The tool and its rule logic are external. This script freezes the inputs
(manifest sha256), the seed and k in the result, so a later run is comparable.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

try:
    import yaml
except ImportError:
    print("eval/benchmark: PyYAML is required", file=sys.stderr)
    sys.exit(2)

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent                     # priorart/
CACHE = ROOT / "eval" / "cache" / "benchmark"
BUILD = CACHE / "build"
QUERIES = CACHE / "queries"
MANIFEST = HERE / "manifest.yaml"
LABELS = HERE / "labels.yaml"
RESULTS = HERE / "results.json"
FREEZE = HERE / "freeze.json"
K = 5
Z = 1.959964


def wilson(k: int, n: int) -> list[float] | None:
    if n == 0:
        return None
    p = k / n
    denom = 1 + Z * Z / n
    centre = (p + Z * Z / (2 * n)) / denom
    half = Z * math.sqrt(p * (1 - p) / n + Z * Z / (4 * n * n)) / denom
    return [round(max(0.0, centre - half), 4), round(min(1.0, centre + half), 4)]


def sha256_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def sha256_text(text: str) -> str:
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def split_of(seed: str, item_id: str) -> str:
    """Deterministic dev/heldout assignment; frozen, never changed by a run."""
    digest = hashlib.sha256(f"{seed}:{item_id}".encode()).hexdigest()
    return "heldout" if int(digest[:8], 16) % 2 == 1 else "dev"


def in_split(split: str, seed: str, item_id: str) -> bool:
    return split == "all" or split_of(seed, item_id) == split


def write_dossier_a(manifest: dict) -> Path:
    """Dossier A: the prior audit, one file per item, plus the sponsor's known
    issues. One item per file so the item boundary does not depend on how the
    tool splits a large markdown file."""
    dest = BUILD / "dossier-a"
    if dest.exists():
        shutil.rmtree(dest)
    (dest / "audits").mkdir(parents=True)
    audit = (CACHE / "c4-2023-07-tapioca-audit" / "report.md").read_text(encoding="utf-8")
    import re
    heads = list(re.finditer(r"^##\s+\[\[(H|M)-(\d+)\]\s+(.+?)\]\(#?", audit, re.M))
    for i, h in enumerate(heads):
        end = heads[i + 1].start() if i + 1 < len(heads) else len(audit)
        locator = f"{h.group(1)}-{int(h.group(2)):02d}"
        body = audit[h.start():end].strip()
        (dest / "audits" / f"c4-2023-07-{locator}.md").write_text(body + "\n", encoding="utf-8", newline="\n")
    readme = (CACHE / "sherlock-2024-02-tapioca-contest" / "README.md").read_text(encoding="utf-8")
    m = re.search(r"(?ims)^#{1,4}\s*Q:\s*Please list any known issues.*?\n(.*?)(?=^___|\n#{1,4}\s*Q:|\Z)", readme)
    ki = ("# Known issues / acceptable risks\n\n" + (m.group(1).strip() if m else "not found")) + "\n"
    (dest / "known-issues.md").write_text(ki, encoding="utf-8", newline="\n")
    (dest / "program.yaml").write_text(
        "name: tapioca-sherlock-2024-02\n"
        "platform: sherlock\n"
        "url: https://audits.sherlock.xyz/contests/298\n"
        "known_issues: known-issues.md\n",
        encoding="utf-8", newline="\n")
    return dest


def write_dossier_b(manifest: dict) -> Path:
    """Dossier B (supplementary duplicate variant): one file per judged
    representative, so each representative is one prior item."""
    dest = BUILD / "dossier-b"
    if dest.exists():
        shutil.rmtree(dest)
    (dest / "audits").mkdir(parents=True)
    src = CACHE / "sherlock-2024-02-tapioca-judging"
    for row in manifest["findings"]:
        text = (src / row["pointer"]["path"]).read_text(encoding="utf-8", errors="replace")
        head = f"## [[{row['id']}]] {row.get('title', '')}\n\n"
        (dest / "audits" / (row["id"].replace(":", "-") + ".md")).write_text(
            head + text, encoding="utf-8", newline="\n")
    (dest / "program.yaml").write_text(
        "name: tapioca-sherlock-2024-02-judged\nplatform: sherlock\n"
        "known_issues: none\n", encoding="utf-8", newline="\n")
    (dest / "known-issues.md").write_text("# Known issues\n\nnone\n", encoding="utf-8", newline="\n")
    return dest


def run(cmd: list[str], timeout: int = 120) -> subprocess.CompletedProcess:
    return subprocess.run(cmd, capture_output=True, text=True, encoding="utf-8", errors="replace", timeout=timeout)


def build_dossier(binary: str, program_dir: Path, out_file: Path) -> dict:
    if out_file.exists():
        out_file.unlink()
    proc = run([binary, "build", str(program_dir), "--out", str(out_file), "--json"])
    if proc.returncode != 0:
        raise RuntimeError(f"priorart build failed ({proc.returncode}): {proc.stderr[:400]}")
    if not out_file.is_file():
        raise RuntimeError(f"priorart build did not write {out_file}")
    return json.loads(out_file.read_text(encoding="utf-8"))


class ToolFailure(RuntimeError):
    """A single query the tool could not answer (crash, parse error). The run
    continues and the query is reported in errors, not scored."""


def check_one(binary: str, finding_path: Path, dossier: Path) -> dict:
    proc = run([binary, "check", str(finding_path), "--dossier", str(dossier), "--top-k", str(K), "--json"])
    if proc.returncode not in (0, 1):
        raise ToolFailure(f"exit {proc.returncode}: {(proc.stderr or '').strip().splitlines()[:2]}")
    try:
        return json.loads(proc.stdout)
    except json.JSONDecodeError as exc:
        raise ToolFailure(f"non-JSON output: {exc}; head={proc.stdout[:160]}")


def prior_text(item: dict) -> str:
    return " ".join(str(item.get(k) or "") for k in
                    ("item_id", "id", "source_id", "source_title", "title", "section", "quote", "text"))


def norm(value: object) -> str:
    return str(value or "").replace(":", "-").lower()


def matches(expected: dict, item: dict) -> bool:
    """Match a returned prior item to the expected locator by text, because the
    dossier item ids are generated by the tool. The locators are the report's
    own ids (H-01, KI-1) or the judged finding id (acc:001-M). Colon and dash
    are both used for the judged id depending on the file name, so normalise."""
    text = norm(prior_text(item))
    locator = norm(expected.get("locator") or "")
    if not locator:
        return False
    if locator in text:
        return True
    for alias in expected.get("aliases") or []:
        if norm(alias) and norm(alias) in text:
            return True
    # section ids may be normalised (H-1 vs H-01); compare the numeric form too
    if "-" in locator:
        head, _, tail = locator.partition("-")
        if tail.isdigit():
            for width in (1, 2):
                if f"{head}-{int(tail):0{width}d}" in text:
                    return True
            if f"{head}{int(tail)}" in text:
                return True
    return False


def evaluate(manifest: dict, binary: str, split: str, version: str) -> dict:
    dossier_a = write_dossier_a(manifest)
    dossier_b = write_dossier_b(manifest)
    QUERIES.mkdir(parents=True, exist_ok=True)
    build_dossier(binary, dossier_a, BUILD / "dossier-a.dossier")
    build_dossier(binary, dossier_b, BUILD / "dossier-b.dossier")

    priors = {p["id"]: p for p in manifest["prior_items"]["prior_audit_and_known"]}
    rows = {r["id"]: r for r in manifest["findings"]}
    per_query = []

    errors = []

    def ask(qid: str, source_path: Path, dossier: Path) -> list[dict] | None:
        target = QUERIES / (qid.replace(":", "_").replace("/", "_") + ".md")
        target.write_text(source_path.read_text(encoding="utf-8", errors="replace"), encoding="utf-8", newline="\n")
        try:
            report = check_one(binary, target, dossier)
        except ToolFailure as exc:
            errors.append({"id": qid, "error": str(exc)})
            return None
        return report.get("prior_art", [])

    # 1. known-prior recall
    kp = [r for r in rows.values() if r["label"] == "known_prior" and in_split(split, manifest["seed"], r["id"])]
    kp_hits = 0
    kp_scored = 0
    for row in kp:
        got = ask(row["id"], CACHE / "sherlock-2024-02-tapioca-judging" / row["pointer"]["path"], BUILD / "dossier-a.dossier")
        if got is None:
            continue
        kp_scored += 1
        expected = priors.get(row["prior_ref"])
        hit = bool(expected) and any(matches(expected, item) for item in got[:K])
        kp_hits += hit
        per_query.append({"id": row["id"], "measure": "known_prior_recall", "hit": hit,
                          "expected": row["prior_ref"], "returned": [i.get("item_id") for i in got]})

    # 2. false alarms on judged paid and unique findings
    uv = [r for r in rows.values() if r["label"] == "unique_valid" and in_split(split, manifest["seed"], r["id"])]
    uv_alarms = 0
    uv_scored = 0
    for row in uv:
        got = ask(row["id"], CACHE / "sherlock-2024-02-tapioca-judging" / row["pointer"]["path"], BUILD / "dossier-a.dossier")
        if got is None:
            continue
        uv_scored += 1
        alarm = len(got) > 0
        uv_alarms += alarm
        per_query.append({"id": row["id"], "measure": "false_alarm_rate", "alarm": alarm,
                          "returned": [i.get("item_id") for i in got]})

    # 3. supplementary duplicate retrieval
    dup = [d for d in manifest["duplicates"] if in_split(split, manifest["seed"], d["id"])]
    dup_hits = 0
    dup_scored = 0
    for d in dup:
        got = ask(d["id"], CACHE / "sherlock-2024-02-tapioca-judging" / d["pointer"]["path"], BUILD / "dossier-b.dossier")
        if got is None:
            continue
        dup_scored += 1
        expected = {"locator": d["expected"]}
        hit = any(matches(expected, item) for item in got[:K])
        dup_hits += hit
        per_query.append({"id": d["id"], "measure": "duplicate_recall", "hit": hit,
                          "expected": d["expected"], "returned": [i.get("item_id") for i in got]})

    return {
        "known_prior_recall": {
            "n": kp_scored, "hits": kp_hits,
            "recall": round(kp_hits / kp_scored, 4) if kp_scored else None,
            "wilson95": wilson(kp_hits, kp_scored),
            "status": "measured" if kp_scored >= 5 else "underpowered",
            "reason": None if len(kp) >= 5 else (
                "fewer than 5 defensible known-prior positives exist in this corpus; "
                "the value is reported with its n but is not a rate (see BENCHMARK.md dead ends)"),
        },
        "false_alarm_rate": {
            "n": uv_scored, "alarms": uv_alarms,
            "rate": round(uv_alarms / uv_scored, 4) if uv_scored else None,
            "wilson95": wilson(uv_alarms, uv_scored),
            "status": "measured" if uv_scored else "not_measured",
            "tool_errors": len(errors),
        },
        "duplicate_recall": {
            "n": dup_scored, "hits": dup_hits,
            "recall": round(dup_hits / dup_scored, 4) if dup_scored else None,
            "wilson95": wilson(dup_hits, dup_scored),
            "status": "measured" if dup_scored else "not_measured",
            "note": "supplementary judged-duplicate retrieval, not the plan's known-issue metric",
        },
        "per_query": per_query,
        "errors": errors,
        "tool_version": version,
    }


def label_agreement() -> dict:
    labels = yaml.safe_load(LABELS.read_text(encoding="utf-8"))
    findings = labels.get("findings", {})
    agreement = labels.get("agreement", {})
    sample = agreement.get("sample", [])
    blind = agreement.get("blind", {})
    agree = sum(1 for i in sample if findings.get(i, {}).get("label") == blind.get(i))
    return {
        "n": len(sample), "agree": agree,
        "rate": round(agree / len(sample), 4) if sample else None,
        "wilson95": wilson(agree, len(sample)),
        "method": agreement.get("method"),
        "note": "label stability of one reader, not correctness; the labels and the sample are published",
    }


def corpus_facts() -> dict:
    manifest = yaml.safe_load(MANIFEST.read_text(encoding="utf-8"))
    return {
        "manifest_sha256": sha256_file(MANIFEST),
        "labels_sha256": sha256_file(LABELS),
        "sources": manifest["sources"],
        "prior_items": manifest["counts"]["prior_items"],
        "findings": manifest["counts"]["findings"],
        "accepted": manifest["counts"]["accepted"],
        "invalid": manifest["counts"]["invalid"],
        "duplicates": manifest["counts"]["duplicates"],
        "known_prior": manifest["counts"]["known_prior"],
        "seed": manifest["seed"],
    }


def write_result(doc: dict) -> None:
    RESULTS.write_text(json.dumps(doc, indent=2) + "\n", encoding="utf-8", newline="\n")


def blocked(reason: str) -> int:
    """No tool, no measurement. Still publish the corpus facts and labels: the
    difference between 'not run' and 'run and bad' must be visible."""
    doc = {
        "schema": "priorart.benchmark.results/v1",
        "generated_by": "eval/benchmark/run.py",
        "corpus": corpus_facts(),
        "label_agreement": label_agreement(),
        "tool": None,
        "frozen": {"k": K, "seed": yaml.safe_load(MANIFEST.read_text(encoding="utf-8"))["seed"],
                   "manifest_sha256": sha256_file(MANIFEST), "heldout_opened": False,
                   "binary_sha256": None},
        "measurements": {name: {"status": "not_measured", "reason": reason, "n": None}
                         for name in ("known_prior_recall", "false_alarm_rate", "duplicate_recall")},
        "not_claimed": NOT_CLAIMED,
    }
    write_result(doc)
    print(json.dumps({"status": "not_measured", "reason": reason,
                      "corpus": doc["corpus"], "label_agreement": doc["label_agreement"]}, indent=2))
    return 1


RUN_HISTORY = [
    {
        "binary_sha256": "22648e333dbde7e9d9ea0982cdc93ed7581ba1d8abd4045f29650994cd03ce82",
        "label": "pre-fix matcher",
        "date": "2026-11-12",
        "far": {"n": 60, "alarms": 18, "rate": 0.3, "wilson95": [0.199, 0.4251]},
        "duplicate_recall": {"n": 64, "hits": 36, "recall": 0.5625, "wilson95": [0.4409, 0.6771]},
        "known_prior_recall": {"n": 1, "hits": 0},
        "tool_errors": 0,
    },
    {
        "binary_sha256": "da176bc86b86b9b81dcb3a3095dabeef00768ab8f4baf04c4b23e1e11af9e8db",
        "label": "intermediate matcher",
        "date": "2026-11-13",
        "far": {"n": 60, "alarms": 15, "rate": 0.25, "wilson95": [0.1578, 0.3723]},
        "duplicate_recall": {"n": 64, "hits": 33, "recall": 0.5156, "wilson95": [0.3958, 0.6337]},
        "known_prior_recall": {"n": 1, "hits": 0},
        "tool_errors": 0,
    },
    {
        "binary_sha256": "176f19869065b1a27caa0f8c91c91b7790f248b20dee621b869cdf2e309e7d0c",
        "label": "superseded by dabfcc0c (one prose-only fix; same matcher and numbers)",
        "date": "2026-11-13",
        "far": {"n": 60, "alarms": 26, "rate": 0.4333, "wilson95": [0.3157, 0.559]},
        "duplicate_recall": {"n": 64, "hits": 39, "recall": 0.6094, "wilson95": [0.4869, 0.7194]},
        "known_prior_recall": {"n": 1, "hits": 0},
        "tool_errors": 0,
    },
    {
        "binary_sha256": "dabfcc0ca197ee35b40772cb7db666f17105c5c36faa7f9da7ef783a9a78e12a",
        "label": "FINAL (green tree, 91 tests; matcher identical to 2ec4c3c9, stale-prose wording only after it)",
        "date": "2026-11-13",
        "far": {"n": 60, "alarms": 26, "rate": 0.4333, "wilson95": [0.3157, 0.559]},
        "duplicate_recall": {"n": 64, "hits": 39, "recall": 0.6094, "wilson95": [0.4869, 0.7194]},
        "known_prior_recall": {"n": 1, "hits": 0},
        "tool_errors": 0,
        "note": "number of record; the plan's >30% rule applies to this run",
    },
]

# The debug binary is what this harness measured. It is not the crates.io or
# release artefact: those are release-profile builds with different hashes. The
# durable provenance is the source commit (and tag) the release is cut from, so
# the fields are recorded here once the Lead tags, and are null until then.
RELEASE_PROVENANCE = {
    "measured_binary_sha256": "dabfcc0ca197ee35b40772cb7db666f17105c5c36faa7f9da7ef783a9a78e12a",
    "release_binary_sha256": None,
    "source_commit": None,
    "tag": None,
    "status": "pending: the release tree is uncommitted at the time of measurement "
              "(priorart HEAD f715c07 with 226 changed/untracked paths), so no commit "
              "or tag exists yet. Record the commit and tag here when the Lead cuts the "
              "release; the release-profile binary hash will differ from the debug hash.",
}

NOT_CLAIMED = [
    "not a measurement of whether priorart finds bugs: the corpus is judged human findings",
    "not a false-positive rate on model-written hypotheses",
    "not a duplicate-detector score for submissions against public reports: the duplicate variant is a reading list and never a duplicate verdict",
    "the known-prior positives are one reader's grounding, not the judge's published reason",
    "no held-out split was opened",
]


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description="score the priorart public benchmark (offline)")
    parser.add_argument("--priorart", default=str(ROOT / "target" / "release" / "priorart"))
    parser.add_argument("--split", choices=["dev", "heldout", "all"], default="dev",
                        help="all = dev + held-out, allowed only after freeze.json exists")
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args(argv)

    if args.check:
        doc = json.loads(RESULTS.read_text(encoding="utf-8"))
        got = label_agreement()
        if doc.get("label_agreement", {}).get("agree") != got["agree"]:
            print("eval/benchmark check failed: label agreement drifted", file=sys.stderr)
            return 1
        print(f"eval/benchmark check ok: label agreement {got['agree']}/{got['n']}, "
              f"measurements {sorted(doc.get('measurements', {}))}")
        return 0

    if args.self_test:
        return self_test()

    if not MANIFEST.is_file():
        return blocked("manifest.yaml absent; run fetch.py --manifest")
    if args.split in ("heldout", "all"):
        if not FREEZE.is_file():
            print("refusing to open the held-out split before the rule logic is frozen: "
                  "run --split dev first (writes freeze.json)", file=sys.stderr)
            return 2
        frozen = json.loads(FREEZE.read_text(encoding="utf-8"))
        if frozen.get("manifest_sha256") != sha256_file(MANIFEST):
            print("refusing: manifest changed since freeze.json was written", file=sys.stderr)
            return 2
    binary = args.priorart
    if not Path(binary).is_file():
        return blocked(f"priorart binary not found at {binary}; the benchmark is not run until it exists")
    version = run([binary, "--version"]).stdout.strip()
    manifest = yaml.safe_load(MANIFEST.read_text(encoding="utf-8"))
    measurements = evaluate(manifest, binary, args.split, version)
    doc = {
        "schema": "priorart.benchmark.results/v1",
        "generated_by": "eval/benchmark/run.py",
        "corpus": corpus_facts(),
        "label_agreement": label_agreement(),
        "tool": {"path": binary, "version": version,
                 "binary_sha256": sha256_file(Path(binary)),
                 "matching_rule": ">= 2 strong identifiers, ranked top-k (EXECUTION-BRIEF 3.3)"},
        "frozen": {"k": K, "seed": manifest["seed"], "manifest_sha256": sha256_file(MANIFEST),
                   "heldout_opened": args.split in ("heldout", "all"),
                   "harness_sha256": sha256_file(HERE / "run.py")},
        "measurements": {k: v for k, v in measurements.items() if k not in ("per_query", "tool_version", "errors")},
        "run_history": RUN_HISTORY,
        "release": RELEASE_PROVENANCE,
        "per_query": measurements["per_query"],
        "tool_errors": measurements["errors"],
        "not_claimed": NOT_CLAIMED,
    }
    write_result(doc)
    if args.split == "dev" and not FREEZE.is_file():
        FREEZE.write_text(json.dumps({
            "frozen_at": "written by eval/benchmark/run.py --split dev",
            "manifest_sha256": sha256_file(MANIFEST),
            "labels_sha256": sha256_file(LABELS),
            "harness_sha256": sha256_file(HERE / "run.py"),
            "seed": manifest["seed"], "k": K,
        }, indent=2) + "\n", encoding="utf-8", newline="\n")
    print(json.dumps(doc["measurements"], indent=2))
    return 0


# --------------------------------------------------------------------------
# Self-test: a fake priorart binary and a six-query fixture corpus. It checks
# the arithmetic, the locator matching and the split, not the tool.
# --------------------------------------------------------------------------

SELFTEST_MANIFEST = {
    "version": 1, "seed": "selftest-seed", "sources": {},
    "prior_items": {
        "prior_audit_and_known": [
            {"id": "c4:H-01", "source": "c4", "locator": "H-01", "title": "first"},
            {"id": "ki:1", "source": "ki", "locator": "KI-1", "title": "zero address"},
        ],
        "judged_primary": [],
    },
    "findings": [
        {"id": "q1", "judged": "invalid", "label": "known_prior", "prior_ref": "c4:H-01",
         "pointer": {"path": "q1.md"}},
        {"id": "q2", "judged": "invalid", "label": "known_prior", "prior_ref": "c4:H-01",
         "pointer": {"path": "q2.md"}},
        {"id": "q3", "judged": "invalid", "label": "known_prior", "prior_ref": "ki:1",
         "pointer": {"path": "q3.md"}},
        {"id": "q4", "judged": "invalid", "label": "known_prior", "prior_ref": "ki:1",
         "pointer": {"path": "q4.md"}},
        {"id": "n1", "judged": "accepted", "label": "unique_valid", "pointer": {"path": "n1.md"}},
        {"id": "n2", "judged": "accepted", "label": "unique_valid", "pointer": {"path": "n2.md"}},
    ],
    "duplicates": [
        {"id": "d1", "expected": "n1", "pointer": {"path": "d1.md"}},
        {"id": "d2", "expected": "n2", "pointer": {"path": "d2.md"}},
    ],
}


def fake_spec() -> dict:
    """Answers the fake binary gives: q1 and q3 hit, q2 and q4 miss, n1 raises a
    prior item (false alarm), n2 clean, d1 hits, d2 misses."""
    return {
        "prior_items": [
            {"id": "H-01", "source_id": "c4-2023-07-tapioca-audit", "title": "H-01", "quote": "H-01 first"},
            {"id": "KI-1", "source_id": "known-issues", "title": "KI-1 zero address", "quote": "KI-1 zero address"},
            {"id": "n1", "source_id": "judged-primary", "title": "n1", "quote": "n1"},
            {"id": "n2", "source_id": "judged-primary", "title": "n2", "quote": "n2"},
        ],
        "checks": {
            "q1": ["H-01", "KI-1"],
            "q2": ["KI-1"],
            "q3": ["KI-1", "H-01"],
            "q4": ["H-01"],
            "n1": ["H-01"],
            "n2": [],
            "d1": ["n1"],
            "d2": ["n1"],
        },
    }


def self_test() -> int:
    """Run the pipeline on a fixture corpus with a fake binary, assert the
    expected numbers, then restore nothing (it all lives under a temp dir)."""
    tmp = Path(tempfile.mkdtemp(prefix="priorart-bench-selftest-"))
    saved = (CACHE, BUILD, QUERIES)
    saved_spec = os.environ.get("FAKE_PRIORART_SPEC")
    try:
        spec_path = tmp / "fake-spec.json"
        spec_path.write_text(json.dumps(fake_spec()), encoding="utf-8")
        fake = tmp / "fake_priorart.py"
        fake.write_text(FAKE_BINARY, encoding="utf-8")
        cache = tmp / "cache"
        (cache / "sherlock-2024-02-tapioca-judging").mkdir(parents=True)
        audit = cache / "c4-2023-07-tapioca-audit"
        audit.mkdir(parents=True)
        (audit / "report.md").write_text(
            "## [[H-01] first](https://example.invalid/1)\n\nbody\n", encoding="utf-8")
        contest = cache / "sherlock-2024-02-tapioca-contest"
        contest.mkdir(parents=True)
        (contest / "README.md").write_text(
            "# T\n\n### Q: Please list any known issues/acceptable risks\nZero address check\n___\n",
            encoding="utf-8")
        for row in SELFTEST_MANIFEST["findings"] + SELFTEST_MANIFEST["duplicates"]:
            (cache / "sherlock-2024-02-tapioca-judging" / row["pointer"]["path"]).write_text(
                "# fixture\n", encoding="utf-8")
        globals()["CACHE"], globals()["BUILD"], globals()["QUERIES"] = cache, cache / "build", cache / "queries"
        if os.name == "nt":
            binary = tmp / "fake.cmd"
            binary.write_text("@echo off\r\npython \"" + str(fake) + "\" %*\r\n", encoding="utf-8")
        else:
            fake.chmod(0o755)
            binary = fake
        os.environ["FAKE_PRIORART_SPEC"] = str(spec_path)
        got = evaluate(SELFTEST_MANIFEST, str(binary), "dev", "fake-0.0.0")
        # split_of("selftest-seed", id) decides dev/heldout; recompute what was run.
        dev_kp = [r["id"] for r in SELFTEST_MANIFEST["findings"] if r.get("label") == "known_prior"
                  and split_of("selftest-seed", r["id"]) == "dev"]
        dev_uv = [r["id"] for r in SELFTEST_MANIFEST["findings"] if r.get("label") == "unique_valid"
                  and split_of("selftest-seed", r["id"]) == "dev"]
        dev_dup = [d["id"] for d in SELFTEST_MANIFEST["duplicates"]
                   if split_of("selftest-seed", d["id"]) == "dev"]
        expected_hits = sum(1 for i in dev_kp if i in ("q1", "q3"))
        expected_alarms = 1 if "n1" in dev_uv else 0
        expected_dups = sum(1 for i in dev_dup if i == "d1")
        problems = []
        if got["known_prior_recall"]["n"] != len(dev_kp):
            problems.append(f"known_prior n {got['known_prior_recall']['n']} != {len(dev_kp)}")
        if got["known_prior_recall"]["hits"] != expected_hits:
            problems.append(f"known_prior hits {got['known_prior_recall']['hits']} != {expected_hits}")
        if got["false_alarm_rate"]["n"] != len(dev_uv):
            problems.append(f"false alarm n {got['false_alarm_rate']['n']} != {len(dev_uv)}")
        if got["false_alarm_rate"]["alarms"] != expected_alarms:
            problems.append(f"false alarms {got['false_alarm_rate']['alarms']} != {expected_alarms}")
        if got["duplicate_recall"]["hits"] != expected_dups:
            problems.append(f"duplicate hits {got['duplicate_recall']['hits']} != {expected_dups}")
        if problems:
            for p in problems:
                print("SELFTEST FAIL " + p, file=sys.stderr)
            return 1
        print(json.dumps({
            "self_test": "pass",
            "dev_split": {"known_prior": dev_kp, "unique_valid": dev_uv, "duplicates": dev_dup},
            "measurements": {k: got[k] for k in ("known_prior_recall", "false_alarm_rate", "duplicate_recall")},
        }, indent=2))
        return 0
    finally:
        globals()["CACHE"], globals()["BUILD"], globals()["QUERIES"] = saved
        if saved_spec is None:
            os.environ.pop("FAKE_PRIORART_SPEC", None)
        else:
            os.environ["FAKE_PRIORART_SPEC"] = saved_spec
        shutil.rmtree(tmp, ignore_errors=True)


FAKE_BINARY = '''#!/usr/bin/env python3
import json, os, sys

spec = json.load(open(os.environ["FAKE_PRIORART_SPEC"], encoding="utf-8"))
cmd = sys.argv[1]
if cmd == "build":
    out = sys.argv[sys.argv.index("--out") + 1]
    json.dump({"schema": "priorart.dossier/v1", "items": spec["prior_items"]}, open(out, "w", encoding="utf-8"))
    print(json.dumps({"dossier": out}))
elif cmd == "check":
    finding = sys.argv[2]
    name = os.path.basename(finding)[:-3]
    ids = spec["checks"].get(name, [])
    items = [i for i in spec["prior_items"] if i["id"] in ids]
    top = int(sys.argv[sys.argv.index("--top-k") + 1])
    print(json.dumps({"schema": "priorart.check/v1", "prior_art": items[:top],
                      "finding": name, "dossier": "", "exclusions": [], "payout": {}, "notes": []}))
elif cmd == "--version":
    print("fake-0.0.0")
else:
    print("usage", file=sys.stderr); sys.exit(2)
'''


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
