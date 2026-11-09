# Contributing

Corrections are the most valuable contribution to this repository.

## The two things that help most

### 1. A correction to a classification

Every row is a judgement made by one person. If a row's `triage_class`, `earliest_gate`,
`root_cause_class`, `duplicate_source` or `preventable_by` looks wrong, open a
[correction issue](https://github.com/KhomDev/rejection-taxonomy/issues/new?template=correction.md)
and say which row and why. Cite the program's own public wording where you can.

If the correction would mean describing the row in more detail than the dataset does - naming its
program, say - do not put that in a public issue. Use private vulnerability reporting instead (see
[SECURITY.md](SECURITY.md)).

The dataset is generated. A classification is corrected in the private classifier and the data is
rebuilt, so **never open a PR that hand-edits `data/`**: the next build would overwrite it. The
derived columns (`repeat_of_own`, `duplicate_source`, `preventable_by`) are computed by public rules
in `scripts/vocab.py`; a PR that changes one of those rules is welcome, with a test.

### 2. A new mechanism class, or a fix to the vocabulary

`mechanism_class` is a closed vocabulary. If a row does not fit any class, that is a bug in the
vocabulary. Open an issue naming the class you would add and what it would cover; the bar is that
it describes a *shape* of bug rather than one instance. A vocabulary change is a schema change and
goes in a major release.

## What will not be accepted

- **Any change that adds a target dimension.** No protocol, chain, ecosystem, language, date or
  severity column, under any name. `--check` fails if one appears, and CI runs it.
- **Any change that adds free text to the published data.** The allowlist is the design.
- **Any de-anonymisation attempt**, including "just tell me which program row N is". That is the
  one thing this dataset exists not to do.

## Working on the code

```bash
python -m pip install ".[dev]"
python -m pytest
python -m ruff check .
python scripts/build_public_export.py --check --out data/
python scripts/stats.py --check
```

None of these needs a private input. The full build runs in CI on the synthetic fixture in
`tests/fixtures/`.

## Commit messages

Conventional commits (`fix:`, `feat:`, `docs:`, `test:`, `chore:`). A commit that changes published
numbers says which numbers changed and why, and updates `CSV_SHA256` in `tests/test_data.py`.
