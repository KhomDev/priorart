#!/usr/bin/env python3
"""Validate a private program ledger against the shipped schema and taxonomies.

The ledger is YOUR transcription of bug-bounty program terms, and it is private:
entries name real programs. This validator checks the ledger against
`schemas/ledger.schema.json` (unknown keys, types, formats), then applies the
rules a schema cannot express - most importantly, that every entry is CITED. An
uncited entry is a guess, and a guessed payout floor is worse than none because
you will act on it.

The vocabularies come from `data/tier-shapes.yaml` and `data/clause-classes.yaml`,
so this script and the docs cannot drift apart.

Usage:
    python scripts/validate.py examples/ledger.example.yaml
    python scripts/validate.py my-ledger.yaml --strict
    python scripts/validate.py my-ledger.yaml --format json
    python scripts/validate.py --taxonomy          # print the vocabularies

Exit codes:
    0  valid
    1  validation errors
    2  usage / parse error
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import re
import sys
from pathlib import Path
from urllib.parse import urlparse

try:
    import yaml
except ImportError:
    print("ERROR: PyYAML required.  pip install pyyaml", file=sys.stderr)
    raise SystemExit(2) from None

try:
    import jsonschema
except ImportError:
    print("ERROR: jsonschema required.  pip install jsonschema", file=sys.stderr)
    raise SystemExit(2) from None


def _data_root() -> Path:
    """Where data/ and schemas/ live: the repository, or the installed package.

    In a checkout this file is scripts/validate.py and the data sits beside
    scripts/. Installed, it is bounty_economics/validate.py and the data is
    packaged as bounty_economics/data and bounty_economics/schemas.
    """
    here = Path(__file__).resolve().parent
    for root in (here.parent, here):
        if (root / "data" / "tier-shapes.yaml").exists():
            return root
    return here.parent


ROOT = _data_root()
TIER_FILE = ROOT / "data" / "tier-shapes.yaml"
CLAUSE_FILE = ROOT / "data" / "clause-classes.yaml"
SCHEMA_FILE = ROOT / "schemas" / "ledger.schema.json"

# The ledger format version this validator implements. An entry may pin it with
# `schema_version`; an entry that pins a version this script does not implement
# is rejected rather than silently mis-read. See CHANGELOG.md for migrations.
SCHEMA_VERSION = 1

FLOOR_SEVERITIES = {"critical", "high", "medium", "low", None}

# Whether the FLOOR itself was read off the page. This is the field that stops
# "the page says nothing about a floor" being transcribed as "Low pays".
#
#   documented  — the reward table was read and its lowest paying tier recorded,
#                 or the page explicitly states "SEVERITY FLOOR: NONE / any"
#   unstated    — the page says nothing about a floor
#   conflicting — the program's own materials disagree about the floor
FLOOR_STATUSES = ("documented", "unstated", "conflicting")

SLUG_RE = re.compile(r"^[a-z0-9][a-z0-9._-]*$")
DATE_RE = re.compile(r"^\d{4}-\d{2}-\d{2}$")

# Placeholder hosts: a source_url on one of these was copied without editing.
# Matched against the URL's HOST, never as a substring of the whole URL - a real
# repository called "todo-list" is not a placeholder.
PLACEHOLDER_HOSTS = ("example.com", "www.example.com", "localhost", "127.0.0.1")
PLACEHOLDER_SUFFIXES = (".invalid", ".localhost")
PLACEHOLDER_LABELS = ("todo", "tbd", "changeme")

STALE_DAYS = 180

# Constructs Python's `re` accepts but viability-gate's Rust `regex` crate does
# not. A ledger is read by both tools, so a pattern must be in their common
# subset: one that compiles here and fails there would make vg's G2 fail.
UNSUPPORTED_REGEX = (
    (re.compile(r"\(\?<?[=!]"), "lookaround"),
    (re.compile(r"(?<!\\)\\[1-9]"), "a numbered backreference"),
    (re.compile(r"\(\?P=|\\k<"), "a named backreference"),
    (re.compile(r"\(\?>"), "an atomic group"),
    (re.compile(r"[*+?}][+]"), "a possessive quantifier"),
)

# Schema errors the semantic checks below already report with a better
# message. Everything else the schema finds (unknown keys, wrong types, bad
# formats) is reported as it comes.
_SCHEMA_COVERED = {"required", "enum", "const", "pattern", "minLength"}


def placeholder_host(url: str) -> str | None:
    """The placeholder host in url, or None."""
    host = (urlparse(url).hostname or "").lower().rstrip(".")
    if not host:
        return None
    if host in PLACEHOLDER_HOSTS or host.endswith(PLACEHOLDER_SUFFIXES):
        return host
    if any(label in PLACEHOLDER_LABELS for label in host.split(".")):
        return host
    return None


def regex_problem(pattern: str) -> str | None:
    """Why pattern is unusable by both tools, or None."""
    for probe, name in UNSUPPORTED_REGEX:
        if probe.search(pattern):
            return (
                f"uses {name}, which viability-gate's Rust regex does not support "
                f"(no lookaround, no backreferences)"
            )
    try:
        re.compile(pattern, re.IGNORECASE)
    except re.error as err:
        return f"invalid regex: {err}"
    return None


def load_taxonomies() -> tuple[dict[str, dict], dict[str, dict]]:
    """Return (tier_shapes, clause_classes), keyed by id."""
    if not TIER_FILE.exists() or not CLAUSE_FILE.exists():
        print(
            f"ERROR: taxonomy files missing ({TIER_FILE}, {CLAUSE_FILE})",
            file=sys.stderr,
        )
        raise SystemExit(2)
    tiers = yaml.safe_load(TIER_FILE.read_text(encoding="utf-8")) or {}
    clauses = yaml.safe_load(CLAUSE_FILE.read_text(encoding="utf-8")) or {}
    tier_map = {s["id"]: s for s in tiers.get("shapes", [])}
    clause_map = {c["id"]: c for c in clauses.get("classes", [])}
    return tier_map, clause_map


_ENTRY_VALIDATOR: jsonschema.protocols.Validator | None = None


def entry_validator() -> jsonschema.protocols.Validator:
    """A validator for ONE ledger entry: the schema's `items`."""
    global _ENTRY_VALIDATOR
    if _ENTRY_VALIDATOR is None:
        schema = json.loads(SCHEMA_FILE.read_text(encoding="utf-8"))
        item = dict(schema["items"])
        item["$schema"] = schema["$schema"]
        cls = jsonschema.validators.validator_for(item)
        _ENTRY_VALIDATOR = cls(item, format_checker=cls.FORMAT_CHECKER)
    return _ENTRY_VALIDATOR


