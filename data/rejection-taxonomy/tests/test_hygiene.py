"""check_hygiene.py: encoding, private paths, tripwires and the history scan.

Every case runs on a temporary tree or a temporary git repository with invented
tokens; nothing here reads a private input.
"""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "scripts"))

import check_hygiene  # noqa: E402
import token_scan  # noqa: E402


def write(path: Path, data: bytes) -> Path:
    path.write_bytes(data)
    return path


def test_a_byte_order_mark_fails(tmp_path, monkeypatch):
    monkeypatch.setattr(check_hygiene, "ROOT", tmp_path)
    f = write(tmp_path / ".gitattributes", "\ufeff* text=auto eol=lf\n".encode())
    assert any("byte-order mark" in p for p in check_hygiene.check_encoding([f]))


def test_mojibake_and_invalid_utf8_fail(tmp_path, monkeypatch):
    monkeypatch.setattr(check_hygiene, "ROOT", tmp_path)
    moji = write(tmp_path / "a.md", "an em dash \u00e2\u20ac\u201d here\n".encode())
    bad = write(tmp_path / "b.md", b"\xff\xfe not utf-8\n")
    problems = check_hygiene.check_encoding([moji, bad])
    assert any("mojibake" in p for p in problems)
    assert any("not valid UTF-8" in p for p in problems)


def test_clean_file_passes(tmp_path, monkeypatch):
    monkeypatch.setattr(check_hygiene, "ROOT", tmp_path)
    f = write(tmp_path / "ok.md", b"plain text\n")
    assert check_hygiene.check_encoding([f]) == []


@pytest.mark.parametrize("token", check_hygiene.PRIVATE)
def test_every_private_path_is_caught(tmp_path, monkeypatch, token):
    monkeypatch.setattr(check_hygiene, "ROOT", tmp_path)
    f = write(tmp_path / "doc.md", f"see {token}notes\n".encode())
    assert len(check_hygiene.check_private([f])) == 1


def test_tripwires_are_found_with_a_private_list(tmp_path, monkeypatch):
    monkeypatch.setattr(check_hygiene, "ROOT", tmp_path)
    f = write(tmp_path / "doc.md", b"nothing here\nwe hunted on qorvex\n")
    hits = check_hygiene.check_tripwires([f], token_scan.compile_tokens(["qorvex"]))
    assert hits == ["doc.md:2: tripwire token 'qorvex'"]


def test_without_private_inputs_the_private_checks_skip(capsys, monkeypatch):
    monkeypatch.delenv("RT_PRIVATE_DIR", raising=False)
    assert check_hygiene.main(["--no-history"]) == 0
    out = capsys.readouterr().out
    assert "SKIP  tripwires" in out


def git(repo: Path, *args: str) -> None:
    subprocess.run(
        ["git", "-c", "user.email=t@example.invalid", "-c", "user.name=t", *args],
        cwd=repo,
        check=True,
        capture_output=True,
    )


def test_history_scan_finds_a_token_deleted_later_and_one_in_a_message(tmp_path, monkeypatch):
    repo = tmp_path / "repo"
    repo.mkdir()
    git(repo, "init", "-q")
    (repo / "a.md").write_text("we found it on Qorvex\n", encoding="utf-8")
    git(repo, "add", ".")
    git(repo, "commit", "-qm", "first")
    (repo / "a.md").write_text("generic now\n", encoding="utf-8")
    git(repo, "commit", "-qam", "mention zun in a message")
    monkeypatch.setattr(check_hygiene, "ROOT", repo)
    problems = check_hygiene.check_history({"Qorvex"}, set(), token_scan.compile_tokens(["zun"]))
    assert any("a.md" in p for p in problems), problems
    assert any("commit message" in p for p in problems), problems
    # The report never prints the token itself.
    assert not any("Qorvex" in p or "zun" in p for p in problems)


def test_blocklist_case_sensitivity(tmp_path):
    path = tmp_path / "b.yaml"
    path.write_text(
        "case_sensitive_groups: [names]\nnames: [Qorvex]\nhandles: [zunbot]\n", encoding="utf-8"
    )
    cs, ci = check_hygiene.load_blocklist(path)
    assert check_hygiene.blocklist_hits("a qorvex b", cs, ci) == []
    assert check_hygiene.blocklist_hits("a Qorvex b", cs, ci) == ["Qorvex"]
    assert check_hygiene.blocklist_hits("ZUNBOT", cs, ci) == ["zunbot"]


def test_this_repository_is_clean():
    files = check_hygiene.iter_files()
    assert check_hygiene.check_encoding(files) == []
    assert check_hygiene.check_private(files) == []
