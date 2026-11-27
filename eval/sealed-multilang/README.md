# Sealed multilingual corpus (scaffold)

This directory is a new held-out corpus. It does not modify Dup-Bench, the six
benchmark corpora, their labels, salts, or splits.

**This commit contains no units.** `SALT` and `split.py` are fixed here, before
any unit id is chosen.

```text
bucket = int(sha256(salt + "|" + unit_id).hexdigest()[:8], 16) % 100
0-39 DEV, 40-59 V1, 60-74 V2, 75-99 FINAL
```

`priorart` is not run on these units. Reader labels stay `pending`.

Raw downloads go in `.corpus/` (gitignored). Third-party text with no licence
is stored as a pointer plus sha256 only.

`SEARCH_LOG.md` is added with the units, in a later commit. Queries are not
part of this scaffold commit.
