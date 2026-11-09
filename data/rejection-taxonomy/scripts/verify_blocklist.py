#!/usr/bin/env python3
"""Defence in depth against a private token reaching a public file. Needs the private inputs.

The primary guarantee is architectural: `data/` holds vocabulary values only, and
the classifier that reads private text is not in this repository. This script
covers the two ways that guarantee can still be broken.

Mode 1 - blocklist coverage (default)
    Harvest every target-looking token from the private source - the protocol
    column, ALL-CAPS and CamelCase names, lowerCamel and snake_case identifiers,
    finding-id shapes - and report any that the blocklist does not cover.
    Run it after adding rows to the private log.

Mode 2 - repository cross-check (--scan-repo)
    Compare every tracked file in this repository with the private source.
    Reports (a) any identifier-shaped token from the private source that appears
    in a public file, and (b) any run of five consecutive words copied from the
    private source's free text. This is the check that would have caught the
    0.5.1 disclosure: classifier regexes and documentation quoting private report
    titles. Run it before every release.

Both modes read an optional private allowlist (--allowlist, a YAML mapping with a
`generic:` list) for domain words that are not target names but that would say
too much about the author's work if listed publicly.

Usage:
    python scripts/verify_blocklist.py --source <private log> --blocklist <blocklist> [--allowlist <file>]
    python scripts/verify_blocklist.py --source <private log> --scan-repo [--allowlist <file>]
"""

from __future__ import annotations

import argparse
import re
import subprocess
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DECLARATION_KEYS = {"case_sensitive_groups", "case_insensitive_groups"}

# Plain English, markdown structure and generic vulnerability vocabulary only.
# Anything domain-specific belongs in the private allowlist.
GENERIC_ALLOWLIST = {
    "protocol", "report", "rejection", "reason", "triage", "class", "root", "cause", "category",
    "earliest", "catchable", "gate", "fix", "ref", "total", "count", "summary", "statistics",
    "query", "index", "full", "database", "schema", "the", "and", "for", "not", "with",
    "without", "that", "this", "from", "into", "over", "under", "above", "below", "same", "other",
    "both", "all", "any", "each", "one", "two", "three", "first", "second", "third", "new", "old",
    "prior", "after", "before", "during", "when", "where", "which", "who", "why", "how",
    "duplicate", "duplicates", "informative", "informational", "invalid", "valid", "spam", "held",
    "applicable", "scope", "design", "issue", "intentional", "known", "previously", "reported",
    "resolved", "accepted", "acknowledged", "documented", "privileged", "role", "admin", "owner",
    "input", "validation", "best", "practice", "tradeoff", "visibility", "floor", "severity",
    "payout", "reward", "threshold", "ceiling", "tier", "unpaid", "paid", "eligible", "bounty",
    "program", "reentrancy", "overflow", "underflow", "oracle", "price", "staleness", "stale",
    "front", "run", "running", "race", "griefing", "dos", "denial", "service", "freeze", "frozen",
    "lock", "locked", "fund", "funds", "loss", "theft", "drain", "accounting", "rounding",
    "precision", "dilution", "replay", "signature", "nonce", "callback", "storage", "upgrade",
    "proxy", "governance", "vault", "pool", "lending", "borrow", "collateral", "liquidation",
    "staking", "bridge", "swap", "token", "attacker", "attack", "exploit", "unprivileged",
    "external", "internal", "public", "private", "caller", "recipient", "amount", "balance",
    "invariant", "assertion", "panic", "revert", "abort", "state", "config", "configuration",
    "parameter", "bound", "bounds", "check", "guard", "missing", "unchecked", "hardcoded",
    "constant", "fee", "fees", "rate", "ratio", "value", "supply", "mint", "burn", "transfer",
    "approve", "allowance", "deadline", "timestamp", "queue", "pending", "claim", "withdraw",
    "deposit", "redeem", "repay", "poc", "oos", "cei", "api", "json", "yaml", "csv", "url", "id",
    "realism", "concession", "reachability", "headline", "evidence", "mismatch", "self", "harm",
    "inflicted", "net", "positive", "economics", "cooperating", "party", "permanent", "critical",
    "high", "medium", "low", "info", "khomdev",
}

