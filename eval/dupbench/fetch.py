#!/usr/bin/env python3
"""Fetches and verifies every Dup-Bench v1 pin.

Default: materialise missing report/data files and label pages in the
gitignored cache `.corpus/dupbench/`, then verify. `--check`: verify only; no
network.

Pinned files: per contest the findings-repo commit, report.md sha256, the data/
listing hash (names and count; bodies are fetched from the commit), and every
label page sha256. Label pages hold GitHub issue metadata fetched with
`gh api` (>= 7 s between calls); only issue numbers and labels reach
labels.json, never a finding body.

A label fetch is slow (one call per 100 issues per contest). Re-running is
resumable: existing pages are kept.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import shutil
import subprocess
import sys
import urllib.request
import time
import pathlib

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
HERE = ROOT / "eval" / "dupbench"
CACHE = ROOT / ".corpus" / "dupbench"
PINS = CACHE / "repos"
LABELS = CACHE / "labels"
CHECKOUTS = CACHE / "checkouts"


def sha(p: pathlib.Path) -> str:
    return hashlib.sha256(p.read_bytes()).hexdigest()


def listing(d: pathlib.Path) -> str:
    names = "\n".join(sorted(p.name for p in d.glob("*.json"))) if d.is_dir() else ""
    return hashlib.sha256(names.encode()).hexdigest()


def run(cmd: list[str]) -> subprocess.CompletedProcess:
    return subprocess.run(cmd, capture_output=True, text=True, encoding="utf-8", errors="replace")


def ensure_checkout(slug: str, repo: str, commit: str) -> pathlib.Path:
    dest = CHECKOUTS / f"c4-{slug}"
    if dest.is_dir():
        head = run(["git", "-C", str(dest), "rev-parse", "HEAD"])
        if head.returncode == 0 and head.stdout.strip() == commit:
            return dest
        shutil.rmtree(dest)
    dest.mkdir(parents=True)
    run(["git", "-C", str(dest), "init", "-q"])
    run(["git", "-C", str(dest), "remote", "add", "origin",
         f"https://github.com/{repo}.git"])
    got = run(["git", "-C", str(dest), "fetch", "-q", "--depth", "1", "origin", commit])
    if got.returncode != 0:
        raise SystemExit(f"{repo}@{commit}: {got.stderr.strip()}")
    run(["git", "-C", str(dest), "checkout", "-q", "FETCH_HEAD"])
    return dest


def fetch(slug: str, repo: str, commit: str) -> None:
    pin = PINS / f"c4-{slug}"
    if not (pin / "report.md").is_file():
        src = ensure_checkout(slug, repo, commit)
        pin.mkdir(parents=True, exist_ok=True)
        shutil.copy(src / "report.md", pin / "report.md")
        if (src / "data").is_dir():
            if (pin / "data").is_dir():
                shutil.rmtree(pin / "data")
            shutil.copytree(src / "data", pin / "data")
        print(f"pins: {slug}")
    d = LABELS / slug
    d.mkdir(parents=True, exist_ok=True)
    page = 1
    while True:
        out = d / f"page-{page}.json"
        if out.is_file():
            if len(json.loads(out.read_text(encoding="utf-8"))) < 100:
                return
            page += 1
            continue
        time.sleep(7)
        r = run(["gh", "api", f"repos/{repo}/issues?state=all&per_page=100&page={page}"])
        if r.returncode != 0:
            raise SystemExit(f"{slug} label page {page}: {r.stderr.strip()}")
        out.write_text(r.stdout, encoding="utf-8")
        print(f"labels: {slug} page {page} ({len(json.loads(r.stdout))})")
        if len(json.loads(r.stdout)) < 100:
            return
        page += 1


def verify_codebases() -> list[str]:
    errs = []
    p = HERE / "codebases.json"
    if not p.is_file():
        return ["missing codebases.json"]
    doc = json.loads(p.read_text(encoding="utf-8"))
    for slug, e in doc.get("codebases", {}).items():
        if e.get("status") not in ("repo+commit", "repo-only", "missing"):
            errs.append(f"codebase {slug}: bad status")
        if e.get("status") == "repo+commit" and not re.fullmatch(r"[0-9a-f]{40}", e.get("commit") or ""):
            errs.append(f"codebase {slug}: bad commit")
        if e.get("status") == "repo-only" and e.get("commit"):
            errs.append(f"codebase {slug}: repo-only carries a commit")
        if e.get("status") == "missing" and not e.get("reason"):
            errs.append(f"codebase {slug}: missing without a reason")
        r = e.get("readme") or {}
        if r.get("sha256"):
            rp = CACHE / r.get("file", "")
            if not rp.is_file() or sha(rp) != r["sha256"]:
                errs.append(f"codebase {slug}: README pin")
    if len(doc.get("codebases", {})) != 111:
        errs.append("codebases.json does not hold 111 entries")
    return errs


def verify(lock: dict) -> list[str]:
    errs = []
    for slug, c in lock["contests"].items():
        pin = PINS / f"c4-{slug}"
        rp = pin / "report.md"
        if not rp.is_file() or sha(rp) != c["files"]["report.md"]:
            errs.append(f"{slug}: report pin")
        if listing(pin / "data") != c["files"]["data_listing_sha256"]:
            errs.append(f"{slug}: data listing pin")
        for name, want in c["label_pages"].items():
            p = LABELS / slug / name
            if not p.is_file() or sha(p) != want:
                errs.append(f"{slug}: {name} pin")
    return errs


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true")
    args = ap.parse_args(argv[1:])
    lock = json.loads((HERE / "cache.lock.json").read_text(encoding="utf-8"))
    if not args.check:
        for slug, c in lock["contests"].items():
            fetch(slug, c["repo"], c["commit"])
        cb = json.loads((HERE / "codebases.json").read_text(encoding="utf-8"))
        for slug, e in cb.get("codebases", {}).items():
            r = e.get("readme") or {}
            rp = CACHE / r.get("file", "")
            if r.get("sha256") and (not rp.is_file() or sha(rp) != r["sha256"]):
                rp.parent.mkdir(parents=True, exist_ok=True)
                req = urllib.request.Request(r["raw_url"], headers={"User-Agent": UA})
                with urllib.request.urlopen(req, timeout=60) as resp:
                    data = resp.read()
                if hashlib.sha256(data).hexdigest() != r["sha256"]:
                    raise SystemExit(f"{slug}: README sha mismatch")
                rp.write_bytes(data)
                print(f"readme: {slug}")
    errs = verify(lock) + verify_codebases()
    if errs:
        print(f"{len(errs)} pin error(s):", file=sys.stderr)
        for e in errs[:30]:
            print("  " + e, file=sys.stderr)
        return 1
    n_lab = sum(1 for c in lock["contests"].values() if c["label_pages"])
    print(f"all {len(lock['contests'])} contest pin sets verified; {n_lab} with label pages"
          + (" (offline)" if args.check else ""))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
