#!/usr/bin/env python3
"""Fetch the pinned public inputs for the priorart benchmark. Network is used
here and only here; run.py is offline.

    python eval/benchmark/fetch.py            # fetch everything, write cache.lock.json
    python eval/benchmark/fetch.py --check    # offline: cache matches cache.lock.json
    python eval/benchmark/fetch.py --manifest # offline: write manifest.yaml
    python eval/benchmark/fetch.py --list     # what is pinned

A second corpus lives in its own directory and is selected with --corpus-dir;
the default is the directory this script sits in, so the Tapioca corpus is
unchanged:

    python eval/benchmark/fetch.py --corpus-dir eval/benchmark/pooltogether

What is downloaded is public contest/audit material; it is written under
eval/cache/benchmark/ (this corpus) or eval/cache/benchmark/<corpus>/ (another
corpus), which the repository gitignores. The committed files are the pointers
(sources.yaml), the labels (labels.yaml), the integrity lock (cache.lock.json)
and the generated manifest.yaml. No finding body is committed.

A token comes from GH_TOKEN, GITHUB_TOKEN, or "gh auth token". Unauthenticated
access works but is rate-limited to 60 requests an hour, which is not enough.
"""

from __future__ import annotations

import argparse
import base64
import hashlib
import http.client
import json
import os
import re
import subprocess
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

try:
    import yaml
except ImportError:
    print("eval/benchmark: PyYAML is required", file=sys.stderr)
    sys.exit(2)

BENCH = Path(__file__).resolve().parent
ROOT = BENCH.parent.parent            # priorart/
DEFAULT_CACHE = ROOT / "eval" / "cache" / "benchmark"
CACHE = DEFAULT_CACHE
SOURCES = BENCH / "sources.yaml"
LABELS = BENCH / "labels.yaml"
LOCK = BENCH / "cache.lock.json"
MANIFEST = BENCH / "manifest.yaml"
API = "https://api.github.com"

# The original corpus predates --corpus-dir; its generator parameters are the
# defaults, so its committed files and manifest bytes do not change. A second
# corpus declares the same keys in a `corpus:` block in its sources.yaml.
LEGACY_CORPUS = {
    "prior_item_prefix": "c4-2023-07",
    "known_item_prefix": "ki-2024-02",
    "known_issue_section": "Q: Please list any known issues/acceptable risks",
    # The regex whose group 1 is the sponsor's known-issues text. The default is
    # Sherlock's README Q&A; a Code4rena corpus names its README heading.
    "known_issue_pattern": r"(?ims)^#{1,4}\s*Q:\s*Please list any known issues.*?\n(.*?)(?=^___|\n#{1,4}\s*Q:|\Z)",
}

# Code4rena judge labels (see BENCHMARK.md, "Third and fourth corpora").
C4_SEVERITY = ("2 (Med Risk)", "3 (High Risk)")
C4_REPORT_ID = re.compile(r"^([HM])-(\d+)$")
C4_DUPLICATE = re.compile(r"^duplicate-(\d+)$")
C4_WITHDRAWN = ("withdrawn by warden",)


def set_corpus(value: str) -> None:
    """Point every path at one corpus directory. The original directory keeps
    the historical cache root so its lock and freeze stay valid."""
    global CACHE, SOURCES, LABELS, LOCK, MANIFEST
    corpus = Path(value).resolve()
    if not (corpus / "sources.yaml").is_file():
        raise SystemExit(f"eval/benchmark: no sources.yaml under {corpus}")
    CACHE = DEFAULT_CACHE if corpus == BENCH else DEFAULT_CACHE / corpus.name
    SOURCES = corpus / "sources.yaml"
    LABELS = corpus / "labels.yaml"
    LOCK = corpus / "cache.lock.json"
    MANIFEST = corpus / "manifest.yaml"


def token() -> str:
    for name in ("GH_TOKEN", "GITHUB_TOKEN"):
        if os.environ.get(name):
            return os.environ[name]
    try:
        return subprocess.check_output(["gh", "auth", "token"], text=True).strip()
    except (OSError, subprocess.CalledProcessError):
        print("eval/benchmark: no token; set GH_TOKEN or run gh auth login", file=sys.stderr)
        raise SystemExit(2)


TOKEN = ""


