#!/usr/bin/env python3
"""Build the PA-B4 ranker dataset from the Dup-Bench train and validation splits.

For every contest in the requested split that has an audited codebase checkout
at its pinned commit, this script:

1. builds (or reuses, only when the recorded commit is the pin) the
   `priorart anchors index` for the checkout;
2. prepares the prior corpus from the contest's paid-unique primaries
   (`labels.json` `primaries`, whose text is read from the pinned issue pages);
3. runs `priorart candidates` for every finding that can be a query:
   - each judged duplicate against the contest's primaries (label 1 for its
     primary, 0 for every other candidate);
   - each primary against the other primaries (label 0 for every candidate);
4. records one row per candidate pair with the raw feature vector, and one
   row per attempted query (with its abstention reason, if any).

A contest without a public repository pin, without a checkout, or with a
checkout at the wrong commit is skipped and recorded; it is never guessed. An
abstained query is recorded with its reason and excluded from fitting by the
trainer; it is never treated as a negative.

The FINAL split is never opened: `--split` accepts only `train`, `validation`
or `all` (which is the two of them).

Output is deterministic JSONL under a gitignored `target/` path; the issue
texts it reads are third-party and stay in the gitignored cache and output.
No network. No LLM.

Usage:
  python -I eval/dupbench/build_ranker_data.py --split all
  python -I eval/dupbench/build_ranker_data.py --split validation --only 2024-02-tapioca
"""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent

# The nine raw features, by the exact JSON path `priorart candidates --json`
# uses. The trainer and the Rust inference both read this list from the
# committed weights file.
FEATURE_KEYS = [
    "anchor_function_overlap.shared",
    "anchor_function_overlap.ratio",
    "anchor_state_overlap.shared",
    "anchor_state_overlap.ratio",
    "call_graph_distance",
    "mechanism_match.shared_root_causes",
    "mechanism_match.same_contract",
    "impact_match.shared_impacts",
    "identifier_score.specific_symbols",
    "identifier_score.shared_symbols",
    "identifier_score.rarity",
    "identifier_score.ranked",
]

SPLITS = ("train", "validation")


def launch_root() -> Path:
    """The khomdev-launch directory, for the shared caches and the main checkout."""
    return ROOT.parent.parent if ROOT.parent.name == "worktrees" else ROOT.parent


def default_labels_dir() -> Path:
    return launch_root() / "worktrees" / "research-pa" / ".corpus" / "dupbench" / "labels"


def default_source_dirs() -> list[Path]:
    launch = launch_root()
    return [
        ROOT / "target" / "dupbench-sources",
        launch / "priorart" / "target" / "dupbench-sources",
    ]


def default_reuse_index_dirs() -> list[Path]:
    launch = launch_root()
    return [
        launch / "priorart" / "target" / "candidates-measure" / "index",
        launch / "priorart" / "target" / "candidates-measure-dupbench-src" / "index",
        launch / "worktrees" / "pa-b3" / "target" / "candidates-measure" / "index",
    ]


def run(cmd: list[str], what: str) -> subprocess.CompletedProcess:
    proc = subprocess.run(
        cmd, capture_output=True, text=True, encoding="utf-8", errors="replace"
    )
    if proc.returncode != 0:
        raise SystemExit(
            f"{what} failed ({proc.returncode}): {' '.join(cmd)}\nstderr: {proc.stderr[:2000]}"
        )
    return proc


def git_head(checkout: Path) -> str:
    proc = subprocess.run(
        ["git", "-c", "core.longpaths=true", "-C", str(checkout), "rev-parse", "HEAD"],
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
    )
    return proc.stdout.strip() if proc.returncode == 0 else ""


def git_deleted_files(checkout: Path) -> str:
    """Tracked files missing from the worktree.

    A checkout interrupted midway (for example on a Windows-invalid path)
    leaves HEAD at the pin with a partial worktree, which would index as
    silently empty. Modified files (` M`, e.g. line-ending churn on binary
    archive files the indexer never reads) are not a completeness problem;
    deleted tracked files are.
    """
    proc = subprocess.run(
        ["git", "-c", "core.longpaths=true", "-C", str(checkout), "ls-files", "-d"],
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
    )
    return proc.stdout if proc.returncode == 0 else "ls-files-failed"


