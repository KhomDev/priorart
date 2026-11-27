#!/usr/bin/env python3
"""Salted split for eval/sealed-multilang.

Committed with the salt before any unit id exists. Do not change the salt
or this function after units are added.

    bucket = int(sha256(salt + '|' + unit_id).hexdigest()[:8], 16) % 100
    0-39 DEV, 40-59 V1, 60-74 V2, 75-99 FINAL
"""

from __future__ import annotations

import hashlib
from pathlib import Path

ROOT = Path(__file__).resolve().parent
SALT = (ROOT / "SALT").read_text(encoding="utf-8").strip()


def bucket(unit_id: str, salt: str = SALT) -> int:
    digest = hashlib.sha256(f"{salt}|{unit_id}".encode("utf-8")).hexdigest()
    return int(digest[:8], 16) % 100


def split_of(unit_id: str, salt: str = SALT) -> str:
    n = bucket(unit_id, salt)
    if n <= 39:
        return "DEV"
    if n <= 59:
        return "V1"
    if n <= 74:
        return "V2"
    return "FINAL"


def _self_test() -> None:
    # A placeholder id, not a corpus unit. Pins the arithmetic only.
    assert split_of("example-not-a-unit") in {"DEV", "V1", "V2", "FINAL"}
    assert bucket("example-not-a-unit") == bucket("example-not-a-unit")
    assert 0 <= bucket("example-not-a-unit") <= 99


if __name__ == "__main__":
    _self_test()
    print("split self-test ok")