def get(url: str) -> dict:
    """GET a GitHub API URL, retrying the transient statuses."""
    last = None
    for attempt in range(5):
        req = urllib.request.Request(
            url,
            headers={
                "Authorization": f"Bearer {TOKEN}",
                "Accept": "application/vnd.github+json",
                "User-Agent": "priorart-eval-benchmark",
                "X-GitHub-Api-Version": "2022-11-28",
            },
        )
        try:
            with urllib.request.urlopen(req, timeout=60) as resp:
                return json.loads(resp.read().decode("utf-8"))
        except urllib.error.HTTPError as exc:
            last = exc
            if exc.code in (403, 429, 500, 502, 503, 504):
                time.sleep(2 ** attempt)
                continue
            raise
        except (urllib.error.URLError, http.client.HTTPException, ConnectionError, OSError) as exc:
            last = exc
            time.sleep(1 + 2 ** attempt)
    raise RuntimeError(f"GET {url} failed: {last}")


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def spec_file() -> dict:
    return yaml.safe_load(SOURCES.read_text(encoding="utf-8"))


def specs() -> list[dict]:
    return spec_file()["sources"]


def seed() -> str:
    return spec_file()["seed"]


def wanted_from_tree(source: dict) -> dict[str, str]:
    """Map repository path -> blob sha for the paths this source pins."""
    tree = get(f"{API}/repos/{source['repo']}/git/trees/{source['commit']}?recursive=1")
    if tree.get("truncated"):
        raise RuntimeError(f"{source['repo']}: git tree truncated; pin a narrower path")
    patterns = [re.compile(p) for p in source["include"]]
    out: dict[str, str] = {}
    for entry in tree.get("tree", []):
        if entry.get("type") != "blob":
            continue
        path = entry["path"]
        if any(p.search(path) for p in patterns):
            out[path] = entry["sha"]
    return out


def fetch_source(source: dict) -> dict[str, str]:
    """Download one source into the cache. Returns relpath -> sha256."""
    sid = source["id"]
    dest = CACHE / sid
    hashes: dict[str, str] = {}
    if source["kind"] == "single":
        # A path may contain spaces (Wildcat's `docs/Known Issues.md`); quote
        # it, keeping the separators.
        quoted = urllib.parse.quote(source["path"], safe="/")
        data = get(f"{API}/repos/{source['repo']}/contents/{quoted}?ref={source['commit']}")
        if isinstance(data, list) or data.get("type") != "file":
            raise RuntimeError(f"{sid}: {source['path']} is not a file")
        raw = base64.b64decode(data["content"])
        target = dest / source["path"]
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(raw)
        hashes[source["path"]] = sha256(raw)
        print(f"  {sid}: {source['path']} ({len(raw)} bytes)")
        return hashes
    if source["kind"] == "tree":
        blobs = wanted_from_tree(source)
        if not blobs:
            raise RuntimeError(f"{sid}: include patterns matched no file")
        for path, blob_sha in sorted(blobs.items()):
            raw = base64.b64decode(get(f"{API}/repos/{source['repo']}/git/blobs/{blob_sha}")["content"])
            target = dest / path
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(raw)
            hashes[path] = sha256(raw)
            time.sleep(0.05)
        print(f"  {sid}: {len(blobs)} files")
        return hashes
    if source["kind"] == "c4-issues":
        return fetch_c4_issues(source, dest)
    raise RuntimeError(f"{sid}: unknown kind {source['kind']!r}")


def c4_issue_pages(repo: str) -> list[dict]:
    """Every issue of a Code4rena findings repository (bodies and labels are in
    the list response), oldest first. Pull requests are dropped."""
    out: list[dict] = []
    page = 1
    while True:
        batch = get(f"{API}/repos/{repo}/issues?state=all&per_page=100&page={page}&direction=asc&sort=created")
        if not batch:
            break
        out.extend(i for i in batch if "pull_request" not in i)
        page += 1
        time.sleep(0.2)
    return out


