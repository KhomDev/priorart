#!/usr/bin/env python3
"""Download a public judged-findings set and score it with vg.

This script uses the network. `vg` does not. The bodies are written under
eval/cache/ which is gitignored. What gets committed is the manifest (urls,
labels, sha256) and the aggregate results.

Usage:
    python eval/run.py fetch          # download, write dataset.manifest.yaml
    python eval/run.py score          # run vg on the cache, write eval/results/
    python eval/run.py all            # fetch then score
    python eval/run.py check          # no network: results add up

`gh` must be authenticated for fetch. score needs a built vg binary
(target/release/vg by default).
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import subprocess
import sys
import time
import urllib.error
import urllib.request
from collections import Counter, defaultdict
from pathlib import Path

try:
    import yaml
except ImportError:
    print("eval: PyYAML is required", file=sys.stderr)
    sys.exit(2)

ROOT = Path(__file__).resolve().parent.parent
EVAL = ROOT / "eval"
CACHE = EVAL / "cache"
SOURCES = EVAL / "sources.yaml"
MANIFEST = EVAL / "dataset.manifest.yaml"
RESULTS = EVAL / "results"

# Labels that mean "this is a duplicate of another issue", not a primary.
def is_dup_label(name: str) -> bool:
    low = name.lower()
    return low == "duplicate" or low.startswith("duplicate-")


def gh_token() -> str:
    env = os.environ.get("GH_TOKEN") or os.environ.get("GITHUB_TOKEN")
    if env:
        return env
    return subprocess.check_output(["gh", "auth", "token"], text=True).strip()


def get_json(url: str, token: str) -> tuple[object, str]:
    last = None
    for attempt in range(5):
        req = urllib.request.Request(
            url,
            headers={
                "Authorization": f"Bearer {token}",
                "Accept": "application/vnd.github+json",
                "User-Agent": "viability-gate-eval",
                "X-GitHub-Api-Version": "2022-11-28",
            },
        )
        try:
            with urllib.request.urlopen(req, timeout=60) as resp:
                body = resp.read()
                link = resp.headers.get("Link", "")
                return json.loads(body.decode("utf-8")), link
        except urllib.error.HTTPError as exc:
            last = exc
            if exc.code in (403, 429, 502, 503):
                time.sleep(2 ** attempt)
                continue
            raise
    raise RuntimeError(f"GET {url} failed: {last}")


def next_url(link: str) -> str | None:
    for part in link.split(","):
        if 'rel="next"' in part:
            return part.split(";")[0].strip().strip("<>")
    return None


def issues_with_label(repo: str, label: str, token: str) -> list[dict]:
    from urllib.parse import quote

    url = (
        f"https://api.github.com/repos/{repo}/issues"
        f"?state=all&per_page=100&labels={quote(label)}"
    )
    out: list[dict] = []
    while url:
        page, link = get_json(url, token)
        if not isinstance(page, list):
            raise RuntimeError(f"{repo} label {label!r}: {page}")
        # The issues API also returns pull requests. Contest repos should not
        # have any, but skip them if one appears.
        out.extend(item for item in page if "pull_request" not in item)
        url = next_url(link)
    return out


def classify_sherlock(labels: list[str]) -> str | None:
    names = set(labels)
    if "Invalid" in names:
        return "invalid"
    if "Won't Fix" in names:
        return "wontfix"
    if ("High" in names or "Medium" in names) and "Reward" in names and "Duplicate" not in names:
        return "accepted"
    if "Low/Info" in names and "Reward" not in names and "High" not in names and "Medium" not in names:
        return "informative"
    return None


def classify_code4rena(labels: list[str]) -> str | None:
    low = [n.lower() for n in labels]
    if any(is_dup_label(n) for n in low):
        dup = True
    else:
        dup = False
    # "unsatisfactory" is a quality label, not a judged invalid. It is not
    # used as the negative class: a repo can have hundreds of them, and they
    # would crowd out issues the platform actually labeled invalid.
    if "invalid" in low or "out of scope" in low:
        if not dup:
            return "invalid"
    if any(n.startswith("qa") for n in low) and "selected for report" not in low and not dup:
        return "qa"
    if dup or "downgraded by judge" in low:
        return None
    if "selected for report" not in low:
        return None
    if "invalid" in low or "out of scope" in low:
        return None
    high = any("high" in n for n in low)
    med = any("med" in n for n in low)
    if high or med:
        return "accepted"
    return None


def severity_of(labels: list[str], klass: str) -> str:
    low = [n.lower() for n in labels]
    if any(n == "high" or "high" in n for n in low):
        return "high"
    if any(n == "medium" or "med" in n for n in low):
        return "medium"
    if klass == "qa":
        return "qa"
    if klass == "informative":
        return "low_or_info"
    if klass == "invalid":
        return "invalid"
    if klass == "wontfix":
        return "wontfix"
    return "unspecified"


def sha256(text: str) -> str:
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def normalise(body: str) -> str:
    return body.replace("\r\n", "\n").replace("\r", "\n")


def fetch(argv: list[str]) -> int:
    del argv
    spec = yaml.safe_load(SOURCES.read_text(encoding="utf-8"))
    token = gh_token()
    CACHE.mkdir(parents=True, exist_ok=True)
    buckets: dict[str, list[dict]] = defaultdict(list)
    seen: set[tuple[str, int]] = set()
    for repo in spec["repos"]:
        name = repo["name"]
        kind = repo["kind"]
        print(f"fetch {name}", flush=True)
        if kind == "sherlock":
            labels = ["High", "Medium", "Invalid", "Won't Fix", "Low/Info"]
            classify = classify_sherlock
        elif kind == "code4rena":
            labels = ["selected for report", "invalid", "QA (Quality Assurance)", "out of scope", "unsatisfactory"]
            classify = classify_code4rena
        else:
            raise SystemExit(f"unknown kind {kind}")
        by_number: dict[int, dict] = {}
        for label in labels:
            try:
                batch = issues_with_label(name, label, token)
            except (urllib.error.HTTPError, RuntimeError) as exc:
                print(f"  skip {label}: {exc}", flush=True)
                continue
            print(f"  {label}: {len(batch)}", flush=True)
            for item in batch:
                by_number[item["number"]] = item
        for number, item in sorted(by_number.items()):
            key = (name, number)
            if key in seen:
                continue
            label_names = [l["name"] for l in item.get("labels", [])]
            klass = classify(label_names)
            if klass is None:
                continue
            body = item.get("body") or ""
            body = normalise(body).strip()
            if len(body) < 80:
                continue
            seen.add(key)
            digest = sha256(body)
            title = (item.get("title") or "").replace("\n", " ").strip()
            if "@" in title and "." in title.split("@")[-1]:
                title = "[title redacted]"
            row = {
                "id": f"{name}#{number}",
                "repo": name,
                "kind": kind,
                "number": number,
                "class": klass,
                "severity": severity_of(label_names, klass),
                "title": title[:180],
                "labels": label_names,
                "url": item.get("html_url") or f"https://github.com/{name}/issues/{number}",
                "api_url": item.get("url") or "",
                "sha256": digest,
                "bytes": len(body.encode("utf-8")),
            }
            buckets[klass].append(row)
            (CACHE / f"{digest}.md").write_text(body + "\n", encoding="utf-8")

    accepted = []
    per_kind = spec.get("accepted_per_kind") or {}
    if per_kind:
        for kind, cap in per_kind.items():
            rows = [r for r in buckets["accepted"] if r["kind"] == kind]
            accepted.extend(rows[: int(cap)])
    else:
        accepted = buckets["accepted"][: int(spec.get("accepted_cap", 220))]
    caps = spec["per_class_cap"]
    negatives = []
    for klass, cap in caps.items():
        negatives.extend(buckets[klass][: int(cap)])
    rows = accepted + negatives
    # Stable order.
    rows.sort(key=lambda r: (r["class"], r["repo"], r["number"]))
    counts = Counter(r["class"] for r in rows)
    raw_counts = {k: len(v) for k, v in buckets.items()}
    doc = {
        "version": 1,
        "note": (
            "Public contest issues, selected by the label rules in eval/run.py. "
            "Bodies are not in this repository. sha256 is of the issue body with "
            "newlines normalised to \\n. Licences of the underlying reports stay "
            "with their authors and the platforms; this file is a list of URLs."
        ),
        "sources": SOURCES.name,
        "raw_class_counts_before_cap": raw_counts,
        "counts": dict(counts),
        "items": rows,
    }
    MANIFEST.write_text(yaml.safe_dump(doc, sort_keys=False, allow_unicode=True), encoding="utf-8")
    print("wrote", MANIFEST)
    print("counts", dict(counts), "raw", raw_counts)
    return 0


def load_manifest() -> dict:
    return yaml.safe_load(MANIFEST.read_text(encoding="utf-8"))


def score(argv: list[str]) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--vg", default=str(ROOT / "target" / "release" / "vg"))
    args = parser.parse_args(argv)
    vg = args.vg
    if not Path(vg).exists():
        print("eval: vg binary not found at " + vg, file=sys.stderr)
        return 1
    doc = load_manifest()
    RESULTS.mkdir(parents=True, exist_ok=True)
    per_item = []
    missing = 0
    for item in doc["items"]:
        path = CACHE / f"{item['sha256']}.md"
        if not path.exists():
            missing += 1
            continue
        proc = subprocess.run(
            [vg, "check", str(path), "--json"],
            capture_output=True,
            text=True,
            check=False,
        )
        if proc.returncode != 0:
            print(item["id"], proc.stderr[:300], file=sys.stderr)
            return 1
        report = json.loads(proc.stdout)
        rules = []
        hard = []
        for f in report.get("findings", []):
            rid = f["rule"]
            # DUP: and KI: are corpus hits, not catalog rules. Recorded apart.
            rules.append(rid)
            if f["verdict"] == "hard":
                hard.append(rid)
        per_item.append(
            {
                "id": item["id"],
                "class": item["class"],
                "severity": item["severity"],
                "sha256": item["sha256"],
                "outcome": report["outcome"],
                "score": report["viability_score"],
                "rules": rules,
                "hard": hard,
            }
        )
    if missing:
        print(f"eval: {missing} bodies missing from {CACHE}. Run fetch.", file=sys.stderr)
        return 1
    (RESULTS / "per-item.jsonl").write_text(
        "".join(json.dumps(row, sort_keys=True) + "\n" for row in per_item),
        encoding="utf-8",
    )
    summary = summarise(per_item)
    (RESULTS / "summary.json").write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    (RESULTS / "per-rule.csv").write_text(summary["csv"], encoding="utf-8")
    print(json.dumps({k: summary[k] for k in summary if k != "csv" and k != "per_rule"}, indent=2))
    print("per-rule rows", len(summary["per_rule"]))
    return 0


def summarise(items: list[dict]) -> dict:
    by_class: dict[str, list[dict]] = defaultdict(list)
    for item in items:
        by_class[item["class"]].append(item)
    paid = by_class["accepted"]
    # The negative class used for precision is invalid + wontfix. QA and
    # Low/Info are reported beside it, because they are not the same judgement.
    invalid = by_class["invalid"] + by_class["wontfix"]
    rule_ids = sorted({r for item in items for r in item["rules"] if not r.startswith(("DUP:", "KI:"))})
    # Also list catalog rules that never fired, so the matrix is not only the hits.
    per_rule = []
    for rid in rule_ids:
        def fires(group: list[dict], rule: str = rid) -> int:
            return sum(1 for item in group if rule in item["rules"])

        def hards(group: list[dict], rule: str = rid) -> int:
            return sum(1 for item in group if rule in item["hard"])

        paid_n = len(paid)
        inv_n = len(invalid)
        pf = fires(paid)
        ph = hards(paid)
        inf = fires(invalid)
        row = {
            "rule": rid,
            "paid_n": paid_n,
            "paid_fires": pf,
            "paid_hard": ph,
            "paid_fire_rate": rate(pf, paid_n),
            "paid_hard_rate": rate(ph, paid_n),
            "invalid_wontfix_n": inv_n,
            "invalid_wontfix_fires": inf,
            "invalid_wontfix_fire_rate": rate(inf, inv_n),
            "precision_vs_invalid_wontfix": rate(inf, inf + pf),
            "qa_fires": fires(by_class["qa"]),
            "qa_n": len(by_class["qa"]),
            "informative_fires": fires(by_class["informative"]),
            "informative_n": len(by_class["informative"]),
        }
        per_rule.append(row)

    def any_hard(group: list[dict]) -> int:
        return sum(1 for item in group if item["hard"])

    def any_fire(group: list[dict]) -> int:
        return sum(1 for item in group if item["rules"])

    def outcome_counts(group: list[dict]) -> dict:
        return dict(Counter(item["outcome"] for item in group))

    headline = {
        "paid_n": len(paid),
        "paid_any_hard": any_hard(paid),
        "paid_hard_rate": rate(any_hard(paid), len(paid)),
        "paid_any_fire": any_fire(paid),
        "invalid_wontfix_n": len(invalid),
        "invalid_wontfix_any_hard": any_hard(invalid),
        "invalid_wontfix_hard_rate": rate(any_hard(invalid), len(invalid)),
        "qa_n": len(by_class["qa"]),
        "qa_any_hard": any_hard(by_class["qa"]),
        "informative_n": len(by_class["informative"]),
        "informative_any_hard": any_hard(by_class["informative"]),
        "outcomes_paid": outcome_counts(paid),
        "outcomes_invalid_wontfix": outcome_counts(invalid),
        "class_counts": {k: len(v) for k, v in by_class.items()},
    }
    cols = [
        "rule",
        "paid_n",
        "paid_fires",
        "paid_hard",
        "paid_fire_rate",
        "paid_hard_rate",
        "invalid_wontfix_n",
        "invalid_wontfix_fires",
        "invalid_wontfix_fire_rate",
        "precision_vs_invalid_wontfix",
        "qa_n",
        "qa_fires",
        "informative_n",
        "informative_fires",
    ]
    lines = [",".join(cols)]
    for row in per_rule:
        lines.append(",".join(str(row[c]) for c in cols))
    csv = "\n".join(lines) + "\n"
    headline["per_rule"] = per_rule
    headline["csv"] = csv
    headline["items"] = len(items)
    return headline


def rate(n: int, d: int):
    if d <= 0:
        return None
    return round(n / d, 4)


def check(_argv: list[str]) -> int:
    summary = json.loads((RESULTS / "summary.json").read_text(encoding="utf-8"))
    items = [
        json.loads(line)
        for line in (RESULTS / "per-item.jsonl").read_text(encoding="utf-8").splitlines()
        if line.strip()
    ]
    again = summarise(items)
    problems = []
    for key in (
        "paid_n",
        "paid_any_hard",
        "invalid_wontfix_n",
        "invalid_wontfix_any_hard",
        "qa_n",
        "informative_n",
        "items",
    ):
        if summary.get(key) != again.get(key):
            problems.append(f"{key}: file {summary.get(key)} recomputed {again.get(key)}")
    if problems:
        print("eval check failed")
        for p in problems:
            print(" ", p)
        return 1
    print(f"eval check ok ({summary['items']} items, paid hard {summary['paid_any_hard']}/{summary['paid_n']})")
    return 0


def main(argv: list[str] | None = None) -> int:
    argv = list(sys.argv[1:] if argv is None else argv)
    if not argv or argv[0] in ("-h", "--help"):
        print(__doc__)
        return 0
    cmd, rest = argv[0], argv[1:]
    if cmd == "fetch":
        return fetch(rest)
    if cmd == "score":
        return score(rest)
    if cmd == "all":
        code = fetch(rest)
        if code != 0:
            return code
        return score([])
    if cmd == "check":
        return check(rest)
    print("unknown command " + cmd, file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main())
