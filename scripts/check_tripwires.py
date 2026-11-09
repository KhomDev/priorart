#!/usr/bin/env python3
"""Repo hygiene tripwires for viability-gate.

Four checks, all of which fail closed:

  A. PRIVATE STRINGS   No file in this repository may contain a private-workspace
                       path prefix. The prefixes are assembled from fragments at
                       the bottom of this file so that the checker does not itself
                       contain them in plaintext -- otherwise the checker would be
                       the leak it is meant to catch.

  B. PACK BALANCE      rules/packs/ must hold at least MIN_ECOSYSTEMS ecosystem
                       packs, and the largest pack must not carry more than
                       MAX_IMBALANCE times the entries of the smallest. An
                       unbalanced pack set tells a reader which ecosystem the
                       author hunts, which is a disclosure.

  C. TOKEN CONFINEMENT Ecosystem tokens declared by the packs must not appear in
                       the shipped rule catalog. The catalog is embedded in the
                       binary and is the surface a user reads; if it carries one
                       ecosystem's tokens directly, the packs are pointless.

  D. ENCODING          No file may carry a mojibake byte sequence. A UTF-8 dash
                       re-saved through cp1252 is invisible in a diff and wrong in
                       every renderer.

  E. CASE CLASSES      Every catalog pattern is compiled case-insensitive, so an
                       [A-Z] class must sit inside a (?-i:...) group or it also
                       matches lower case.

Run:  python scripts/check_tripwires.py
      python scripts/check_tripwires.py --json

Exit code 0 = clean, 1 = at least one violation, 2 = the check could not run.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent

MIN_ECOSYSTEMS = 7
MAX_IMBALANCE = 2.0

# The shipped catalog: the two files embedded into the binary via include_str!.
# These are the files a user reads and the files whose token inventory is the
# thing that could disclose an author's ecosystem. See docs/tripwires.md for why
# the docs, the tests and src/ are deliberately outside this surface.
CONFINEMENT_SURFACE = (
    Path("rules/anti-patterns.yaml"),
    Path("rules/gates.yaml"),
)

PACK_DIR = Path("rules/packs")

SKIP_DIRS = {
    ".git",
    "target",
    "node_modules",
    "__pycache__",
    ".pytest_cache",
    ".ruff_cache",
    ".mypy_cache",
    ".venv",
    "venv",
    "dist",
    "build",
    ".baseline",
    "repro",
}

# Token shapes that are matched literally (they are not words).
_SYMBOLIC = re.compile(r"[.#@!(){}\[\]/\\:]")


def tracked_files() -> list[Path]:
    """Every file that would be committed: tracked plus untracked-but-not-ignored."""
    try:
        out = subprocess.run(
            ["git", "ls-files", "--cached", "--others", "--exclude-standard"],
            cwd=REPO_ROOT,
            capture_output=True,
            text=True,
            check=True,
        ).stdout
        files = [Path(line) for line in out.splitlines() if line.strip()]
        if files:
            return sorted(files)
    except (OSError, subprocess.CalledProcessError):
        pass
    # No git (or an empty repo): walk the tree.
    files = []
    for dirpath, dirnames, filenames in os.walk(REPO_ROOT):
        dirnames[:] = [d for d in dirnames if d not in SKIP_DIRS]
        for name in filenames:
            files.append(Path(dirpath, name).relative_to(REPO_ROOT))
    return sorted(files)


def read_text(path: Path) -> str | None:
    """File contents, or None for anything that is not decodable text."""
    try:
        raw = (REPO_ROOT / path).read_bytes()
    except OSError:
        return None
    if b"\x00" in raw:
        return None
    try:
        return raw.decode("utf-8")
    except UnicodeDecodeError:
        return None


# --------------------------------------------------------------------- check A


# The vendored snapshots' own denylist checkers name the prefixes in order to
# search for them. Skipping those two files is the same allowance they give
# themselves. Every other file in the snapshot is still scanned.
DENYLIST_CHECKERS = {
    Path("data/rejection-taxonomy/scripts/check_hygiene.py"),
    Path("data/bounty-economics/scripts/check_hygiene.py"),
}


def check_private_strings(files: list[Path]) -> list[str]:
    problems: list[str] = []
    for path in files:
        if path in DENYLIST_CHECKERS:
            continue
        text = read_text(path)
        if text is None:
            continue
        for lineno, line in enumerate(text.splitlines(), start=1):
            for needle in _private_strings():
                if needle in line:
                    problems.append(
                        f"{path}:{lineno}: private-workspace string present "
                        f"(prefix {needle[0]}..., {len(needle)} chars)"
                    )
    return problems


# --------------------------------------------------------------------- check D

# A UTF-8 dash or accent re-saved through a cp1252 reader. These sequences are
# never intentional, and they are invisible in a diff while being wrong in every
# renderer. The sibling repositories check for them; this one did not, which is
# how the gap was noticed.
_MOJIBAKE = (
    "\u00e2\u20ac\u201d",
    "\u00e2\u20ac\u201c",
    "\u00e2\u20ac\u2122",
    "\u00e2\u20ac\u0153",
    "\u00c3\u00a9",
    "\u00c3\u00a8",
    "\u00c2\u00a0",
    "\ufffd",
)


def check_encoding(files: list[Path]) -> list[str]:
    problems: list[str] = []
    for path in files:
        text = read_text(path)
        if text is None:
            continue
        for seq in _MOJIBAKE:
            if seq in text:
                lineno = text[: text.index(seq)].count("\n") + 1
                problems.append(
                    f"{path}:{lineno}: mojibake sequence {seq!r} - re-save the file as UTF-8"
                )
    return problems


# --------------------------------------------------------------------- check B


def load_packs() -> tuple[list[dict], list[str]]:
    problems: list[str] = []
    packs: list[dict] = []
    pack_dir = REPO_ROOT / PACK_DIR
    if not pack_dir.is_dir():
        return [], [f"{PACK_DIR}/ is missing"]
    for path in sorted(pack_dir.glob("*.yaml")):
        try:
            import yaml
        except ImportError:  # pragma: no cover
            return [], ["PyYAML is not installed: pip install pyyaml"]
        try:
            doc = yaml.safe_load(path.read_text(encoding="utf-8"))
        except Exception as exc:  # noqa: BLE001 - report, do not crash
            problems.append(f"{PACK_DIR / path.name}: not valid YAML ({exc})")
            continue
        if not isinstance(doc, dict):
            problems.append(f"{PACK_DIR / path.name}: top level must be a mapping")
            continue
        missing = [k for k in ("id", "ecosystem", "tokens", "path_globs", "test_markers", "notes") if k not in doc]
        if missing:
            problems.append(f"{PACK_DIR / path.name}: missing key(s) {', '.join(missing)}")
            continue
        if doc["id"] != path.stem:
            problems.append(f"{PACK_DIR / path.name}: id {doc['id']!r} does not match the filename")
        for key in ("tokens", "path_globs", "test_markers"):
            if not isinstance(doc[key], list) or not all(isinstance(v, str) for v in doc[key]):
                problems.append(f"{PACK_DIR / path.name}: {key} must be a list of strings")
        if not isinstance(doc["notes"], str) or not doc["notes"].strip():
            problems.append(f"{PACK_DIR / path.name}: notes must be non-empty")
        packs.append(doc)
    return packs, problems


def pack_entries(pack: dict) -> int:
    return sum(len(pack.get(k) or []) for k in ("tokens", "path_globs", "test_markers"))


def check_pack_balance(packs: list[dict]) -> list[str]:
    problems: list[str] = []
    if len(packs) < MIN_ECOSYSTEMS:
        problems.append(
            f"{PACK_DIR}/ holds {len(packs)} ecosystem pack(s); "
            f"at least {MIN_ECOSYSTEMS} are required for the set to be balanced"
        )
    if not packs:
        return problems
    counts = {p["id"]: pack_entries(p) for p in packs}
    smallest = min(counts.values())
    largest = max(counts.values())
    if smallest == 0:
        problems.append(f"{PACK_DIR}/: a pack declares no entries at all ({counts})")
        return problems
    if largest > MAX_IMBALANCE * smallest:
        worst = [k for k, v in counts.items() if v == largest]
        best = [k for k, v in counts.items() if v == smallest]
        problems.append(
            f"{PACK_DIR}/ is unbalanced: largest pack {worst} has {largest} entries, "
            f"smallest {best} has {smallest} (limit {MAX_IMBALANCE}x). Add entries to the "
            f"small packs rather than removing them from the large one."
        )
    return problems


# --------------------------------------------------------------------- check C


def token_patterns(token: str) -> list[re.Pattern[str]]:
    """Regexes that count as an appearance of `token`.

    A file extension is also matched in its bare form, because a regex written as
    an alternation of extensions -- a dotted group listing several extensions --
    carries exactly the same disclosure as the dotted form and would otherwise
    slip through. Bare forms
    shorter than three characters are not matched: `go`, `rs` and `ts` collide
    with ordinary words, so only the dotted form counts for those.
    """
    if token.startswith(".") and len(token) >= 4:
        bare = token[1:]
        return [
            re.compile(re.escape(token)),
            re.compile(r"(?<![A-Za-z0-9_])" + re.escape(bare) + r"(?![A-Za-z0-9_])", re.IGNORECASE),
        ]
    if _SYMBOLIC.search(token):
        return [re.compile(re.escape(token))]
    return [re.compile(r"(?<![A-Za-z0-9_])" + re.escape(token) + r"(?![A-Za-z0-9_])", re.IGNORECASE)]


def check_token_confinement(packs: list[dict]) -> list[str]:
    problems: list[str] = []
    tokens: list[tuple[str, str, list[re.Pattern[str]]]] = []
    for pack in packs:
        for token in pack.get("tokens") or []:
            tokens.append((pack["id"], token, token_patterns(token)))
    for rel in CONFINEMENT_SURFACE:
        text = read_text(rel)
        if text is None:
            problems.append(f"{rel}: missing or unreadable")
            continue
        for lineno, line in enumerate(text.splitlines(), start=1):
            hits: list[str] = []
            for pack_id, token, patterns in tokens:
                if any(p.search(line) for p in patterns):
                    hits.append(f"{token!r} (pack {pack_id})")
            if hits:
                problems.append(
                    f"{rel}:{lineno}: ecosystem token(s) {', '.join(hits)} appear in the shipped "
                    f"catalog; they belong in rules/packs/"
                )
    return problems


# ------------------------------------------------------------------ self-test

# Fixtures for the matcher itself, because a boundary rule that is too loose
# reports ordinary English as a disclosure and one that is too tight misses a
# real one. Both directions are pinned, and CI runs this in the tripwires job,
# so a regression in the matcher cannot land silently.
_BOUNDARY_FIXTURES = (
    # (text, token, should_match)
    ("a suite of checks", "sui", False),
    ("the nearest gate", "near", False),
    ("a button in the UI", "ton", False),
    ("function returns a value", "func", False),
    ("remove the guard", "move", False),
    ("movement in storage", "move", False),
    ("checks for sui", "sui", True),
    ("near the boundary", "near", True),
    ("the ton balance", "ton", True),
    ("a func declaration", "func", True),
    ("move the funds", "move", True),
)


def self_test() -> list[str]:
    """Check the boundary rule against the fixtures. Empty list means clean."""
    problems: list[str] = []
    for text, token, expected in _BOUNDARY_FIXTURES:
        got = any(p.search(text) for p in token_patterns(token))
        if got != expected:
            problems.append(
                f"{text!r} against token {token!r}: expected match={expected}, got {got}"
            )
    return problems


# --------------------------------------------------------------------- main


PATTERN_FIELDS = ("any_of", "all_of", "none_of", "escalate_on", "downgrade_on")


def _strip_case_sensitive_groups(pattern: str) -> str:
    """Remove every (?-i:...) group, honouring nested and escaped parentheses."""
    out: list[str] = []
    i = 0
    while i < len(pattern):
        if pattern.startswith("(?-i:", i):
            depth = 0
            while i < len(pattern):
                c = pattern[i]
                if c == "\\":
                    i += 2
                    continue
                if c == "(":
                    depth += 1
                elif c == ")":
                    depth -= 1
                    if depth == 0:
                        i += 1
                        break
                i += 1
            continue
        out.append(pattern[i])
        i += 1
    return "".join(out)


def check_case_classes() -> list[str]:
    """E. CASE CLASSES: every pattern is compiled case-insensitive, so an [A-Z]
    class outside a (?-i:...) group matches lower case too. That silently
    turned P13's ticker suppression into "any short word"."""
    try:
        import yaml
    except ImportError:
        return ["PyYAML is required for the case-class check (pip install pyyaml)"]
    catalog = yaml.safe_load((REPO_ROOT / "rules" / "anti-patterns.yaml").read_text(encoding="utf-8"))
    problems: list[str] = []
    for rule in catalog.get("rules", []):
        for field in PATTERN_FIELDS:
            for pattern in rule.get(field) or []:
                if re.search(r"\[[^\]]*A-Z", _strip_case_sensitive_groups(pattern)):
                    problems.append(
                        f"{rule['id']}.{field}: {pattern!r} has an [A-Z] class outside "
                        f"(?-i:...); under the catalog's case-insensitive flag it also "
                        f"matches lower case"
                    )
    return problems