def c4_layout(issues: list[dict]) -> dict[str, dict]:
    """Map the judge's labels onto the judging layout the Sherlock corpora use.

    Rules, frozen with the corpus (BENCHMARK.md):
    - In scope: an issue with a High/Medium severity label, a report id label
      (H-NN/M-NN), or `downgraded by judge`. QA and Gas reports are out; an
      issue labelled `withdrawn by warden` is out.
    - Group root: follow `duplicate-N` labels transitively; an issue with none
      is its own root. A root outside the in-scope set (for example a QA report
      that absorbed a downgraded finding) does not form a group: its members
      become single excluded findings.
    - A group is accepted (paid) when any member carries a report id; its
      severity is that id's letter and its representative (*-best.md) is the
      member with the report id. Every other group is excluded.
    Returns relpath -> {"issue": n, "text": markdown} for the cache.
    """
    labels = {i["number"]: [l["name"] for l in i.get("labels", [])] for i in issues}
    by_number = {i["number"]: i for i in issues}

    def in_scope(n: int) -> bool:
        ls = labels.get(n, [])
        if any(w in ls for w in C4_WITHDRAWN):
            return False
        return (any(s in ls for s in C4_SEVERITY) or "downgraded by judge" in ls
                or any(C4_REPORT_ID.match(x) for x in ls))

    def root_of(n: int) -> int:
        seen = set()
        while n not in seen:
            seen.add(n)
            nxt = [int(m.group(1)) for x in labels.get(n, []) if (m := C4_DUPLICATE.match(x))]
            if not nxt or nxt[0] == n:
                return n
            n = nxt[0]
        return n

    groups: dict[int, list[int]] = {}
    for n in sorted(labels):
        if not in_scope(n):
            continue
        r = root_of(n)
        key = r if in_scope(r) else -n          # orphaned member: its own group
        groups.setdefault(key, []).append(n)
    for key in list(groups):
        if key > 0 and key not in groups[key]:
            groups[key].insert(0, key)

    def text(n: int) -> str:
        issue = by_number[n]
        body = (issue.get("body") or "").replace("\r\n", "\n")
        return f"# {issue['title'].strip()}\n\n{body}\n"

    files: dict[str, dict] = {}
    for key, members in sorted(groups.items(), key=lambda kv: abs(kv[0])):
        members = sorted(set(members))
        ids = [(n, m) for n in members for x in labels.get(n, []) if (m := C4_REPORT_ID.match(x))]
        root = abs(key)
        if root > 999:
            raise RuntimeError(f"issue #{root}: the judging layout uses three-digit directory names")
        if ids:
            rep, m = sorted(ids)[0]
            folder = f"{root:03d}-{m.group(1)}"
            for n in members:
                name = f"{n:03d}-best.md" if n == rep else f"{n:03d}.md"
                files[f"{folder}/{name}"] = {"issue": n, "text": text(n)}
        elif len(members) == 1:
            n = members[0]
            files[f"invalid/{n:03d}.md"] = {"issue": n, "text": text(n)}
        else:
            for n in members:
                name = f"{n:03d}-best.md" if n == root else f"{n:03d}.md"
                files[f"invalid/invalid-{root:03d}/{name}"] = {"issue": n, "text": text(n)}
    return files


def fetch_c4_issues(source: dict, dest: Path) -> dict[str, str]:
    """Code4rena judging lives in GitHub issue labels, not files. Fetch every
    issue once and write the judged layout into the cache; the lock pins each
    written file by sha256, so a later edit to an issue shows up in --check."""
    issues = c4_issue_pages(source["repo"])
    if not issues:
        raise RuntimeError(f"{source['id']}: no issues")
    files = c4_layout(issues)
    hashes: dict[str, str] = {}
    for rel, entry in sorted(files.items()):
        raw = entry["text"].encode("utf-8")
        target = dest / rel
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(raw)
        hashes[rel] = sha256(raw)
    # The label snapshot behind the layout, for the reader and for audits.
    snap = json.dumps(
        [{"number": i["number"], "updated_at": i.get("updated_at"),
          "labels": sorted(l["name"] for l in i.get("labels", []))} for i in issues],
        indent=1, sort_keys=True).encode("utf-8")
    (dest / "_labels.json").write_bytes(snap)
    hashes["_labels.json"] = sha256(snap)
    print(f"  {source['id']}: {len(issues)} issues -> {len(files)} judged files")
    return hashes


def read_lock() -> dict:
    return json.loads(LOCK.read_text(encoding="utf-8"))


def verify_lock() -> list[str]:
    problems = []
    lock = read_lock()
    for sid, entry in lock["sources"].items():
        for path, want in entry["files"].items():
            target = CACHE / sid / path
            if not target.is_file():
                problems.append(f"missing {sid}/{path}")
                continue
            got = sha256(target.read_bytes())
            if got != want:
                problems.append(f"sha256 changed {sid}/{path}: lock {want[:12]}, cache {got[:12]}")
    return problems


