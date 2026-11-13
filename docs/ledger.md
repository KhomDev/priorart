# Ledger validation: schema v1

The ledger is your own transcription of a program's terms: the payout shape and
floor, the exclusion clauses, the known issues and prior audits. It is private
(it names real programs), so it never ships with the tool. `priorart ledger
validate` is the Rust port of bounty-economics `scripts/validate.py`; the schema
is [`schemas/ledger.schema.json`](../schemas/ledger.schema.json), byte-identical
to the bounty-economics v0.6.1 tag (sha256
`403324A1CA205BE9521FE9F51FCCF45030F3BE9B7AA642A2C727DF5842220A51`, checked
2026-11-13), and the version stays `1`.

## How to run it

```sh
priorart ledger validate my-ledger.yaml
priorart ledger validate ledgers/*.yaml --json
priorart ledger validate examples/ledger.example.yaml examples/tier-shapes.example.yaml
priorart ledger validate examples/ledger.example.yaml --taxonomy   # vocabularies instead
```

Each entry has 13 keys available: `schema_version`, `id`, `tier_shape`,
`floor_status`, `floor_severity`, `source_url`, `retrieved`, `conflicts`,
`exclusions`, `prior_audits`, `post_audit_delta`, `notes`, `engagement`.
Unknown keys are errors, because a silently ignored field is a field you believe
you recorded and did not.

## What it checks

Schema checks: unknown keys, types and enums, the `id` slug pattern
(`^[a-z0-9][a-z0-9._-]*$`), the `source_url` http(s) requirement, and the
`retrieved` date format.

Checks a schema cannot express:

- **Every entry is cited.** A missing `source_url` or `retrieved` is an error:
  an uncited entry is a guess, and a guessed payout floor is worse than none.
- **An unstated floor is not "Low pays".** `floor_status: unstated` requires
  `tier_shape: unknown` and a null `floor_severity`. `documented` requires a
  severity and a shape that is not `unknown`. `conflicting` keeps the shape
  `unknown` and warns.
- **The removed `any` shape** gets a migration message naming
  `low_and_above` + `documented` or `unknown` + `unstated`, not a bare enum
  error.
- **`informational` is not a floor.** No program pays it in practice; the error
  says what to write instead (`low` + `documented`, or null + `unstated`).
- **Placeholder hosts** are caught by host, not by substring: `localhost`,
  `127.0.0.1`, `[::1]`, `0.0.0.0`, `*.invalid`, `*.localhost`, the reserved
  `example.com/.org/.net` domains, and a `todo`/`tbd`/`changeme` label fail
  in a real ledger and warn in `*.example.yaml`. A real URL that happens to
  contain "todo" is fine.
- **Dates** are checked as real calendar dates; a future date is an error and one
  older than 180 days warns (terms change).
- **Patterns** must compile in the Rust regex engine and stay in the dialect
  both engines share: lookaround, backreferences, possessive quantifiers, `\Z`,
  `\0`, comment groups, conditionals and Python-only inline flags are errors.
  Character classes such as `[*+]` are not mistaken for quantifiers.
- **Several ledger files** in one run are supported; with more than one, errors
  are prefixed by file and no ambiguous top-level `ledger` key is emitted.

## Output

Text: `WARN` lines on stdout, `ERROR` lines on stderr, then a one-line summary.
`--json` prints
`{schema_version, ok, strict, ledgers[], entries, errors[], warnings[]}`, with
`ledger` added for a single-file run, matching the Python validator's shape.

```text
$ priorart ledger validate examples/ledger.example.yaml examples/tier-shapes.example.yaml
WARN  priorart/examples/ledger.example.yaml: entry[0] (example-lending-v2): source_url uses a placeholder host (example.invalid) - expected in an example file, not in a real ledger.
...
priorart/examples/ledger.example.yaml: 1 entry, 0 error(s), 1 warning(s)
priorart/examples/tier-shapes.example.yaml: 5 entries, 0 error(s), 9 warning(s)

6 entries, 0 error(s), 10 warning(s)
```

Exit codes: 0 valid (warnings allowed), 1 errors (or warnings under
`--strict`), 2 missing file, unparseable YAML, a non-list document, or more than
one YAML document.

## Golden fixtures

`examples/ledger.example.yaml` (1 entry) and
`examples/tier-shapes.example.yaml` (5 entries, one per shape) are the fixtures
the Python test suite used; they are kept as golden files. `ledger validate` on
both prints `6 entries, 0 error(s), 10 warning(s)` and exits 0, because both are
example files (placeholder hosts warn, not fail).

## What it does not check

- Whether the page you transcribed says what you recorded. Only you can re-read
  it.
- Whether the program still pays. Terms change; the staleness warning is the
  only defence.
- Anything about a finding. The ledger is the program side; `check` is the
  finding side.