def git_tracked_count(checkout: Path) -> int:
    """How many files the index tracks. A checkout interrupted before the
    index was populated has HEAD at the pin and zero tracked files, which the
    deleted-files check cannot see."""
    proc = subprocess.run(
        ["git", "-c", "core.longpaths=true", "-C", str(checkout), "ls-files"],
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
    )
    if proc.returncode != 0:
        return -1
    return len(proc.stdout.splitlines())


def find_checkout(slug: str, source_dirs: list[Path]) -> Path | None:
    for root in source_dirs:
        candidate = root / slug
        if candidate.is_dir():
            return candidate
    return None


def repo_id_of(repo: str) -> str:
    return repo.rstrip("/").split("github.com/")[-1]


def load_json(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def load_issue_pages(labels_dir: Path, slug: str) -> dict[int, dict]:
    issues: dict[int, dict] = {}
    for page in sorted((labels_dir / slug).glob("page-*.json")):
        for item in load_json(page):
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


def index_for_contest(
    slug: str,
    entry: dict,
    checkout: Path,
    out_dir: Path,
    reuse_dirs: list[Path],
    bin_path: Path,
) -> tuple[Path | None, str]:
    """Return (index path, status). Reuse only a cache whose commit is the pin."""
    pin = entry["commit"].lower()
    repo_id = repo_id_of(entry["repo"])
    own = out_dir / "index" / f"dupbench-{slug}.json"
    candidates = [own, *[d / f"dupbench-{slug}.json" for d in reuse_dirs]]
    for path in candidates:
        if not path.is_file():
            continue
        try:
            cached = load_json(path)
        except (OSError, json.JSONDecodeError):
            continue
        if str(cached.get("commit", "")).lower() != pin:
            continue
        if str(cached.get("repo", "")) != repo_id:
            continue
        return path, "cached"
    own.parent.mkdir(parents=True, exist_ok=True)
    with own.open("w", encoding="utf-8", newline="\n") as handle:
        proc = subprocess.run(
            [
                str(bin_path),
                "anchors",
                "index",
                str(checkout),
                "--repo-id",
                repo_id,
                "--commit",
                entry["commit"],
                "--json",
            ],
            stdout=handle,
            stderr=subprocess.PIPE,
            text=True,
            encoding="utf-8",
            errors="replace",
        )
    if proc.returncode != 0:
        return None, f"index-failed:{(proc.stderr or '').strip().splitlines()[-1][:160] if proc.stderr.strip() else 'no stderr'}"
    return own, "built"


def write_corpus(slug: str, primary_texts: dict[str, str], out_dir: Path) -> Path:
    priors = out_dir / "priors" / slug
    if priors.is_dir():
        for stale in priors.glob("*.md"):
            stale.unlink()
    priors.mkdir(parents=True, exist_ok=True)
    for number, text in primary_texts.items():
        (priors / f"{number}.md").write_text(text, encoding="utf-8", newline="\n")
    return priors


def clear_queries(slug: str, out_dir: Path) -> Path:
    queries = out_dir / "queries" / slug
    if queries.is_dir():
        for stale in queries.glob("*"):
            stale.unlink()
    queries.mkdir(parents=True, exist_ok=True)
    return queries


def write_query(queries: Path, name: str, text: str) -> Path:
    path = queries / f"{name}.md"
    path.write_text(text, encoding="utf-8", newline="\n")
    return path


def features_of(candidate: dict) -> dict:
    f = candidate["features"]
    return {
        "anchor_function_overlap.shared": len(f["anchor_function_overlap"]["shared"]),
        "anchor_function_overlap.ratio": f["anchor_function_overlap"]["ratio"],
        "anchor_state_overlap.shared": len(f["anchor_state_overlap"]["shared"]),
        "anchor_state_overlap.ratio": f["anchor_state_overlap"]["ratio"],
        "call_graph_distance": f["call_graph_distance"],
        "mechanism_match.shared_root_causes": len(
            f["mechanism_match"]["shared_root_causes"]
        ),
        "mechanism_match.same_contract": bool(f["mechanism_match"]["same_contract"]),
        "impact_match.shared_impacts": len(f["impact_match"]["shared_impacts"]),
        "identifier_score.specific_symbols": f["identifier_score"]["specific_symbols"],
        "identifier_score.shared_symbols": f["identifier_score"]["shared_symbols"],
        "identifier_score.rarity": f["identifier_score"]["rarity"],
        "identifier_score.ranked": bool(f["identifier_score"]["ranked"]),
    }


class Dataset:
    def __init__(self) -> None:
        self.rows: list[dict] = []
        self.counts = {
            "contests_requested": 0,
            "contests_used": 0,
            "contests_skipped": {},
            "queries_attempted": {},
            "abstentions": {},
            "pairs": 0,
            "positives": 0,
            "negatives": 0,
            "queries_without_positive_pair": 0,
        }

    def bump(self, group: str, key: str, n: int = 1) -> None:
        bucket = self.counts[group]
        bucket[key] = bucket.get(key, 0) + n

    def add(self, key: str, n: int = 1) -> None:
        self.counts[key] = self.counts.get(key, 0) + n


def process_contest(
    slug: str,
    split: str,
    entry: dict,
    labels: dict,
    issues: dict[int, dict],
    dataset: Dataset,
    out_dir: Path,
    bin_path: Path,
    source_dirs: list[Path],
    reuse_dirs: list[Path],
    top_k: int,
) -> None:
    checkout = find_checkout(slug, source_dirs)
    if checkout is None:
        dataset.bump("contests_skipped", "checkout-missing")
        dataset.rows.append(
            {"kind": "skip", "split": split, "contest": slug, "reason": "checkout-missing"}
        )
        return
    head = git_head(checkout)
    if head.lower() != entry["commit"].lower():
        dataset.bump("contests_skipped", "head-mismatch")
        dataset.rows.append(
            {
                "kind": "skip",
                "split": split,
                "contest": slug,
                "reason": "head-mismatch",
                "head": head[:12],
                "pin": entry["commit"][:12],
            }
        )
        return
    # A HEAD at the pin is not enough: a checkout interrupted midway (for
    # example on a Windows-invalid path) leaves HEAD at the pin with a partial
    # worktree, which would index as silently empty. Require no deleted
    # tracked files.
    deleted = git_deleted_files(checkout)
    tracked = git_tracked_count(checkout)
    if deleted.strip() or tracked <= 0:
        dataset.bump("contests_skipped", "checkout-incomplete")
        dataset.rows.append(
            {
                "kind": "skip",
                "split": split,
                "contest": slug,
                "reason": "checkout-incomplete",
                "deleted_files": len(deleted.splitlines()),
                "tracked_files": tracked,
            }
        )
        return

    primaries = sorted((str(n) for n in labels.get("primaries", [])), key=int)
    duplicates = labels.get("duplicates", {})
    primary_texts: dict[str, str] = {}
    for number in primaries:
        text = issue_text(issues.get(int(number)))
        if text:
            primary_texts[number] = text
    if not primary_texts:
        dataset.bump("contests_skipped", "no-primary-text")
        dataset.rows.append(
            {"kind": "skip", "split": split, "contest": slug, "reason": "no-primary-text"}
        )
        return

    index_path, index_status = index_for_contest(
        slug, entry, checkout, out_dir, reuse_dirs, bin_path
    )
    if index_path is None:
        dataset.bump("contests_skipped", "index-failed")
        dataset.rows.append(
            {
                "kind": "skip",
                "split": split,
                "contest": slug,
                "reason": index_status,
            }
        )
        return

    priors = write_corpus(slug, primary_texts, out_dir)
    prepared = out_dir / "prepared" / f"{slug}.json"
    prepared.parent.mkdir(parents=True, exist_ok=True)
    run(
        [
            str(bin_path),
            "candidates",
            "--corpus",
            str(priors),
            "--index",
            str(index_path),
            "--prepare",
            str(prepared),
            "--json",
        ],
        f"{slug}: prepare",
    )

    queries: list[tuple[str, str, str | None, str]] = []
    for primary in sorted(duplicates, key=int):
        if primary not in primary_texts:
            for dup in sorted(duplicates[primary], key=int):
                dataset.rows.append(
                    {
                        "kind": "skip",
                        "split": split,
                        "contest": slug,
                        "query_issue": dup,
                        "query_role": "duplicate",
                        "expected_primary": primary,
                        "reason": "primary-not-in-corpus",
                    }
                )
            continue
        for dup in sorted(duplicates[primary], key=int):
            text = issue_text(issues.get(int(dup)))
            if text is None:
                dataset.rows.append(
                    {
                        "kind": "skip",
                        "split": split,
                        "contest": slug,
                        "query_issue": dup,
                        "query_role": "duplicate",
                        "expected_primary": primary,
                        "reason": "duplicate-text-missing",
                    }
                )
                continue
            queries.append(("duplicate", primary, dup, text))
    for primary in primaries:
        text = primary_texts.get(primary)
        if text is None:
            dataset.rows.append(
                {
                    "kind": "skip",
                    "split": split,
                    "contest": slug,
                    "query_issue": primary,
                    "query_role": "primary",
                    "expected_primary": None,
                    "reason": "primary-text-missing",
                }
            )
            continue
        queries.append(("primary", primary, None, text))

    query_dir = clear_queries(slug, out_dir)
    for role, primary, dup, text in queries:
        issue = dup if role == "duplicate" else primary
        query_id = f"{slug}#{'dup' if role == 'duplicate' else 'primary'}-{issue}"
        query_path = write_query(query_dir, f"{'dup' if role == 'duplicate' else 'primary'}-{issue}", text)
        proc = run(
            [
                str(bin_path),
                "candidates",
                str(query_path),
                "--corpus",
                str(prepared),
                "--index",
                str(index_path),
                "--exclude",
                issue,
                "--top-k",
                str(top_k),
                "--json",
            ],
            f"{slug}: query {query_id}",
        )
        report = json.loads(proc.stdout)
        dataset.bump("queries_attempted", role)
        abstention = report.get("abstention") or None
        candidates = report.get("candidates", [])
        if abstention is not None:
            dataset.bump("abstentions", f"{role}:{abstention['reason']}")
        dataset.rows.append(
            {
                "kind": "query",
                "split": split,
                "contest": slug,
                "query": query_id,
                "query_role": role,
                "query_issue": issue,
                "expected_primary": primary if role == "duplicate" else None,
                "candidates": len(candidates),
                "abstention": abstention["reason"] if abstention else None,
            }
        )
        if abstention is not None:
            continue
        positive_present = False
        for candidate in candidates:
            candidate_id = candidate["prior_id"]
            label = 1 if role == "duplicate" and candidate_id == primary else 0
            positive_present = positive_present or label == 1
            dataset.rows.append(
                {
                    "kind": "pair",
                    "split": split,
                    "contest": slug,
                    "query": query_id,
                    "query_role": role,
                    "query_issue": issue,
                    "expected_primary": primary if role == "duplicate" else None,
                    "candidate": candidate_id,
                    "label": label,
                    "features": features_of(candidate),
                }
            )
            dataset.add("pairs")
            dataset.add("positives" if label else "negatives")
        if role == "duplicate" and not positive_present:
            dataset.add("queries_without_positive_pair")

    dataset.counts["contests_used"] += 1
    print(
        f"{split} {slug}: index={index_status} primaries={len(primary_texts)} "
        f"queries={len(queries)}",
        file=sys.stderr,
        flush=True,
    )


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--split", default="all", choices=["train", "validation", "all"])
    parser.add_argument("--only", action="append", default=[])
    parser.add_argument("--manifest", default=str(HERE / "manifest.json"))
    parser.add_argument("--labels", default=str(HERE / "labels.json"))
    parser.add_argument("--codebases", default=str(HERE / "codebases.json"))
    parser.add_argument("--labels-dir", default=str(default_labels_dir()))
    parser.add_argument("--bin", default=str(ROOT / "target" / "release" / "priorart.exe"))
    parser.add_argument("--source-dir", action="append", default=[])
    parser.add_argument("--reuse-index-dir", action="append", default=[])
    parser.add_argument("--out", default=str(ROOT / "target" / "ranker-data"))
    parser.add_argument("--top-k", type=int, default=1000)
    args = parser.parse_args(argv[1:])

    manifest = load_json(Path(args.manifest))
    labels = load_json(Path(args.labels))["contests"]
    codebases = load_json(Path(args.codebases))["codebases"]
    labels_dir = Path(args.labels_dir)
    bin_path = Path(args.bin)
    out_dir = Path(args.out)
    source_dirs = [Path(p) for p in (args.source_dir or default_source_dirs())]
    reuse_dirs = [Path(p) for p in (args.reuse_index_dir or default_reuse_index_dirs())]

    if not bin_path.is_file():
        raise SystemExit(f"binary not found: {bin_path}")
    if args.split == "all":
        splits = set(SPLITS)
    else:
        splits = {args.split}
    contests = [
        c
        for c in manifest["contests"]
        if c.get("split") in splits and (not args.only or c["slug"] in args.only)
    ]
    contests.sort(key=lambda c: c["slug"])
    dataset = Dataset()
    dataset.counts["contests_requested"] = len(contests)
    out_dir.mkdir(parents=True, exist_ok=True)

    for contest in contests:
        slug = contest["slug"]
        split = contest["split"]
        entry = codebases.get(slug)
        if not entry or entry.get("status") != "repo+commit" or not entry.get("commit"):
            dataset.bump("contests_skipped", "no-public-repo")
            dataset.rows.append(
                {"kind": "skip", "split": split, "contest": slug, "reason": "no-public-repo"}
            )
            continue
        if slug not in labels:
            dataset.bump("contests_skipped", "labels-missing")
            dataset.rows.append(
                {"kind": "skip", "split": split, "contest": slug, "reason": "labels-missing"}
            )
            continue
        issues = load_issue_pages(labels_dir, slug)
        if not issues:
            dataset.bump("contests_skipped", "label-pages-missing")
            dataset.rows.append(
                {
                    "kind": "skip",
                    "split": split,
                    "contest": slug,
                    "reason": "label-pages-missing",
                }
            )
            continue
        rows_before = list(dataset.rows)
        counts_before = copy.deepcopy(dataset.counts)
        try:
            process_contest(
                slug,
                split,
                entry,
                labels[slug],
                issues,
                dataset,
                out_dir,
                bin_path,
                source_dirs,
                reuse_dirs,
                args.top_k,
            )
        except (SystemExit, Exception) as exc:  # noqa: BLE001 - keep the corpus run alive
            # One contest's failure must not throw away the other 90. Roll the
            # partial contest back so the JSONL stays consistent, and record
            # the reason instead of guessing.
            dataset.rows = rows_before
            dataset.counts = counts_before
            reason = str(exc).strip().splitlines()[0][:300] if str(exc).strip() else repr(exc)
            dataset.bump("contests_skipped", "error")
            dataset.rows.append(
                {
                    "kind": "skip",
                    "split": split,
                    "contest": slug,
                    "reason": "error",
                    "detail": reason,
                }
            )
            print(f"{split} {slug}: ERROR {reason}", file=sys.stderr, flush=True)

    dataset_path = out_dir / "dataset.jsonl"
    with dataset_path.open("w", encoding="utf-8", newline="\n") as handle:
        for row in dataset.rows:
            handle.write(json.dumps(row, ensure_ascii=False, sort_keys=False) + "\n")
    binary_sha256 = hashlib.sha256(bin_path.read_bytes()).hexdigest()
    summary = {
        "schema": "priorart.ranker-dataset/v1",
        "feature_keys": FEATURE_KEYS,
        "splits": sorted(splits),
        "binary_sha256": binary_sha256,
        "counts": dataset.counts,
    }
    (out_dir / "summary.json").write_text(
        json.dumps(summary, indent=1) + "\n", encoding="utf-8", newline="\n"
    )
    print(json.dumps(dataset.counts, indent=1))
    print(f"dataset: {dataset_path} ({len(dataset.rows)} rows)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
