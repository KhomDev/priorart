#!/usr/bin/env python3
"""Clone the pinned Dup-Bench audited codebases for a split.

Reads `eval/dupbench/codebases.json` and checks out each contest's sponsor
snapshot at its pinned commit under `--dest/<slug>`. Only `status ==
"repo+commit"` entries are cloned; a contest whose sponsor repository is not
publicly reachable stays absent and the measurement must abstain on it (never
guess).

Network: this script clones from GitHub. It is the only network step in the
Dup-Bench measurement; `priorart anchors index` and the PA-B3 driver are
offline. Downloaded code is untrusted: it is only parsed, never executed.

Usage:
  eval/dupbench/clone_codebases.py --split train [--dest DIR] [--only SLUG] [--dry-run]
"""
from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent


def git(args: list[str], cwd: Path | None = None) -> subprocess.CompletedProcess:
    return subprocess.run(
        ["git", "-c", "core.longpaths=true", *args],
        cwd=cwd,
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
    )


def clone_one(slug: str, url: str, commit: str, dest: Path, dry_run: bool) -> str:
    target = dest / slug
    if target.is_dir():
        head = git(["-C", str(target), "rev-parse", "HEAD"]).stdout.strip()
        if head.lower() == commit.lower():
            return "present"
        if head:
            return f"present-at-wrong-head:{head[:12]}"
    if dry_run:
        return "would-clone"
    target.parent.mkdir(parents=True, exist_ok=True)
    proc = git(["clone", "--filter=blob:none", "--no-checkout", url, str(target)])
    if proc.returncode != 0:
        return f"clone-failed:{(proc.stderr or proc.stdout).strip().splitlines()[-1][:160]}"
    proc = git(["-C", str(target), "fetch", "--depth", "1", "origin", commit])
    if proc.returncode != 0:
        return f"fetch-failed:{(proc.stderr or proc.stdout).strip().splitlines()[-1][:160]}"
    proc = git(["-C", str(target), "checkout", "-f", "FETCH_HEAD"])
    if proc.returncode != 0:
        return f"checkout-failed:{(proc.stderr or proc.stdout).strip().splitlines()[-1][:160]}"
    head = git(["-C", str(target), "rev-parse", "HEAD"]).stdout.strip()
    if head.lower() != commit.lower():
        return f"head-mismatch:{head[:12]}"
    return "cloned"


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--split", default="train", choices=["train", "validation", "FINAL"])
    parser.add_argument("--dest", default=str(ROOT / "target" / "dupbench-sources"))
    parser.add_argument("--only", action="append", default=[])
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args(argv[1:])

    codebases = json.loads((HERE / "codebases.json").read_text(encoding="utf-8"))["codebases"]
    dest = Path(args.dest).resolve()
    selected = [
        (slug, entry)
        for slug, entry in sorted(codebases.items())
        if entry.get("split") == args.split and (not args.only or slug in args.only)
    ]
    counts: dict[str, int] = {}
    missing: list[str] = []
    for slug, entry in selected:
        status = entry.get("status")
        if status != "repo+commit":
            counts["pinned-missing"] = counts.get("pinned-missing", 0) + 1
            missing.append(slug)
            print(f"{slug}\t{status or 'missing'}\tnot cloned (must abstain)", flush=True)
            continue
        outcome = clone_one(slug, entry["repo"], entry["commit"], dest, args.dry_run)
        counts[outcome.split(":")[0]] = counts.get(outcome.split(":")[0], 0) + 1
        print(f"{slug}\t{outcome}", flush=True)
    print(json.dumps({"split": args.split, "dest": str(dest), "counts": counts, "not_cloned": missing}, indent=1))
    failures = {k: v for k, v in counts.items() if k not in ("cloned", "present", "would-clone")}
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
