"""The tripwire matcher, on synthetic tokens.

The real token list is private (see scripts/token_scan.py for why), so these
fixtures use invented tokens with the same shapes: a short word that ordinary
English words contain, and a prefix token ending in '-'.
"""

from __future__ import annotations

import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "scripts"))

import check_hygiene  # noqa: E402
import token_scan  # noqa: E402

TOKENS = ("zun", "qorvex", "xyz-")
COMPILED = token_scan.compile_tokens(TOKENS)

# Words that CONTAIN a token but are not the token.
CONTAINS_A_TOKEN = ("zunit", "azun", "qorvexes", "preqorvex", "abcxyz-", "xyzzy")


@pytest.mark.parametrize("word", CONTAINS_A_TOKEN)
def test_a_word_containing_a_token_passes(word):
    assert token_scan.scan_text(f"The {word} is described here.", COMPILED) == []


@pytest.mark.parametrize("token", TOKENS)
def test_every_token_standing_alone_fails(token):
    text = f"A sentence that mentions {token}1 plainly." if token.endswith("-") else f"It mentions {token} plainly."
    assert [t for t, _ in token_scan.scan_text(text, COMPILED)] == [token]


def test_matching_is_case_insensitive():
    assert [t for t, _ in token_scan.scan_text("QORVEX and Zun", COMPILED)] == ["zun", "qorvex"]


def test_line_numbers_are_reported():
    assert token_scan.scan_text("first line\nsecond zun\n", COMPILED) == [("zun", 2)]


def test_load_tokens_reads_a_private_file(tmp_path):
    path = tmp_path / "tripwire-tokens.PRIVATE.yaml"
    path.write_text("tokens:\n  - zun\n  - qorvex\n", encoding="utf-8")
    assert token_scan.load_tokens(path) == ("zun", "qorvex")


def test_load_tokens_refuses_an_empty_file(tmp_path):
    path = tmp_path / "t.yaml"
    path.write_text("tokens: []\n", encoding="utf-8")
    with pytest.raises(SystemExit):
        token_scan.load_tokens(path)


def test_no_real_token_list_ships_in_this_repository():
    # The matcher module must not carry a token vocabulary again.
    text = (ROOT / "scripts" / "token_scan.py").read_text(encoding="utf-8")
    assert "TOKENS" not in text


def test_the_checker_and_the_tests_share_one_matcher():
    assert check_hygiene.scan_text is token_scan.scan_text