def schema_errors(entry: dict, where: str) -> list[str]:
    out = []
    for err in sorted(entry_validator().iter_errors(entry), key=lambda e: list(e.absolute_path)):
        if err.validator in _SCHEMA_COVERED:
            continue
        path = ".".join(str(p) for p in err.absolute_path) or "(entry)"
        out.append(f"{where}: schema: {path}: {err.message}")
    return out


def validate_entry(
    entry: dict,
    index: int,
    tier_map: dict[str, dict],
    clause_map: dict[str, dict],
    example_mode: bool = False,
) -> tuple[list[str], list[str]]:
    """Return (errors, warnings) for one ledger entry.

    `example_mode` relaxes the placeholder-host check, which would otherwise
    reject the repository's own fictional example. Files named `*.example.yaml`
    are treated as examples automatically.
    """
    errors: list[str] = []
    warnings: list[str] = []
    where = f"entry[{index}]"

    if not isinstance(entry, dict):
        return [f"{where}: not a mapping"], []

    ident = entry.get("id")
    if not ident:
        errors.append(f"{where}: missing required field 'id'")
    else:
        where = f"entry[{index}] ({ident})"
        if not SLUG_RE.match(str(ident)):
            errors.append(f"{where}: id must match {SLUG_RE.pattern}")

    # --- the schema: unknown keys, types, formats ------------------------
    #
    # An unquoted YAML date is read as a date object; it is reported as a
    # warning below and checked as its string form here, so the one mistake is
    # not reported twice.
    as_text = dict(entry)
    if isinstance(as_text.get("retrieved"), dt.date):
        as_text["retrieved"] = as_text["retrieved"].isoformat()
    errors.extend(schema_errors(as_text, where))

    # --- ledger format version -------------------------------------------
    version = entry.get("schema_version")
    if version is not None and version != SCHEMA_VERSION:
        errors.append(
            f"{where}: schema_version {version!r} is not implemented by this "
            f"validator (it implements {SCHEMA_VERSION}). Update the tool, or "
            f"follow the migration notes in CHANGELOG.md."
        )

    # --- tier shape ------------------------------------------------------
    shape = entry.get("tier_shape")
    spec: dict | None = None
    if shape is None:
        errors.append(f"{where}: missing required field 'tier_shape'")
    elif shape == "any":
        # Removed from the taxonomy. Give the migration rather than a bare
        # "not in enum", because the fix depends on what the page says.
        errors.append(
            f"{where}: tier_shape 'any' was removed. If the page explicitly states "
            f"'SEVERITY FLOOR: NONE' or 'any', use 'low_and_above' with "
            f"floor_status 'documented' - informational findings are acknowledged "
            f"rather than paid, so Low is the lowest tier that pays. If the page "
            f"says nothing about a floor, use 'unknown' with floor_status 'unstated'."
        )
    elif shape not in tier_map:
        errors.append(f"{where}: tier_shape {shape!r} not in {sorted(tier_map)}")
    else:
        spec = tier_map[shape]
        if shape == "unknown":
            warnings.append(
                f"{where}: tier_shape is 'unknown' — this is NOT permissive. "
                f"Hunting an untranscribed program is an unhedged bet."
            )
        elif len(spec.get("pays") or []) <= 2:
            warnings.append(
                f"{where}: tier_shape '{shape}' pays only "
                f"{'/'.join(spec.get('pays') or []) or 'nothing'}. "
                f"Confirm your thesis can reach it before hunting."
            )

    # --- floor consistency ----------------------------------------------
    floor = entry.get("floor_severity")
    if floor == "informational":
        errors.append(
            f"{where}: floor_severity 'informational' is not a real floor — no "
            f"program pays informational in practice. If the page explicitly states "
            f"no floor, record 'low' with floor_status 'documented'; if it says "
            f"nothing about a floor, leave floor_severity null with floor_status "
            f"'unstated'."
        )
    elif floor is not None and floor not in FLOOR_SEVERITIES:
        allowed = sorted(s for s in FLOOR_SEVERITIES if s)
        errors.append(f"{where}: floor_severity {floor!r} not in {allowed}")

    if spec:
        expected = spec.get("floor")
        if expected and floor and floor != expected:
            warnings.append(
                f"{where}: tier_shape '{shape}' implies floor '{expected}' "
                f"but floor_severity is '{floor}' — check which is right."
            )
        if shape != "unknown" and not floor:
            warnings.append(f"{where}: tier_shape is set but floor_severity is empty.")

    # --- floor_status: was the floor READ, or merely assumed? -------------
    #
    # The defect this field exists to prevent: transcribing "the page says
    # nothing about a floor" as "Low pays". An unstated floor is not evidence
    # about the program at all — it is evidence the page has not been read. So
    # an unstated floor must keep tier_shape at 'unknown'.
    status = entry.get("floor_status")
    if status is None:
        warnings.append(
            f"{where}: floor_status is missing. Record whether the floor was "
            f"'documented', 'unstated' or 'conflicting'. Without it the validator "
            f"cannot tell a floor you read from one you never looked for, and "
            f"viability-gate's G1 fails the entry."
        )
    elif status not in FLOOR_STATUSES:
        errors.append(
            f"{where}: floor_status {status!r} not in {list(FLOOR_STATUSES)}"
        )
    elif status == "documented":
        if not floor:
            errors.append(
                f"{where}: floor_status is 'documented' but floor_severity is "
                f"empty. Record the lowest severity with a non-zero reward."
            )
        if shape == "unknown":
            errors.append(
                f"{where}: floor_status is 'documented' but tier_shape is "
                f"'unknown' — 'unknown' means the page has not been read. Record "
                f"the shape you read, or set floor_status to 'unstated'."
            )
    elif status == "unstated":
        if floor:
            errors.append(
                f"{where}: floor_status is 'unstated' but floor_severity is "
                f"{floor!r}. An unstated floor has no severity: leave "
                f"floor_severity null and tier_shape 'unknown'."
            )
        if shape is not None and shape != "unknown":
            errors.append(
                f"{where}: floor_status is 'unstated' but tier_shape is "
                f"{shape!r}. An unstated floor is not evidence that Low pays; it "
                f"is evidence the page has not been read. Keep tier_shape "
                f"'unknown' until you find the reward table."
            )
        else:
            warnings.append(
                f"{where}: floor_status is 'unstated' — the floor is not "
                f"evidence-backed. Do not read it as 'Low pays'. Open the rewards "
                f"page before spending hours here."
            )
    else:  # conflicting
        if shape is not None and shape != "unknown":
            errors.append(
                f"{where}: floor_status is 'conflicting' but tier_shape is "
                f"{shape!r} — the program's materials disagree, so the shape is "
                f"not settled. Keep tier_shape 'unknown' until you resolve it."
            )
        warnings.append(
            f"{where}: the program's own materials disagree about the floor. "
            f"Record both readings in 'conflicts' rather than resolving it "
            f"silently."
        )

    # --- citation (the important part) -----------------------------------
    url = entry.get("source_url")
    if not url:
        errors.append(
            f"{where}: missing 'source_url'. An uncited entry is a guess, "
            f"and a guessed payout floor is worse than none."
        )
    else:
        if not str(url).startswith(("http://", "https://")):
            errors.append(f"{where}: source_url must be http(s)")
        host = placeholder_host(str(url))
        if host:
            if example_mode:
                warnings.append(
                    f"{where}: source_url uses a placeholder host ({host}) — "
                    f"expected in an example file, not in a real ledger."
                )
            else:
                errors.append(
                    f"{where}: source_url still points at a placeholder host "
                    f"({host}). Replace it with the real page."
                )

    retrieved = entry.get("retrieved")
    if not retrieved:
        errors.append(f"{where}: missing 'retrieved' date.")
    else:
        text = retrieved.isoformat() if isinstance(retrieved, dt.date) else str(retrieved)
        if not isinstance(retrieved, str):
            # YAML turns an unquoted 2026-10-01 into a date object. The ledger
            # format and the JSON Schema say string.
            warnings.append(
                f"{where}: retrieved was written unquoted, so YAML read it as "
                f"{type(retrieved).__name__}, not a string. The schema declares a "
                f"string; quote it ({text!r}) so every consumer agrees."
            )
        if not DATE_RE.match(text):
            errors.append(f"{where}: retrieved must be YYYY-MM-DD, got {text!r}")
        else:
            try:
                age = (dt.date.today() - dt.date.fromisoformat(text)).days
            except ValueError:
                errors.append(f"{where}: retrieved {text!r} is not a valid date")
            else:
                if age < 0:
                    errors.append(
                        f"{where}: retrieved {text} is in the future. Record the "
                        f"date you actually read the page."
                    )
                elif age > STALE_DAYS:
                    warnings.append(
                        f"{where}: retrieved {age} days ago. Program terms change "
                        f"without notice — re-read the page before relying on this."
                    )

    # --- exclusions -------------------------------------------------------
    exclusions = entry.get("exclusions") or []
    if not isinstance(exclusions, list):
        errors.append(f"{where}: exclusions must be a list")
        exclusions = []
    for j, exc in enumerate(exclusions):
        if not isinstance(exc, dict):
            errors.append(f"{where}: exclusions[{j}] is not a mapping")
            continue
        klass = exc.get("class")
        if klass not in clause_map:
            errors.append(
                f"{where}: exclusions[{j}].class {klass!r} not in {sorted(clause_map)}"
            )
        if not exc.get("clause"):
            errors.append(f"{where}: exclusions[{j}] missing verbatim 'clause'")
        pattern = exc.get("pattern")
        if pattern:
            problem = regex_problem(str(pattern))
            if problem:
                errors.append(f"{where}: exclusions[{j}].pattern {problem}")

    # --- prior audits -----------------------------------------------------
    audits = entry.get("prior_audits") or []
    if not isinstance(audits, list):
        errors.append(f"{where}: prior_audits must be a list")
        audits = []
    for j, audit in enumerate(audits):
        if not isinstance(audit, dict):
            errors.append(f"{where}: prior_audits[{j}] is not a mapping")
            continue
        if not audit.get("firm"):
            errors.append(f"{where}: prior_audits[{j}] missing 'firm'")
        if not audit.get("url"):
            warnings.append(
                f"{where}: prior_audits[{j}] has no 'url'. Cite the report, as for "
                f"the floor."
            )
        if not audit.get("covers"):
            warnings.append(
                f"{where}: prior_audits[{j}] has no 'covers' list. A report naming "
                f"a file is not the same as covering your function."
            )

    return errors, warnings


