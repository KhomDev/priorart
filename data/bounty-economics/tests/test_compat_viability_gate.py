#!/usr/bin/env python3
"""Cross-repo compatibility: the ledger contract between this repo and viability-gate.

Two repositories read the same private ledger file:

  * **bounty-economics** (this repo) owns the schema: ``schemas/ledger.schema.json``,
    plus the validator and the taxonomies.
  * **viability-gate** (``../viability-gate``) owns the CLI. Its ``vg init`` scaffolds a
    ledger, and its ``LedgerEntry`` model deserializes one.

Nothing enforces that those two agree except this file. The contract is checked at
a **pinned tag** of the sibling, so a change over there cannot silently break the
format here without failing a build:

  * forward — ``vg init`` output, once filled in, must satisfy this repo's schema.
  * reverse — a ledger this repo accepts must be readable by ``vg``, and read the
    way we mean it: an unstated floor fails vg's G1, every pattern we accept
    compiles in vg's G2, and vg's G2 runs our patterns over a draft.

Locally it SKIPS, with a specific reason, when the sibling checkout is absent, when
the pinned tag is missing, or when ``git`` / ``cargo`` is unavailable: a sibling
that is merely absent from a contributor's machine is not a defect in this repo.
With ``BE_COMPAT_REQUIRED=1`` (set in CI) every one of those reasons FAILS
instead, so a broken pin can never pass silently.

The pin lives in exactly one place: ``PINNED_VG_TAG`` below.

Run:
    python -m pytest tests/test_compat_viability_gate.py -q
"""

from __future__ import annotations

import json
import os
import re
import shutil
import subprocess
import sys
import tarfile
from pathlib import Path

import pytest
import yaml

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "scripts"))

from validate import (  # noqa: E402
    SCHEMA_FILE,
    load_taxonomies,
    validate_entry,
)

# ---------------------------------------------------------------- the pin

# The single source of truth for the cross-repo pin. Bump it when viability-gate
# publishes a tag that changes the ledger format or `vg init`.
PINNED_VG_TAG = "v0.5.0"

# The sibling checkout, relative to this repo. Both live under the same parent.
# VG_SIBLING overrides it, for a checkout kept under a different folder name.
SIBLING = Path(os.environ.get("VG_SIBLING") or ROOT.parent / "viability-gate").resolve()

TIERS, CLAUSES = load_taxonomies()
SCHEMA = json.loads(SCHEMA_FILE.read_text(encoding="utf-8"))


# ---------------------------------------------------------------- skip logic


def _skip_reason() -> str | None:
    """Why the cross-repo half cannot run here, or None when it can."""
    if not SIBLING.is_dir():
        return f"sibling checkout not present at {SIBLING}"
    if shutil.which("git") is None:
        return "git is not on PATH"
    probe = subprocess.run(
        ["git", "-C", str(SIBLING), "rev-parse", "--verify", f"refs/tags/{PINNED_VG_TAG}^{{commit}}"],
        capture_output=True,
        text=True,
    )
    if probe.returncode != 0:
        return f"pinned tag {PINNED_VG_TAG} does not exist in {SIBLING}"
    if shutil.which("cargo") is None:
        return "cargo is not on PATH, so vg cannot be built"
    return None


# CI sets this, so the contract is enforced there rather than skipped.
REQUIRED = os.environ.get("BE_COMPAT_REQUIRED") == "1"

requires_sibling = pytest.mark.skipif(
    not REQUIRED and _skip_reason() is not None, reason=_skip_reason() or ""
)


