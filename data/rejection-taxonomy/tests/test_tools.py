"""The smaller tools: query.py, check_links.py, check_vocabulary.py, verify_blocklist.py."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "scripts"))

import check_links  # noqa: E402
import check_vocabulary  # noqa: E402
import query  # noqa: E402
import verify_blocklist  # noqa: E402

# ------------------------------------------------------------------ query.py


def test_query_rejects_an_unknown_value(capsys):
    with pytest.raises(SystemExit) as exc:
        query.main(["--triage", "dupe"])
    assert exc.value.code == 2
    assert "invalid choice" in capsys.readouterr().err


def test_query_json_filters_combine(capsys):
    assert query.main(["--triage", "duplicate", "--preventable-by", "own_log", "--json"]) == 0
    out = json.loads(capsys.readouterr().out)
    assert out and all(r["triage_class"] == "duplicate" and r["preventable_by"] == "own_log" for r in out)


def test_query_overview_does_not_claim_pre_hunt(capsys):
    assert query.main([]) == 0
    out = capsys.readouterr().out
    assert "preventable from the public record" in out
    assert "before reading any code" not in out


# ------------------------------------------------------------------ check_links.py


def test_same_file_anchor_is_checked(tmp_path, monkeypatch):
    monkeypatch.setattr(check_links, "ROOT", tmp_path)
    doc = tmp_path / "a.md"
    doc.write_text("# Real heading\n\n[ok](#real-heading) [bad](#missing)\n", encoding="utf-8")
    problems = check_links.check([doc])
    assert len(problems) == 1 and "#missing" in problems[0]


def test_broken_relative_link_is_caught(tmp_path, monkeypatch):
    monkeypatch.setattr(check_links, "ROOT", tmp_path)
    doc = tmp_path / "a.md"
    doc.write_text("[x](nope.md)\n", encoding="utf-8")
    assert check_links.check([doc])


# ------------------------------------------------------------------ check_vocabulary.py


def titles() -> dict[str, str]:
    t = dict(check_vocabulary.SHARED_IDS.values())
    t.update(dict(check_vocabulary.KNOWN_COLLISIONS.values()))
    return t


def test_vocabulary_agrees_when_titles_match():
    assert check_vocabulary.check(titles(), {"D1_duplicate_check": 3}) == []


def test_a_retitled_sibling_id_fails():
    t = titles()
    t["P17"] = "Something else"
    assert any("P17" in p for p in check_vocabulary.check(t, {}))


def test_a_missing_sibling_id_fails():
    t = titles()
    del t["G4"]
    assert any("G4" in p for p in check_vocabulary.check(t, {}))


def test_the_known_collision_fails_once_a_row_uses_it():
    assert check_vocabulary.check(titles(), {"P18_design_tradeoff": 0}) == []
    assert any("P18" in p for p in check_vocabulary.check(titles(), {"P18_design_tradeoff": 1}))


def test_the_old_duplicate_id_never_returns():
    assert check_vocabulary.check(titles(), {"P2_duplicate_check": 1})


# ------------------------------------------------------------------ verify_blocklist.py

SOURCE = """# private log

## Full Database

| Protocol | Report | Reason | Triage | Gate | Root cause | Fix |
|---|---|---|---|---|---|---|
| Qorvex | lastZunFees desync via the zun_pool accumulator path | sponsor says the zebra crossing guard was already reviewed carefully | Duplicate | 59 Phase 2 | known issue | log it |

## Next
"""


def make_repo(tmp_path: Path, files: dict[str, str]) -> Path:
    repo = tmp_path / "repo"
    repo.mkdir()
    for name, text in files.items():
        (repo / name).write_text(text, encoding="utf-8")
    subprocess.run(["git", "init", "-q"], cwd=repo, check=True)
    subprocess.run(["git", "add", "."], cwd=repo, check=True)
    return repo


def test_identifier_harvest_includes_lower_camel_and_snake_case():
    found = verify_blocklist.identifier_candidates("lastZunFees and zun_pool and Plain", allcaps=False)
    assert {"lastzunfees", "zun_pool"} <= set(found)
    assert "plain" not in found


def test_scan_repo_catches_identifiers_names_and_copied_text(tmp_path):
    src = tmp_path / "log.md"
    src.write_text(SOURCE, encoding="utf-8")
    repo = make_repo(
        tmp_path,
        {
            "rules.py": 'RULE = r"lastzunfees"\n',
            "notes.md": "We hunted qorvex once.\n",
            "doc.md": "the zebra crossing guard was already reviewed in the end\n",
            "clean.md": "An accumulator desync is a common bug.\n",
        },
    )
    allow, phrases = verify_blocklist.load_allowlist(None)
    problems = verify_blocklist.scan_repo(src, allow, phrases, repo)
    assert any(p.startswith("rules.py") for p in problems), problems
    assert any(p.startswith("notes.md") for p in problems), problems
    assert any(p.startswith("doc.md") and "copies private text" in p for p in problems), problems
    assert not any(p.startswith("clean.md") for p in problems), problems


def test_scan_repo_honours_allowed_phrases(tmp_path):
    src = tmp_path / "log.md"
    src.write_text(SOURCE, encoding="utf-8")
    repo = make_repo(tmp_path, {"doc.md": "the zebra crossing guard was already reviewed\n"})
    allow_file = tmp_path / "allow.yaml"
    allow_file.write_text('phrases:\n  - "the zebra crossing guard was already reviewed"\n', encoding="utf-8")
    allow, phrases = verify_blocklist.load_allowlist(allow_file)
    assert verify_blocklist.scan_repo(src, allow, phrases, repo) == []


def test_public_allowlist_carries_no_ecosystem_vocabulary():
    # These were in the published allowlist until 0.5.1; each says something about
    # the author's targets and now lives in the private allowlist.
    for word in ("ptb", "wasm", "utxo", "clmm", "evm", "erc4626", "package", "entrypoint"):
        assert word not in verify_blocklist.GENERIC_ALLOWLIST, word