def fetch(_args) -> int:
    global TOKEN
    TOKEN = token()
    CACHE.mkdir(parents=True, exist_ok=True)
    lock = {"version": 1, "api": API, "sources": {}}
    for source in specs():
        print(f"fetch {source['id']}")
        files = fetch_source(source)
        lock["sources"][source["id"]] = {
            "repo": source["repo"],
            "commit": source["commit"],
            "kind": source["kind"],
            "role": source["role"],
            "files": files,
        }
        if source["kind"] == "c4-issues":
            # Issues are not in git: the commit above is the findings
            # repository's tip when fetched, recorded for context only.
            lock["sources"][source["id"]]["fetched_at"] = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
    LOCK.write_text(json.dumps(lock, indent=2) + "\n", encoding="utf-8", newline="\n")
    total = sum(len(e["files"]) for e in lock["sources"].values())
    print(f"wrote {LOCK.relative_to(ROOT).as_posix()} ({total} files)")
    return 0


def check(_args) -> int:
    if not LOCK.is_file():
        print("eval/benchmark: no cache.lock.json; run fetch first", file=sys.stderr)
        return 1
    problems = verify_lock()
    if problems:
        for p in problems:
            print(f"MISMATCH {p}", file=sys.stderr)
        return 1
    lock = read_lock()
    total = sum(len(e["files"]) for e in lock["sources"].values())
    print(f"cache matches cache.lock.json ({total} files across {len(lock['sources'])} sources)")
    return 0


# --------------------------------------------------------------------------
# Manifest: pointers + labels only. Never finding bodies.
# --------------------------------------------------------------------------

AUDIT_HEADING = re.compile(r"^##\s+\[\[(H|M)-(\d+)\]\s+(.+?)\]\(#?", re.M)


def prior_items(audit_text: str, item_prefix: str, source_id: str) -> list[dict]:
    """Every High/Medium finding heading is one prior item. The locator is the
    report's own id (H-01..); the title is truncated: a locator, not the text."""
    items = []
    for match in AUDIT_HEADING.finditer(audit_text):
        sev, num, title = match.group(1), match.group(2), match.group(3)
        locator = f"{sev}-{int(num):02d}"
        items.append({
            "id": f"{item_prefix}:{locator}",
            "source": source_id,
            "locator": locator,
            "title": re.sub(r"\s+", " ", title).strip()[:100],
        })
    return sorted(items, key=lambda i: (i["locator"][0], i["locator"]))


def known_issue_items(readme_text: str, item_prefix: str, source_id: str,
                      pattern: str = LEGACY_CORPUS["known_issue_pattern"]) -> list[dict]:
    """The sponsor's known-issues section (group 1 of `pattern`). Bullets become
    items; a wrapped bullet's continuation lines are joined into it. Kept as
    short locators."""
    m = re.search(pattern, readme_text)
    if not m:
        return []
    body = m.group(1).strip()
    items: list[str] = []
    for raw in body.splitlines():
        line = raw.strip()
        if not line:
            continue
        bullet = line[:2] in ("- ", "* ") or line.startswith("•")
        if bullet or not items:
            items.append(line.strip(" -*•").strip())
        else:
            # A continuation line belongs to the bullet above it; before this
            # fix every wrapped line became an item of its own.
            items[-1] = f"{items[-1]} {line.strip(' -*•').strip()}".strip()
    out = []
    for i, line in enumerate(items, 1):
        out.append({
            "id": f"{item_prefix}:{i}",
            "source": source_id,
            "locator": f"KI-{i}",
            "text": re.sub(r"\s+", " ", line)[:100],
            "aliases": [re.sub(r"\s+", " ", line).lower()[:100]],
        })
    return out


def title_of(path: Path) -> str:
    """The first markdown heading of a submission, truncated. A locator, not the
    finding text."""
    text = path.read_text(encoding="utf-8", errors="replace")
    for line in text.splitlines():
        if line.strip().startswith("#"):
            return re.sub(r"\s+", " ", line.strip("# ").strip())[:100]
    return ""