@pytest.fixture(scope="module")
def vg_binary(tmp_path_factory: pytest.TempPathFactory) -> Path:
    """Extract the sibling at the pinned tag, build it, and return the binary.

    ``git archive`` is used rather than ``git worktree`` so that building the pinned
    revision touches nothing inside the sibling repository - it may be under
    concurrent edit, and a test must not add state to somebody else's checkout.
    """
    reason = _skip_reason()
    if reason:
        if REQUIRED:
            pytest.fail(f"BE_COMPAT_REQUIRED=1 but the contract cannot run: {reason}")
        pytest.skip(reason)

    work = tmp_path_factory.mktemp("vg-pinned")
    archive = work / "src.tar"
    subprocess.run(
        [
            "git", "-C", str(SIBLING), "archive",
            "--format=tar", f"--output={archive}", PINNED_VG_TAG,
        ],
        check=True,
        capture_output=True,
        text=True,
    )
    src = work / "src"
    src.mkdir()
    with tarfile.open(archive) as tar:
        # Our own repository's archive, at a pinned tag. The filter kwarg exists
        # only from 3.12; CI covers 3.10 and 3.11 too.
        if sys.version_info >= (3, 12):
            tar.extractall(src, filter="data")
        else:
            tar.extractall(src)

    build = subprocess.run(
        ["cargo", "build", "--quiet", "--locked", "--manifest-path", str(src / "Cargo.toml")],
        capture_output=True,
        text=True,
    )
    if build.returncode != 0:
        pytest.fail(f"cargo build of {PINNED_VG_TAG} failed:\n{build.stderr[-4000:]}")

    exe = "vg.exe" if sys.platform == "win32" else "vg"
    binary = src / "target" / "debug" / exe
    if not binary.exists():
        pytest.fail(f"cargo build reported success but {binary} is missing")
    return binary


# ---------------------------------------------------------------- the pin itself


def test_the_pin_is_declared_exactly_once():
    # The pin must not be duplicated in a workflow file or a second test: two
    # copies drift, and then nobody knows which revision was actually checked.
    skip_dirs = (".git", ".venv", "venv", "site-packages", "__pycache__", "target", ".pytest_cache")
    holders = [
        p
        for p in ROOT.rglob("*.py")
        if not any(part in skip_dirs for part in p.parts)
        and "PINNED_VG_TAG" in p.read_text(encoding="utf-8", errors="ignore")
    ]
    assert holders == [Path(__file__).resolve()], (
        f"PINNED_VG_TAG must live only in this file, found in {holders}"
    )


def test_the_pin_looks_like_a_tag():
    assert re.match(r"^v\d+\.\d+\.\d+", PINNED_VG_TAG), PINNED_VG_TAG


def test_ci_requires_the_contract_rather_than_skipping_it():
    # A skip in CI is a silent pass. The compat job must set the switch that
    # turns every skip reason into a failure.
    ci = yaml.safe_load((ROOT / ".github" / "workflows" / "ci.yml").read_text(encoding="utf-8"))
    env = ci["jobs"]["compat"].get("env") or {}
    assert str(env.get("BE_COMPAT_REQUIRED")) == "1", env


# ---------------------------------------------------------------- in-repo half
#
# This half never skips. It is the part of the contract that lives in this repo:
# the examples, the schema and the taxonomies must agree with each other.


def test_our_examples_pass_our_validator():
    # validate_entry checks the JSON Schema as well as the semantic rules.
    for path in sorted((ROOT / "examples").glob("*.yaml")):
        for i, entry in enumerate(yaml.safe_load(path.read_text(encoding="utf-8"))):
            errors, _ = validate_entry(entry, i, TIERS, CLAUSES, example_mode=True)
            assert errors == [], f"{path.name}: {errors}"


def test_our_examples_satisfy_the_json_schema_directly():
    import jsonschema

    for path in sorted((ROOT / "examples").glob("*.yaml")):
        jsonschema.validate(
            instance=yaml.safe_load(path.read_text(encoding="utf-8")), schema=SCHEMA
        )


def test_our_schema_is_closed_and_versioned():
    assert SCHEMA["items"]["additionalProperties"] is False
    assert SCHEMA["items"]["properties"]["schema_version"]["const"] == 1


# ---------------------------------------------------------------- forward


