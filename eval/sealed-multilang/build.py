#!/usr/bin/env python3
"""Collect pointer units for eval/sealed-multilang.

Reads pinned local checkouts only to hash git blobs and copy short outcome
quotes. Does not run priorart. Writes units.json next to this file.

Checkouts (depth 1, at the commits recorded in units.json):
  /tmp/src/defihacklabs
  /tmp/src/immunefi
  /tmp/src/meteora-audits
"""

from __future__ import annotations

import hashlib
import json
import re
import subprocess
from pathlib import Path

import yaml

from split import split_of

ROOT = Path(__file__).resolve().parents[2]
OUT = Path(__file__).resolve().parent / "units.json"

RUNNABLE = re.compile(
    r"(?m)(#\[(?:tokio::)?test\]|#\[cfg\(test\)\]|function\s+test\w*\s*\(|^\s*it\()"
)


def blob(repo: Path, rel: str) -> bytes:
    return subprocess.check_output(["git", "-C", str(repo), "cat-file", "blob", f"HEAD:{rel}"])


def head(repo: Path) -> str:
    return subprocess.check_output(["git", "-C", str(repo), "rev-parse", "HEAD"], text=True).strip()


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def words15(text: str) -> str:
    cleaned = re.sub(r"\s+", " ", text).strip(" \t-:*`")
    return " ".join(cleaned.split()[:15])


def existing_ids() -> tuple[set[str], set[str]]:
    urls: set[str] = set()
    hashes: set[str] = set()
    doc = yaml.safe_load((ROOT / "eval/dataset.manifest.yaml").read_text(encoding="utf-8"))
    for item in doc["items"]:
        urls.add(item["url"])
        hashes.add(item["sha256"])
    sources = json.loads((ROOT / "eval/dupbench/sources.json").read_text(encoding="utf-8"))
    for repo in sources["repos"]:
        urls.add(f"https://github.com/{repo['repo']}")
    labels = json.loads((ROOT / "eval/dupbench/labels.json").read_text(encoding="utf-8"))
    # labels.json is contest -> primaries / duplicates. Record every issue URL
    # the file actually names so a later unit cannot reuse one.
    def walk(node: object, repo: str | None = None) -> None:
        if isinstance(node, dict):
            repo_here = node.get("repo") or repo
            for key, value in node.items():
                walk(value, repo_here if isinstance(repo_here, str) else repo)
        elif isinstance(node, list):
            for value in node:
                walk(value, repo)
        elif isinstance(node, int) and repo and "/" in repo:
            urls.add(f"https://github.com/{repo}/issues/{node}")

    # sources repos are the contest identity; labels may nest issue numbers
    # under a slug. Map slug -> repo from sources.
    slug_repo = {row["slug"]: row["repo"] for row in sources["repos"]}
    if isinstance(labels, dict):
        for slug, payload in labels.items():
            walk(payload, slug_repo.get(slug))
    return urls, hashes


def defihack(urls: set[str], hashes: set[str]) -> list[dict]:
    repo = Path("/tmp/src/defihacklabs")
    commit = head(repo)
    keyinfo = re.compile(r"(?m)^//\s*@KeyInfo\s*-\s*(.+)$")
    test = re.compile(r"function\s+test\w*\s*\(")
    units = []
    seen_hash: set[str] = set()
    for path in sorted(repo.joinpath("src/test").rglob("*.sol")):
        rel = str(path.relative_to(repo))
        raw = blob(repo, rel)
        text = raw.decode("utf-8", errors="replace")
        if not test.search(text):
            continue
        match = keyinfo.search(text)
        if not match:
            continue
        quote = words15(match.group(1))
        if len(quote.split()) < 3 or not re.search(r"[A-Za-z]", quote):
            continue
        digest = sha256(raw)
        normalised = sha256(text.replace("\r\n", "\n").replace("\r", "\n").encode())
        url = f"https://github.com/SunWeb3Sec/DeFiHackLabs/blob/{commit}/{rel}"
        if url in urls or digest in hashes or normalised in hashes or digest in seen_hash:
            continue
        seen_hash.add(digest)
        unit_id = "x/defihacklabs/" + rel[: -len(".sol")]
        units.append(
            {
                "unit_id": unit_id,
                "source_url": url,
                "commit": commit,
                "path": rel,
                "body_sha256": digest,
                "licence": "pointer-only",
                "upstream_licence": "repo Apache-2.0; file SPDX UNLICENSED",
                "tier": "X",
                "language": "solidity",
                "cluster": "defihacklabs",
                "outcome_quote": quote,
                "outcome_url": url,
                "group_id": path.stem,
                "duplicate_of": None,
                "runnable_test": "function test",
                "split": split_of(unit_id),
                "priorart_label": "pending",
                "held_out": True,
            }
        )
    return units