def _private_strings() -> list[str]:
    # Assembled from fragments on purpose: this file must not contain the
    # literals it forbids, or it would be its own first violation.
    return [
        "KNOW" + "LEDGE/",
        "_private" + "-tools",
        "RESULTS" + "/",
        "submission" + "_guides/",
    ]


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--json", action="store_true", help="machine-readable output")
    parser.add_argument(
        "--self-test",
        action="store_true",
        help="check the matcher's boundaries against fixtures, then exit",
    )
    args = parser.parse_args(argv)

    if args.self_test:
        problems = self_test()
        if problems:
            for problem in problems:
                print(f"FAIL  {problem}")
            return 1
        print(f"ok    matcher boundaries ({len(_BOUNDARY_FIXTURES)} fixtures)")
        return 0

    files = tracked_files()
    packs, pack_problems = load_packs()
    results = {
        "private_strings": check_private_strings(files),
        "pack_balance": pack_problems + check_pack_balance(packs),
        "token_confinement": check_token_confinement(packs),
        "encoding": check_encoding(files),
        "case_classes": check_case_classes(),
    }
    total = sum(len(v) for v in results.values())
    if args.json:
        print(json.dumps({"ok": total == 0, "files_scanned": len(files), "violations": results}, indent=2))
        return 0 if total == 0 else 1

    print(f"tripwires: {len(files)} file(s) scanned, {len(packs)} ecosystem pack(s)")
    labels = {
        "private_strings": "A. private strings",
        "pack_balance": "B. pack balance",
        "token_confinement": "C. token confinement",
        "encoding": "D. encoding",
        "case_classes": "E. case classes",
    }
    for key, problems in results.items():
        if problems:
            print(f"\nFAIL  {labels[key]}")
            for p in problems:
                print(f"      {p}")
        else:
            print(f"ok    {labels[key]}")
    if total:
        print(f"\n{total} violation(s).")
        return 1
    print("\nall tripwires clean.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
