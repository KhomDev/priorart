#!/usr/bin/env python3
"""Re-fetch pinned upstream bytes and check sha256.

Does not run priorart. Writes downloads under .corpus/sealed-multilang/
(gitignored). A unit's body_sha256 is the sha256 of the git blob contents,
which is what raw.githubusercontent.com returns for that commit and path.
"""

from __future__ import annotations

import hashlib
import json
import sys
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
UNITS = Path(__file__).resolve().parent / "units.json"
CACHE = ROOT / ".corpus" / "sealed-multilang"


def pins() -> list[tuple[str, str, str]]:
    doc = json.loads(UNITS.read_text(encoding="utf-8"))
    seen: set[tuple[str, str]] = set()
    out = []
    for unit in doc["units"]:
        url = unit["source_url"].split("#", 1)[0]
        prefix = "https://github.com/"
        if not url.startswith(prefix):
            raise SystemExit(f"unexpected url {url}")
        rest = url[len(prefix):]
        owner, repo, blob, commit, path = rest.split("/", 4)
        if blob != "blob":
            raise SystemExit(f"unexpected url {url}")
        key = (commit, path)
        if key in seen:
            continue
        seen.add(key)
        raw = (
            "https://raw.githubusercontent.com/"
            f"{owner}/{repo}/{commit}/{urllib.parse.quote(path)}"
        )
        out.append((raw, path, unit["body_sha256"]))
    return out


def get(url: str) -> bytes:
    req = urllib.request.Request(url, headers={"User-Agent": "priorart-sealed-fetch"})
    last: Exception | None = None
    for _ in range(4):
        try:
            with urllib.request.urlopen(req, timeout=60) as resp:
                return resp.read()
        except urllib.error.HTTPError as exc:
            last = exc
            if exc.code not in (429, 502, 503):
                raise
        except urllib.error.URLError as exc:
            last = exc
    raise RuntimeError(f"GET {url} failed: {last}")


def main() -> int:
    if "--check" not in sys.argv:
        print("usage: python eval/sealed-multilang/fetch.py --check", file=sys.stderr)
        return 2
    rows = pins()
    CACHE.mkdir(parents=True, exist_ok=True)
    bad = []
    for i, (url, path, want) in enumerate(rows, 1):
        data = get(url)
        digest = hashlib.sha256(data).hexdigest()
        target = CACHE / want
        target.write_bytes(data)
        if digest != want:
            bad.append(f"{path}: want {want[:12]} got {digest[:12]}")
        if i % 100 == 0 or i == len(rows):
            print(f"fetched {i}/{len(rows)}", flush=True)
    if bad:
        print(f"fetch check failed ({len(bad)})")
        for line in bad[:20]:
            print(line)
        return 1
    print(f"fetch check ok ({len(rows)} files)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