@requires_sibling
def test_vg_init_ledger_satisfies_our_schema(vg_binary: Path, tmp_path: Path):
    """The scaffold viability-gate generates must be writable into our schema."""
    target = tmp_path / "target"
    out = subprocess.run(
        [str(vg_binary), "init", str(target)],
        capture_output=True,
        text=True,
    )
    assert out.returncode == 0, f"vg init failed:\n{out.stdout}\n{out.stderr}"

    scaffold = yaml.safe_load((target / "ledger.yaml").read_text(encoding="utf-8"))
    assert isinstance(scaffold, list), "vg init must emit a LIST of entries, as our schema requires"
    assert scaffold, "vg init emitted an empty ledger"
    assert "REPLACE-ME" in str(scaffold[0].get("id", "")), (
        "the scaffold is expected to be unfilled; this test asserts its SHAPE"
    )

    # The unfilled scaffold is INTENDED to be invalid - an empty source_url is an
    # uncited floor. So the contract is checked on the fields, not on the
    # placeholder values: fill them the way a researcher would and require zero
    # errors against this repo's validator.
    filled = dict(scaffold[0])
    filled.update(
        id="cross-repo-scaffold",
        tier_shape="medium_and_above",
        floor_status="documented",
        floor_severity="medium",
        source_url="https://real.example.org/rewards",
        retrieved="2026-10-01",
    )
    errors, _ = validate_entry(filled, 0, TIERS, CLAUSES, example_mode=False)
    assert errors == [], f"a filled vg init ledger does not satisfy our schema: {errors}"


@requires_sibling
def test_vg_init_offers_no_shape_or_class_we_do_not_have(vg_binary: Path, tmp_path: Path):
    """Every vocabulary value the scaffold offers must exist in our taxonomies.

    This is the direction that catches a real divergence: viability-gate adding a
    tier shape, or re-offering the removed ``any``, that this repo's schema would
    reject.
    """
    target = tmp_path / "target"
    subprocess.run([str(vg_binary), "init", str(target)], check=True, capture_output=True, text=True)
    template = (target / "ledger.yaml").read_text(encoding="utf-8")

    assert "| any |" not in template, "vg init still offers the removed `any` tier shape"
    for shape in re.findall(r"\b(critical_only|high_and_above|medium_and_above|low_and_above|unknown)\b", template):
        assert shape in TIERS, f"vg init offers {shape!r}, which is not in our taxonomy"
    # The scaffold lists its clause classes as prose; each must be one of ours.
    listed = set(re.findall(r"\b([a-z]+_[a-z_]+)\b", template))
    unknown = {
        k
        for k in listed
        if k not in CLAUSES and k not in TIERS and k not in {"floor_severity", "source_url", "prior_audits", "post_audit_delta", "known_issues", "tier_shape", "schema_version", "floor_status"}
    }
    assert not unknown, f"vg init names vocabulary we do not ship: {sorted(unknown)}"


# ---------------------------------------------------------------- reverse


@requires_sibling
def test_our_example_ledger_is_readable_by_vg(vg_binary: Path, tmp_path: Path):
    """The reverse direction: what we accept, vg must be able to parse.

    ``vg gates`` exits 2 on a parse failure and otherwise reports gate status, so
    exit code 2 is the signal. Any other code means the ledger was read.
    """
    example = ROOT / "examples" / "tier-shapes.example.yaml"
    scope = tmp_path / "scope.txt"
    scope.write_text("# empty on purpose: only the ledger parse is under test\n", encoding="utf-8")

    out = subprocess.run(
        [str(vg_binary), "gates", "--ledger", str(example), "--scope", str(scope)],
        capture_output=True,
        text=True,
    )
    assert out.returncode != 2, (
        f"vg could not parse our shipped example ledger (exit 2):\n{out.stderr}"
    )
    assert "vg:" not in out.stderr, f"vg reported an input error:\n{out.stderr}"


@requires_sibling
def test_vg_ignores_our_additive_fields_without_failing(vg_binary: Path, tmp_path: Path):
    """Fields vg does not model must not break it.

    ``schema_version`` and ``notes`` are fields viability-gate's model does not read
    (it does read ``floor_status``). The contract is that new fields are additive:
    a reader must ignore them, not reject them. A ledger that fails here means the
    two repos have diverged.
    """
    ledger = tmp_path / "additive.yaml"
    ledger.write_text(
        "- id: additive-fields\n"
        "  schema_version: 1\n"
        "  tier_shape: medium_and_above\n"
        "  floor_status: documented\n"
        "  floor_severity: medium\n"
        "  source_url: https://real.example.org/rewards\n"
        "  retrieved: 2026-10-01\n"
        "  notes: \"a free-text note\"\n",
        encoding="utf-8",
    )
    out = subprocess.run(
        [str(vg_binary), "gates", "--ledger", str(ledger)],
        capture_output=True,
        text=True,
    )
    assert out.returncode != 2, (
        f"vg rejected a ledger carrying our additive fields:\n{out.stderr}"
    )


