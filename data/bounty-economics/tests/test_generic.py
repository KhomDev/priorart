#!/usr/bin/env python3
r"""Tests for the genericity check.

The check itself is the fix for the CI step that was red: it grepped for
`github\.com/` anywhere in the schema, and the schema's own `$id` is a
github.com URL. Banning the project's own namespace was never the intent;
banning third-party names was.
"""

from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "scripts"))

import check_generic  # noqa: E402
from token_scan import PLATFORM_TOKENS  # noqa: E402


def test_the_shipped_taxonomies_and_schema_are_generic():
    assert check_generic.main([]) == 0


def test_the_schema_id_is_recognised_as_our_own():
    schema = (ROOT / "schemas" / "ledger.schema.json").read_text(encoding="utf-8")
    assert check_generic.scan(schema) == []


def test_a_platform_name_is_caught():
    platform = PLATFORM_TOKENS[0]
    assert check_generic.scan(f"see the {platform} page") != []


def test_a_platform_name_is_caught_case_insensitively():
    platform = PLATFORM_TOKENS[0].upper()
    assert check_generic.scan(f"See {platform}") != []


def test_a_foreign_repository_link_is_caught():
    assert check_generic.scan("https://github.com/SomeOtherOrg/private-target") != []


def test_a_gitlab_link_is_caught():
    assert check_generic.scan("https://gitlab.com/some/target") != []


def test_our_own_repository_link_is_allowed():
    assert check_generic.scan("https://github.com/KhomDev/viability-gate") == []


def test_a_bad_file_fails_the_command(tmp_path, capsys):
    platform = PLATFORM_TOKENS[0]
    bad = tmp_path / "leaky.yaml"
    bad.write_text(f"note: a real program on {platform}\n", encoding="utf-8")
    assert check_generic.main([str(bad)]) == 1
    assert platform in capsys.readouterr().err


def test_a_missing_file_is_a_usage_error(tmp_path):
    assert check_generic.main([str(tmp_path / "nope.yaml")]) == 2


def test_scanning_nothing_is_a_usage_error(monkeypatch):
    monkeypatch.setattr(check_generic, "default_targets", list)
    assert check_generic.main([]) == 2