def judging_findings(judging_root: Path) -> list[dict]:
    """One finding per accepted directory and per invalid file/group.

    Accepted: NNN-H/ or NNN-M/ with one representative *-best.md (the
    convention in these repos); duplicates in the same directory are the same
    judged finding and are not separate rows. Invalid: invalid/*.md one each,
    invalid/invalid-NNN/ one each via its *-best.md.
    """
    out = []
    for d in sorted(judging_root.iterdir()):
        if not d.is_dir():
            continue
        name = d.name
        if re.match(r"^\d{3}-[HM]$", name):
            best = sorted(d.glob("*-best.md"))
            files = sorted(d.glob("*.md"))
            rep = best[0] if best else files[0] if files else None
            if rep is None:
                continue
            out.append({
                "id": f"acc:{name}",
                "judged": "accepted",
                "path": rep.relative_to(judging_root).as_posix(),
                "group_files": len(files),
                "files": [f.relative_to(judging_root).as_posix() for f in files],
                "title": title_of(rep),
            })
        elif name == "invalid":
            for f in sorted(d.glob("*.md")):
                out.append({
                    "id": f"inv:{f.stem}",
                    "judged": "invalid",
                    "path": f.relative_to(judging_root).as_posix(),
                    "group_files": 1,
                    "files": [f.relative_to(judging_root).as_posix()],
                    "title": title_of(f),
                })
            for g in sorted(d.glob("invalid-*")):
                if not g.is_dir():
                    continue
                best = sorted(g.glob("*-best.md"))
                files = sorted(g.glob("*.md"))
                rep = best[0] if best else files[0] if files else None
                if rep is None:
                    continue
                out.append({
                    "id": f"inv:{g.name}",
                    "judged": "invalid",
                    "path": rep.relative_to(judging_root).as_posix(),
                    "group_files": len(files),
                    "files": [f.relative_to(judging_root).as_posix() for f in files],
                    "title": title_of(rep),
                })
    return out


def source_by_role(role: str) -> dict:
    for source in specs():
        if source["role"] == role:
            return source
    raise RuntimeError(f"sources.yaml has no source with role {role!r}")


def corpus_meta() -> dict:
    meta = dict(LEGACY_CORPUS)
    meta.update(spec_file().get("corpus") or {})
    return meta