# ---------------------------------------------------------------- semantics
#
# Parsing is not agreement. These check that vg reads a ledger the way this
# repository means it.


def _gates(vg_binary: Path, *args: str) -> dict[str, dict]:
    out = subprocess.run(
        [str(vg_binary), "gates", *args, "--json"], capture_output=True, text=True
    )
    assert out.returncode in (0, 1), f"vg gates failed:\n{out.stderr}"
    return {g["gate"]: g for g in json.loads(out.stdout)}


def _one_entry_ledger(tmp_path: Path, body: str) -> Path:
    ledger = tmp_path / "ledger.yaml"
    ledger.write_text(body, encoding="utf-8")
    return ledger


@requires_sibling
def test_vg_fails_an_unstated_floor(vg_binary: Path, tmp_path: Path):
    # The rule this repository exists to enforce: an unstated floor is not Low.
    errors, _ = validate_entry(
        {
            "id": "unstated",
            "tier_shape": "unknown",
            "floor_status": "unstated",
            "floor_severity": None,
            "source_url": "https://real.example.org/rewards",
            "retrieved": "2026-10-01",
        },
        0,
        TIERS,
        CLAUSES,
    )
    assert errors == [], errors
    ledger = _one_entry_ledger(
        tmp_path,
        "- id: unstated\n  tier_shape: unknown\n  floor_status: unstated\n"
        "  floor_severity: null\n  source_url: \"https://real.example.org/rewards\"\n"
        "  retrieved: \"2026-10-01\"\n",
    )
    g1 = _gates(vg_binary, "--ledger", str(ledger))["G1"]
    assert g1["status"] == "fail", g1


@requires_sibling
def test_every_pattern_we_ship_compiles_in_vg(vg_binary: Path, tmp_path: Path):
    # The example ledger's patterns and every suggested_pattern in the clause
    # taxonomy: validate.py accepts them, so vg's G2 must compile them too.
    lines = [
        "- id: patterns",
        "  tier_shape: medium_and_above",
        "  floor_status: documented",
        "  floor_severity: medium",
        "  source_url: \"https://real.example.org/rewards\"",
        "  retrieved: \"2026-10-01\"",
        "  exclusions:",
    ]
    example = yaml.safe_load((ROOT / "examples" / "ledger.example.yaml").read_text(encoding="utf-8"))
    patterns = [(x["class"], x["pattern"]) for x in example[0]["exclusions"] if x.get("pattern")]
    patterns += [(cid, c["suggested_pattern"]) for cid, c in CLAUSES.items() if c.get("suggested_pattern")]
    for klass, pattern in patterns:
        lines += [
            f"    - class: {klass}",
            "      clause: \"verbatim clause\"",
            f"      pattern: {json.dumps(pattern)}",
        ]
    ledger = _one_entry_ledger(tmp_path, "\n".join(lines) + "\n")
    g2 = _gates(vg_binary, "--ledger", str(ledger))["G2"]
    assert g2["status"] == "pass", g2


@requires_sibling
def test_vg_runs_our_exclusion_patterns_over_a_draft(vg_binary: Path, tmp_path: Path):
    ledger = _one_entry_ledger(
        tmp_path,
        "- id: g2\n  tier_shape: medium_and_above\n  floor_status: documented\n"
        "  floor_severity: medium\n  source_url: \"https://real.example.org/rewards\"\n"
        "  retrieved: \"2026-10-01\"\n  exclusions:\n    - class: privileged_role\n"
        "      clause: \"Attacks requiring privileged access\"\n"
        "      pattern: \"(owner|admin) (can|must)\"\n",
    )
    draft = tmp_path / "draft.md"
    draft.write_text("The admin can drain the pool by setting the fee.\n", encoding="utf-8")
    g2 = _gates(vg_binary, "--ledger", str(ledger), "--finding", str(draft))["G2"]
    assert g2["status"] == "fail", g2
    assert "privileged_role" in g2["detail"], g2