MIN_TOKEN_LEN = 3
NGRAM = 5
STOPWORDS = {
    "a", "an", "the", "of", "to", "in", "on", "at", "is", "it", "as", "or", "be", "by", "and",
    "for", "not", "no", "with", "that", "this", "are", "was", "were", "has", "have", "had", "but",
    "if", "then", "than", "so", "from", "into", "its", "can", "only", "any", "all", "one", "there",
}


def load_yaml(path: Path):
    try:
        import yaml  # noqa: PLC0415
    except ImportError:
        raise SystemExit("ERROR: PyYAML required.  pip install pyyaml")
    return yaml.safe_load(path.read_text(encoding="utf-8")) or {}


def load_blocklist(path: Path) -> set[str]:
    data = load_yaml(path)
    tokens: set[str] = set()
    for group, values in data.items():
        if group in DECLARATION_KEYS or not isinstance(values, list):
            continue
        tokens |= {v.strip().lower() for v in values if isinstance(v, str) and v.strip()}
    return tokens


def load_allowlist(path: Path | None) -> tuple[set[str], list[str]]:
    """(generic words, allowed phrases). Phrases are boilerplate - a vocabulary
    label, a standard exclusion clause - that both the private log and the public
    docs legitimately contain."""
    words = set(GENERIC_ALLOWLIST)
    phrases: list[str] = []
    if path:
        data = load_yaml(path)
        words |= {str(w).strip().lower() for w in (data.get("generic") or []) if str(w).strip()}
        phrases = [" ".join(re.findall(r"[a-z0-9][a-z0-9'_-]*", str(p).lower())) for p in (data.get("phrases") or [])]
    return words, phrases


def table_cells(source: Path) -> list[list[str]]:
    rows = []
    in_table = False
    for line in source.read_text(encoding="utf-8").splitlines():
        if line.startswith("## Full Database"):
            in_table = True
            continue
        if in_table and line.startswith("## "):
            break
        if not in_table or not line.startswith("|"):
            continue
        if re.match(r"^\|\s*-", line) or re.match(r"^\|\s*(Protocol|Report)\b", line):
            continue
        rows.append([c.strip() for c in re.split(r"(?<!\\)\|", line)[1:-1]])
    return rows


# ALL-CAPS words are harvested for blocklist coverage only: in the private log they
# are mostly emphasis (README, CURRENT), so in --scan-repo they would be noise.
ALLCAPS_RE = re.compile(r"\b[A-Z][A-Z0-9]{2,}\b")
IDENTIFIER_RES = (
    re.compile(r"\b[A-Z][a-z]+[A-Z][A-Za-z0-9]*\b"),  # UpperCamel
    re.compile(r"\b[a-z]+[A-Z][A-Za-z0-9]*\b"),  # lowerCamel
    re.compile(r"\b[A-Za-z][A-Za-z0-9]*_[A-Za-z0-9_]+\b"),  # snake_case
    re.compile(r"\b[A-Z][A-Z0-9]{3,}-[A-Z]?\d+\b"),  # finding ids
)


def identifier_candidates(text: str, allcaps: bool = True) -> Counter[str]:
    found: Counter[str] = Counter()
    for rx in (ALLCAPS_RE, *IDENTIFIER_RES) if allcaps else IDENTIFIER_RES:
        for token in rx.findall(text):
            found[token.lower()] += 1
    return found


def extract_candidates(source: Path) -> Counter[str]:
    text = source.read_text(encoding="utf-8")
    candidates = identifier_candidates(text)
    for cols in table_cells(source):
        if cols:
            candidates[cols[0].strip().lower()] += 1
    return candidates


def words(text: str) -> list[str]:
    return re.findall(r"[a-z0-9][a-z0-9'_-]*", text.lower())