def jito() -> list[dict]:
    repo = Path("/tmp/src/immunefi")
    commit = head(repo)
    names = [
        "36903-sc-high-the-vault-reward-mechanism-can-be-sandwiched-by-mev.md",
        "37295-sc-high-rewards-can-be-stolen-by-depositing-immediately-after-reward-tokens-get-sent-to-vault.md",
        "37311-sc-high-attackers-can-steal-rewards-by-depositing-updating-vault-balance-and-withdrawing-immed.md",
        "37314-sc-high-vault-creators-can-not-withdraw-their-fees-without-being-recursively-charged-vault-and.md",
        "37315-sc-high-theft-of-unclaimed-yields-due-to-improper-reward-distribution-in-vault-program.md",
    ]
    units = []
    for name in names:
        rel = f"jito-restaking/{name}"
        raw = blob(repo, rel)
        text = raw.decode("utf-8", errors="replace")
        if not RUNNABLE.search(text):
            raise SystemExit(f"{rel} lost its runnable test marker")
        section = re.search(r"(?is)^## Impact(?: Details)?\s*\n+(.*?)(?=^## |\Z)", text, re.M)
        if not section:
            raise SystemExit(f"{rel} has no Impact section")
        paragraph = ""
        for line in section.group(1).splitlines():
            stripped = re.sub(r"[*`]+", "", line).strip()
            if stripped and not stripped.startswith("#"):
                paragraph = stripped
                break
        quote = words15(paragraph)
        if len(quote.split()) < 3:
            raise SystemExit(f"{rel} impact quote too short: {quote!r}")
        number = name.split("-", 1)[0]
        url = f"https://github.com/immunefi-team/Past-Audit-Competitions/blob/{commit}/{rel}"
        unit_id = f"j/immunefi-jito-restaking/{number}"
        units.append(
            {
                "unit_id": unit_id,
                "source_url": url,
                "commit": commit,
                "path": rel,
                "body_sha256": sha256(raw),
                "licence": "pointer-only",
                "upstream_licence": "none",
                "tier": "J",
                "language": "solana",
                "cluster": "immunefi-jito-restaking",
                "outcome_quote": quote,
                "outcome_url": url,
                "group_id": number,
                "duplicate_of": None,
                "runnable_test": "#[test]",
                "split": split_of(unit_id),
                "priorart_label": "pending",
                "held_out": True,
            }
        )
    return units


def pdf_text(repo: Path, rel: str) -> str:
    raw = blob(repo, rel)
    path = Path("/tmp/sealed-pdf") / rel
    path.parent.mkdir(parents=True, exist_ok=True)
    # pdftotext reads a file. The bytes are the git blob, written only under /tmp.
    path.write_bytes(raw)
    return subprocess.check_output(["pdftotext", "-layout", str(path), "-"], stderr=subprocess.DEVNULL).decode(
        "utf-8", errors="replace"
    )


def meteora() -> list[dict]:
    repo = Path("/tmp/src/meteora-audits")
    commit = head(repo)
    specs = [
        ("referral-staking/zenith-referral-staking-audit-0.1.0.pdf", "M-2"),
        ("dbc/zenith-dbc-audit-0.1.3.pdf", "M-1"),
        ("dbc/zenith-dbc-audit-0.1.4.pdf", "M-1"),
        ("damm-v2/zenith-damm-v2-audit-june-2025-1.pdf", "M-1"),
        ("dlmm/sec3-dlmm-audit-feb-2024.pdf", "M-01"),
        ("dlmm/sec3-dlmm-audit-feb-2024.pdf", "M-02"),
        ("dlmm/zenith-dlmm-audit-0.10.1.pdf", "M-2"),
        ("dlmm/zenith-dlmm-audit-0.10.1.pdf", "M-3"),
        ("dlmm/zenith-dlmm-audit-0.10.1.pdf", "M-4"),
        ("dlmm/zenith-dlmm-audit-0.10.1.pdf", "M-5"),
    ]
    units = []
    for rel, finding in specs:
        text = pdf_text(repo, rel)
        parts = re.split(r"(?m)^[ \t]*\[([HCM]-\d+)\]\s*([^\n]*)", text)
        best_title = ""
        best_body = ""
        i = 1
        while i + 2 < len(parts):
            fid, title, body = parts[i], parts[i + 1], parts[i + 2]
            if fid == finding and len(body) > len(best_body):
                best_title, best_body = title.strip(), body
            i += 3
        if not best_body or not RUNNABLE.search(best_body):
            raise SystemExit(f"{rel} {finding} has no runnable test in the finding section")
        quote = words15(best_title)
        if len(quote.split()) < 3:
            raise SystemExit(f"{rel} {finding} title too short: {best_title!r}")
        raw = blob(repo, rel)
        url = f"https://github.com/MeteoraAg/audits/blob/{commit}/{rel}#{finding}"
        unit_id = f"a/meteora/{rel.split('/')[0]}/{Path(rel).stem}/{finding}"
        units.append(
            {
                "unit_id": unit_id,
                "source_url": url,
                "commit": commit,
                "path": rel,
                "body_sha256": sha256(raw),
                "licence": "pointer-only",
                "upstream_licence": "none",
                "tier": "A",
                "language": "solana",
                "cluster": "meteora-audits",
                "outcome_quote": quote,
                "outcome_url": url,
                "group_id": f"{Path(rel).stem}/{finding}",
                "duplicate_of": None,
                "runnable_test": "it( or #[test]",
                "split": split_of(unit_id),
                "priorart_label": "pending",
                "held_out": True,
            }
        )
    return units


def main() -> None:
    urls, hashes = existing_ids()
    units = defihack(urls, hashes) + jito() + meteora()
    ids = [unit["unit_id"] for unit in units]
    if len(ids) != len(set(ids)):
        raise SystemExit("duplicate unit_id")
    payload = {
        "schema": "priorart.sealed-multilang/v1",
        "held_out": True,
        "priorart_label": "pending",
        "salt_commit": "0bf85079b693da95beb915fa9a5a06797d97b385",
        "note": "Pointers and sha256 only. One counted unit per finding. priorart was not run.",
        "counts": {},
        "units": units,
    }
    counts: dict[str, int] = {}
    for unit in units:
        for key in (
            f"tier:{unit['tier']}",
            f"language:{unit['language']}",
            f"cluster:{unit['cluster']}",
            f"split:{unit['split']}",
        ):
            counts[key] = counts.get(key, 0) + 1
    payload["counts"] = counts
    OUT.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(counts, indent=2))
    print("units", len(units), "wrote", OUT)


if __name__ == "__main__":
    main()
