# Contributing

Thanks for considering a contribution. This repository is small on purpose, and the rules that
keep it small are the rules below.

## The one rule that is not negotiable

**Ship no target data.** No program names, no chains, no reward tables, no exclusion clause
quoted from a specific program, no prior-audit list. Examples must be fictional and use the
reserved `.invalid` TLD.

`scripts/check_generic.py` enforces the machine-checkable half of that (platform names, links to
repositories that are not ours) and runs in CI. The half a script cannot check is on you.

## No invented numbers

Every figure in a README, docstring or comment must come from a command someone ran, or be
labelled as not measured. If a number comes from a private log, say so and say that it is not
reproducible from this repository. "Industry-wide" is not a source.

## No weakened tests

If a test fails, fix the code or delete the claim the test was protecting. Do not narrow an
assertion to make it pass. A test that cannot fail is documentation with a false sense of safety.

## Making a change

```bash
python -m pip install -e ".[dev]"
python -m pytest tests/ -q
python -m ruff check .
python scripts/validate.py examples/ledger.example.yaml
python scripts/validate.py --taxonomy
```

All five must pass before a pull request. CI runs them on Python 3.10 through 3.13, plus the
genericity check and the placeholder-host negative test.

## Changing a taxonomy

The vocabularies are **data** (`data/tier-shapes.yaml`, `data/clause-classes.yaml`), and they
are read at runtime by the validator. If you add or rename an entry you must update, in the same
pull request:

1. the data file,
2. `schemas/ledger.schema.json` (the `enum`),
3. the README table,
4. the validator's migration message, if a value was removed,

because `tests/test_counts.py` fails when those drift apart. That test exists because they
drifted once.

## Changing the ledger format

The top level stays a **list** of entries. `vg` ([viability-gate](https://github.com/KhomDev/viability-gate))
deserializes the file as a sequence, so turning the root into a mapping breaks a tool that reads
the same ledger.

Additive fields are fine, and should be accompanied by:

- an optional `schema_version` treatment (absent means the current version),
- a line in the version matrix in `README.md`,
- `PINNED_VG_TAG` in `tests/test_compat_viability_gate.py` reviewed, because that pin is the
  only thing asserting the two repos still agree.

## Commits

Conventional-commit style, small and focused: `fix(ci):`, `feat(schema):`, `docs:`,
`test:`. One idea per commit.

## Licensing

By contributing you agree your contribution is licensed under the repository's MIT license. Do not
add a dependency or a data file whose license you have not checked.

## What is likely to be accepted

- A new clause class with a generic, unattributed example and a suggested pattern.
- A bug in the validator, with the smallest ledger that reproduces it.
- A correction to a claim in the README, with the source for the correction.

## What will not be

- A dataset, or an entry naming a real program — even anonymised.
- A reward table, however old.
- A re-license, or a change to the LICENSE file.