def ngrams(tokens: list[str], n: int = NGRAM) -> set[tuple[str, ...]]:
    out = set()
    for i in range(len(tokens) - n + 1):
        gram = tuple(tokens[i : i + n])
        if sum(1 for w in gram if w not in STOPWORDS) >= 3:
            out.add(gram)
    return out


def tracked_files(repo: Path) -> list[Path]:
    out = subprocess.run(["git", "ls-files"], cwd=repo, capture_output=True, text=True, check=True).stdout
    return [repo / line for line in out.splitlines() if line.strip()]


def scan_repo(source: Path, allow: set[str], phrases: list[str], repo: Path = ROOT) -> list[str]:
    cells = table_cells(source)
    private_text = "\n".join(" ".join(c[:7]) for c in cells)
    identifiers = {
        t for t in identifier_candidates(private_text, allcaps=False) if len(t) >= MIN_TOKEN_LEN and t not in allow
    }
    # Protocol names: some are ordinary English words, which the private allowlist
    # can clear deliberately, one by one.
    identifiers |= {c[0].strip().lower() for c in cells if c and c[0].strip() and c[0].strip().lower() not in allow}
    private_grams = set()
    for c in cells:
        for cell in c[1:7]:  # free-text cells; the protocol cell is covered above
            private_grams |= ngrams(words(cell))

    problems: list[str] = []
    for path in tracked_files(repo):
        name = path.relative_to(repo).as_posix()
        if name.startswith("data/rejections") or name == "data/summary.json":
            continue  # vocabulary values only; validated by the build
        try:
            text = path.read_text(encoding="utf-8")
        except (UnicodeDecodeError, OSError):
            continue
        low = text.lower()
        for token in sorted(identifiers):
            if re.search(rf"(?<![a-z0-9_]){re.escape(token)}(?![a-z0-9_])", low):
                problems.append(f"{name}: private identifier {token!r}")
        for gram in sorted(ngrams(words(text)) & private_grams):
            joined = " ".join(gram)
            if any(joined in phrase for phrase in phrases):
                continue
            problems.append(f"{name}: copies private text {' '.join(gram)!r}")
    return problems


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--source", type=Path, required=True)
    ap.add_argument("--blocklist", type=Path)
    ap.add_argument("--allowlist", type=Path, help="private YAML with a `generic:` list")
    ap.add_argument("--scan-repo", action="store_true", help="cross-check tracked files against the source")
    ap.add_argument("--repo", type=Path, default=ROOT, help="the checkout to cross-check (default: this one)")
    ap.add_argument("--show-all", action="store_true", help="list every finding, not just the first 60")
    args = ap.parse_args(argv)
    allow, phrases = load_allowlist(args.allowlist)
    limit = None if args.show_all else 60

    if args.scan_repo:
        problems = scan_repo(args.source, allow, phrases, args.repo.resolve())
        if not problems:
            print("PASS: no private identifier and no copied private text in any tracked file.")
            return 0
        print(f"FAIL: {len(problems)} finding(s) - private text in a public file.\n")
        for p in problems[:limit]:
            print(f"  {p}")
        return 1

    if not args.blocklist:
        ap.error("--blocklist is required unless --scan-repo is given")
    blocked = load_blocklist(args.blocklist)
    candidates = extract_candidates(args.source)
    print(f"blocklist tokens : {len(blocked)}")
    print(f"source candidates: {len(candidates)}\n")
    uncovered = [
        (t, n)
        for t, n in candidates.most_common()
        if len(t) >= MIN_TOKEN_LEN
        and t not in blocked
        and t not in allow
        and not any(b in t for b in blocked if len(b) >= 4)
    ]
    if not uncovered:
        print("PASS: every candidate token is covered by the blocklist or an allowlist.")
        return 0
    print(f"REVIEW: {len(uncovered)} uncovered token(s) - verify each is not a target name.\n")
    for token, count in uncovered[:limit]:
        print(f"  {token:<45} x{count}")
    if limit and len(uncovered) > limit:
        print(f"  ... and {len(uncovered) - limit} more (use --show-all)")
    print("\nFor each: add it to the blocklist, or to the private allowlist if it is generic.")
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