def print_taxonomy(tier_map: dict[str, dict], clause_map: dict[str, dict]) -> None:
    print(f"tier shapes ({len(tier_map)}) — data/tier-shapes.yaml\n")
    for tid, spec in tier_map.items():
        pays = "/".join(spec.get("pays") or []) or "nothing"
        print(f"  {tid:<18} pays: {pays}")
    print(f"\nclause classes ({len(clause_map)}) — data/clause-classes.yaml\n")
    for cid, spec in clause_map.items():
        print(f"  {cid:<20} {spec.get('kills', '')}")


def taxonomy_payload(tier_map: dict[str, dict], clause_map: dict[str, dict]) -> dict:
    """The taxonomies as machine-readable JSON, for downstream tools."""
    return {
        "schema_version": SCHEMA_VERSION,
        "tier_shapes": {
            "source": "data/tier-shapes.yaml",
            "count": len(tier_map),
            "items": [dict(spec) for spec in tier_map.values()],
        },
        "clause_classes": {
            "source": "data/clause-classes.yaml",
            "count": len(clause_map),
            "items": [dict(spec) for spec in clause_map.values()],
        },
    }


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument(
        "ledger",
        nargs="*",
        type=Path,
        help="one or more ledger files; a shell glob such as examples/*.yaml works",
    )
    ap.add_argument("--strict", action="store_true", help="treat warnings as errors")
    ap.add_argument(
        "--example-mode",
        action="store_true",
        help=(
            "relax the placeholder-host check, for validating a fictional "
            "example. Auto-enabled for files named *.example.yaml."
        ),
    )
    ap.add_argument("--taxonomy", action="store_true", help="print the shipped vocabularies")
    ap.add_argument(
        "--format",
        choices=("text", "json"),
        default="text",
        help="output format; 'json' is stable enough to pipe into another tool",
    )
    args = ap.parse_args(argv)

    as_json = args.format == "json"

    def fail(message: str, code: int = 2) -> int:
        if as_json:
            print(json.dumps({"ok": False, "error": message, "exit_code": code}))
        else:
            print(f"ERROR: {message}", file=sys.stderr)
        return code

    tier_map, clause_map = load_taxonomies()

    if args.taxonomy:
        if as_json:
            print(json.dumps(taxonomy_payload(tier_map, clause_map), indent=2))
        else:
            print_taxonomy(tier_map, clause_map)
        return 0

    if not args.ledger:
        if as_json:
            return fail("a ledger path is required (or use --taxonomy)")
        ap.error("a ledger path is required (or use --taxonomy)")

    for path in args.ledger:
        if not path.exists():
            return fail(f"{path} not found")

    results: list[dict] = []
    for path in args.ledger:
        example_mode = args.example_mode or path.name.endswith(".example.yaml")

        try:
            data = yaml.safe_load(path.read_text(encoding="utf-8-sig"))
        except yaml.YAMLError as err:
            return fail(f"cannot parse YAML in {path}: {err}")

        if data is None:
            return fail(f"ledger is empty: {path}")
        if not isinstance(data, list):
            return fail(f"ledger must be a list of entries: {path}")

        errors: list[str] = []
        warnings: list[str] = []
        seen_ids: set[str] = set()

        for i, entry in enumerate(data):
            entry_errors, entry_warnings = validate_entry(
                entry, i, tier_map, clause_map, example_mode
            )
            errors.extend(entry_errors)
            warnings.extend(entry_warnings)
            if isinstance(entry, dict) and entry.get("id"):
                if entry["id"] in seen_ids:
                    errors.append(f"duplicate id: {entry['id']!r}")
                seen_ids.add(entry["id"])

        results.append(
            {
                "ledger": str(path),
                "example_mode": example_mode,
                "entries": len(data),
                "errors": errors,
                "warnings": warnings,
            }
        )

    several = len(results) > 1

    def label(path: str, message: str) -> str:
        return f"{path}: {message}" if several else message

    all_errors = [label(r["ledger"], m) for r in results for m in r["errors"]]
    all_warnings = [label(r["ledger"], m) for r in results for m in r["warnings"]]
    entries = sum(r["entries"] for r in results)

    failed = bool(all_errors) or (args.strict and bool(all_warnings))

    if as_json:
        payload = {
            "schema_version": SCHEMA_VERSION,
            "ok": not failed,
            "strict": args.strict,
            "ledgers": results,
            "entries": entries,
            "errors": all_errors,
            "warnings": all_warnings,
        }
        if not several:
            payload["ledger"] = results[0]["ledger"]
        print(json.dumps(payload, indent=2))
    else:
        for w in all_warnings:
            print(f"WARN  {w}")
        for e in all_errors:
            print(f"ERROR {e}", file=sys.stderr)

        print()
        if several:
            for r in results:
                print(
                    f"{r['ledger']}: {r['entries']} entr"
                    f"{'y' if r['entries'] == 1 else 'ies'}, "
                    f"{len(r['errors'])} error(s), {len(r['warnings'])} warning(s)"
                )
            print()
        print(
            f"{entries} entr{'y' if entries == 1 else 'ies'}, "
            f"{len(all_errors)} error(s), {len(all_warnings)} warning(s)"
        )

    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
