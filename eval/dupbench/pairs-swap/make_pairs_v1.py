#!/usr/bin/env python3
"""Writes pairs_v1_swapped.json: the six V1 pairs after the swap.

Kept from pairs.json (v1), unchanged: `2024-03-revert-lend`, `2024-10-loopfi`.
Added from sealed_optA.json (sealed before any finding was read):
`2022-11-paraspace`, `2023-02-ethos`, `2023-09-venus`, `2023-07-arcade`.
pairs.json itself is not edited (the pre-registration pins its sha256).

Offline and deterministic. Usage: python -I eval/dupbench/pairs-swap/make_pairs_v1.py [--check]
"""
import json, sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
DB = HERE.parent
NAMES = {"2022-11-paraspace": ("paraspace-2022", "Paraspace"), "2023-02-ethos": ("ethos-2023", "Ethos"),
         "2023-09-venus": ("venus-2023", "Venus"), "2023-07-arcade": ("arcade-2023", "Arcade")}
EVIDENCE = {
    "2023-02-ethos": "WEAK: the only prior-audit copy is a Discord export attachment inside the sponsor's code repo (`discord-export/`), pinned by sha256 in sealed_optA.json; the sponsor README was not read, so the same-codebase link is by project identity only",
}


def build() -> dict:
    v1 = json.loads((DB / "pairs.json").read_text(encoding="utf-8"))
    sealed = json.loads((HERE / "sealed_optA.json").read_text(encoding="utf-8"))
    keep = [p for p in v1["pairs"] if p["contest"]["slug"] in ("2024-03-revert-lend", "2024-10-loopfi")]
    new = []
    for c in sealed["contests"]:
        if c.get("role") != "selected":
            continue
        pid, sponsor = NAMES[c["slug"]]
        f = c["prior_audit_files"][0]
        repo = c["codebase"]["repo"].rstrip("/")
        new.append({
            "pair_id": pid,
            "sponsor": sponsor,
            "prior_report": {
                "project": sponsor,
                "file": f["path"],
                "git_blob": f["git_blob"],
                "bytes": f["bytes"],
                "sha256": f["sha256"],
                "url": f"{repo}/blob/{c['codebase']['commit']}/" + f["path"].replace(" ", "%20"),
                "firm": "not read (the audit file was hashed, never parsed)",
                "date_note": "not read",
            },
            "contest": {"slug": c["slug"], "repo": c["findings"]["repo"], "commit": c["findings"]["commit"]},
            "codebase": {"repo": c["codebase"]["repo"], "commit": c["codebase"]["commit"], "source": c["codebase"]["source"]},
            "same_codebase_evidence": EVIDENCE.get(c["slug"], "the audit file is shipped in the sponsor's code repo at the pinned commit (sealed_optA.json, rule 5/5a)"),
            "drift": "not assessed: no audit text was read",
        })
    return {
        "version": 2,
        "schema": "priorart.dupbench.pairs/v1",
        "note": "V1 pairs after the swap decided by Yura 2026-11-27 (option A). pairs.json (v1, six pairs) is unchanged and still pinned by the pre-registration; this file replaces the four pairs that are Dup-Bench contests (basin, size, ethena, salty) with the four sealed contests.",
        "sponsors_used_before": v1["sponsors_used_before"],
        "pairs": keep + new,
    }


if __name__ == "__main__":
    text = json.dumps(build(), indent=2, sort_keys=True) + "\n"
    out = HERE / "pairs_v1_swapped.json"
    if "--check" in sys.argv:
        sys.exit(0 if out.read_text(encoding="utf-8") == text else 1)
    out.write_text(text, encoding="utf-8", newline="\n")
    print(f"wrote {out}")