def build_manifest(_args) -> int:
    if not LOCK.is_file():
        print("eval/benchmark: no cache.lock.json; run fetch first", file=sys.stderr)
        return 1
    problems = verify_lock()
    if problems:
        for p in problems:
            print(f"MISMATCH {p}", file=sys.stderr)
        return 1
    lock = read_lock()
    labels = yaml.safe_load(LABELS.read_text(encoding="utf-8")) if LABELS.is_file() else {}
    labels = labels.get("findings", {}) if isinstance(labels, dict) else {}

    meta = corpus_meta()
    prior_src = source_by_role("prior-audit")
    known_src = source_by_role("known-issues")
    judging_src = source_by_role("judging")

    audit = (CACHE / prior_src["id"] / prior_src["path"]).read_text(encoding="utf-8")
    readme = (CACHE / known_src["id"] / known_src["path"]).read_text(encoding="utf-8")
    priors = prior_items(audit, meta["prior_item_prefix"], prior_src["id"]) + known_issue_items(
        readme, meta["known_item_prefix"], known_src["id"], meta["known_issue_pattern"])
    findings = judging_findings(CACHE / judging_src["id"])

    rows = []
    for finding in findings:
        label = labels.get(finding["id"], {})
        row = {
            "id": finding["id"],
            "judged": finding["judged"],
            "pointer": {
                "repo": judging_src["repo"],
                "commit": lock["sources"][judging_src["id"]]["commit"],
                "path": finding["path"],
            },
            "group_files": finding["group_files"],
            "files": finding["files"],
            "title": finding["title"],
            "label": label.get("label", "unlabelled"),
            "label_method": label.get("method", "unlabelled"),
            "prior_ref": label.get("prior_ref"),
            "provenance": label.get("provenance"),
            "confidence": label.get("confidence"),
            "notes": label.get("notes"),
        }
        if finding["judged"] == "invalid" and row["label"] == "unlabelled":
            row["label"] = "invalid_unmapped"
        if finding["judged"] == "accepted" and row["label"] == "unlabelled":
            row["label"] = "unique_valid"
            row["label_method"] = "platform"
        rows.append(row)

    doc = {
        "version": 1,
        "schema": "priorart.benchmark.manifest/v1",
        "seed": seed(),
        "note": (
            "Pointers and labels only: repo@commit + path per finding, plus the "
            "prior-item locators it is labelled against. No finding body is stored "
            "here. The bodies are fetched into eval/cache/benchmark/ by fetch.py. "
            "A label without method 'reader' is a platform label or the frozen "
            "rubric, not a person's judgement; see eval/benchmark/BENCHMARK.md."
        ),
        "sources": {
            "prior_audit": {
                "id": prior_src["id"],
                "repo": prior_src["repo"],
                "commit": lock["sources"][prior_src["id"]]["commit"],
                "path": prior_src["path"],
            },
            "known_issues": {
                "id": known_src["id"],
                "repo": known_src["repo"],
                "commit": lock["sources"][known_src["id"]]["commit"],
                "path": known_src["path"],
                "section": meta["known_issue_section"],
            },
        },
        "prior_items": {
            # dossier A: what a hunter would have before writing the finding
            "prior_audit_and_known": priors,
            # dossier B: the judged primary submissions of the same contest,
            # used only by the supplementary duplicate-retrieval variant
            "judged_primary": [
                {
                    "id": r["id"],
                    "source": r["pointer"]["repo"],
                    "locator": r["id"],
                    "title": r["title"] or r["id"],
                    "variant": "judged_primary",
                }
                for r in rows
            ],
        },
        "findings": rows,
        "duplicates": [
            {
                "id": f"dup:{path}",
                "expected": r["id"],
                "pointer": dict(r["pointer"], path=path),
                "provenance": (
                    "judged duplicate: the judging repository groups this file in "
                    "the same directory as the representative (the judge's own grouping)"
                ),
                "label_method": "platform",
            }
            for r in rows
            for path in r["files"]
            if path != r["pointer"]["path"]
        ],
        "evaluation": {
            "known_prior_recall": {
                "dossier": "prior_audit_and_known",
                "queries": "findings labelled known_prior",
                "expected": "the prior item in prior_ref",
                "k": 5,
            },
            "false_alarm_rate": {
                "dossier": "prior_audit_and_known",
                "queries": "findings labelled unique_valid (judged paid Medium/High, unique)",
                "alarm": "any returned prior_art item (the matcher requires >= 2 strong identifiers)",
            },
            "duplicate_recall": {
                "dossier": "judged_primary",
                "queries": "duplicates[] entries",
                "expected": "the duplicate's expected judged representative",
                "k": 5,
                "note": "supplementary: judged within-contest duplicate retrieval, not the plan's known-issue metric",
            },
        },
        "counts": {
            "prior_items": len(priors),
            "findings": len(rows),
            "accepted": sum(1 for r in rows if r["judged"] == "accepted"),
            "invalid": sum(1 for r in rows if r["judged"] == "invalid"),
            "duplicates": sum(len(r["files"]) - 1 for r in rows),
            "known_prior": sum(1 for r in rows if r["label"] == "known_prior"),
            "reader_labelled": sum(1 for r in rows if r["label_method"] == "reader"),
        },
    }
    MANIFEST.write_text(yaml.safe_dump(doc, sort_keys=False, allow_unicode=True), encoding="utf-8", newline="\n")
    print(f"wrote {MANIFEST.relative_to(ROOT).as_posix()}: {len(rows)} findings, {len(priors)} prior items")
    print("  " + json.dumps(doc["counts"]))
    return 0


def list_pins(_args) -> int:
    for source in specs():
        where = source.get("path", ",".join(source.get("include", [])))
        print(f"{source['id']}: {source['role']}, {source['repo']}@{source['commit'][:12]} ({source['kind']}: {where})")
    return 0


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description="fetch, verify and describe the pinned priorart benchmark corpus")
    parser.add_argument("--corpus-dir", default=str(BENCH),
                        help="corpus directory holding sources.yaml (default: the Tapioca corpus here)")
    group = parser.add_mutually_exclusive_group()
    group.add_argument("--check", action="store_true", help="offline: cache matches cache.lock.json")
    group.add_argument("--manifest", action="store_true", help="offline: write manifest.yaml")
    group.add_argument("--list", action="store_true", help="print the pinned sources")
    args = parser.parse_args(argv)
    set_corpus(args.corpus_dir)
    if args.check:
        return check(args)
    if args.manifest:
        return build_manifest(args)
    if args.list:
        return list_pins(args)
    return fetch(args)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
