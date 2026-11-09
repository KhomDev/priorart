#!/usr/bin/env python3
"""Fixtures for the identity-token matcher.

Two directions, both pinned:

  * An English word that merely CONTAINS a token must pass. This is the
    regression the old matcher failed: a word like `suite` was reported as a short
    chain token, and `nearest` was reported as another, because the boundary test
    treated the following letter as a separator.
  * Every declared token must fail when it stands alone in a sentence. The tokens
    are read from the vocabulary rather than retyped, so adding a token to the
    list immediately adds a fixture for it, and this file cannot drift from the
    list it tests.
"""

from __future__ import annotations

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "scripts"))

import check_generic  # noqa: E402
import check_hygiene  # noqa: E402
import token_scan  # noqa: E402

# English words that contain a token as a substring but are not the token.
CONTAINS_A_TOKEN = (
    "suite",
    "suitcase",
    "nearest",
    "nearly",
    "button",
    "cotton",
    "function",
    "functional",
    "remove",
    "removed",
    "movement",
    "anchorite",
    "witnessed",
    "precompiled",
    "creative",
)


def test_english_words_that_contain_a_token_pass():
    for word in CONTAINS_A_TOKEN:
        text = f"The {word} is described in this sentence."
        hits = token_scan.scan_text(text)
        assert hits == [], f"{word!r} must not match, but matched {hits}"


def test_every_declared_token_fails():
    assert token_scan.TOKENS, "the vocabulary is empty"
    for token in token_scan.TOKENS:
        text = f"A sentence that mentions {token} plainly."
        caught = [t for t, _ in token_scan.scan_text(text)]
        assert caught == [token], f"{token!r} must be caught, got {caught}"


def test_the_boundary_is_a_word_boundary_not_a_substring():
    token = token_scan.TOKENS[0]
    assert token_scan.scan_text(f"a {token}tte of checks") == []
    assert [t for t, _ in token_scan.scan_text(f"checks for {token}")] == [token]


def test_only_the_vocabulary_module_is_exempt():
    assert set(check_hygiene.ALLOWED) == {"scripts/token_scan.py"}


def test_the_checks_and_the_tests_share_one_matcher():
    # CI runs scripts/check_hygiene.py and scripts/check_generic.py; the fixtures
    # call the same function. Two matchers would drift, and the drift would be
    # invisible.
    assert check_hygiene.scan_text is token_scan.scan_text
    assert check_generic.scan_tokens is token_scan.scan_tokens


def test_the_genericity_check_catches_each_platform_token():
    for token in token_scan.PLATFORM_TOKENS:
        assert check_generic.scan(f"a program on {token}") != [], token


def test_the_hygiene_check_reports_a_token_in_a_temporary_tree(tmp_path, monkeypatch):
    token = token_scan.TOKENS[0]
    leak = tmp_path / "leak.md"
    leak.write_text(f"a note about {token}\n", encoding="utf-8")
    monkeypatch.setattr(check_hygiene, "ROOT", tmp_path)
    problems = check_hygiene.check_tripwires([leak])
    assert len(problems) == 1
    assert token in problems[0]
