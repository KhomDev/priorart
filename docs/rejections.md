# The rejection dataset

`data/rejections/` is the public export of the author's rejection taxonomy,
moved from rejection-taxonomy v0.6.1. It is the descriptive layer behind the
mechanism vocabulary, never the headline evidence.

## What ships

| File | Content |
|---|---|
| `rejections.csv` | 113 rows, 10 columns, closed vocabularies, no free text |
| `rejections.json` | the same rows as JSON |
| `summary.json` | aggregate counts; **keys are unchanged** and must stay that way |
| `SCHEMA.md` | the column contract, the excluded dimensions, the allowed values |
| `LICENSE-DATA` | CC BY 4.0 for the data and prose |
| `CITATION.cff` | how to cite the dataset (the v0.6.1 file) |
| `provenance.json` | source hashes, plus this copy's provenance |

The CSV, JSON, `summary.json`, `SCHEMA.md` and `LICENSE-DATA` are byte-identical
to the rejection-taxonomy release tree; v0.6.1 changed docs only, and v0.7.0 was
a README banner, so the data is the v0.6.1 data.

Columns: `id, family_id, bug_family, mechanism_class, triage_class,
earliest_gate, root_cause_class, repeat_of_own, duplicate_source,
preventable_by`. Ten columns: identifiers, closed-vocabulary categories and
ordinal/boolean fields. Nothing free-text, and no protocol, chain, ecosystem,
language, date, severity or report title: those are structurally excluded from
the export because with 113 rows they would fingerprint a target.

## Counts

- **113 logged submissions, 110 published, 3 withheld.** The withheld rows stay
  in the CSV with a withheld payload so the row count and the denominator do not
  move; `summary.json` records `withheld_rows: 3` and
  `published_rows: 110`.
- **63 of 113 (55.8%) public-record preventable** and **9 more only by the
  author's own submission log**; 31 were not preventable, 7 unknown, 3 withheld.
- Triage: 43 duplicate, 26 out of scope, 24 informational, 16 intentional design,
  3 below the payout floor, 1 spam.
- Earliest catchable gate: duplicate check 41, G2 exclusion 20, G3 scope 4,
  G4 prior audit 7, G1 severity floor 5, plus the historical P-rule ids that the
  dataset still records.

## The limitation that matters

**It is one researcher's log.** n=113, 3 withheld, 110 published. There is no
positive class (no paid findings to compare against), no target dimension, and
the selection process is one person's hunting and submission history. It cannot
support a precision claim, a base rate, or a statement about the population of
bug-bounty findings. The remediation record for the representativeness question
is `docs/representativeness.md` in rejection-taxonomy v0.6.1.

What it can support, and what priorart uses it for:

- a **vocabulary** of mechanism classes and triage outcomes (the mechanism hints
  in the matcher are keyed to it), and
- a **descriptive** layer: 63 of 113 rejections (55.8%) could have been stopped
  from the public record - the program's pages, its published audits and known
  issues, or its test suite - and 9 more only by the author's own log.

It is not evidence that priorart catches those: the benchmark in
[benchmark.md](benchmark.md) measures retrieval, and its numbers are separate.

## Provenance of this copy

`provenance.json` keeps the four original keys (`source_sha256`,
`classifier_sha256`, `ledger_sha256`, `removed_ids`) and adds the facts a reader
needs to verify this copy:

- `dataset_version: "0.6.1"` and `source_tag: "v0.6.1"`;
- `data_identical_to: "0.6.0"` - v0.6.1 was docs-only, so
  `git diff v0.6.0..v0.6.1 -- data/` is empty;
- `source_repository: https://github.com/KhomDev/rejection-taxonomy`;
- `moved_into: "priorart 0.1.0, data/rejections/"`;
- `licence: "CC-BY-4.0 for data and prose, MIT for code"`.

`removed_ids` is empty because nothing was removed from the export: the three
withheld rows are present with a withheld payload.

## How `priorart rejections summary` reads it

The tool compiles `summary.json` and `provenance.json` into the binary and prints
them; it does not read `data/` at runtime.

```text
$ priorart rejections summary
rejection dataset - 113 logged submissions, 110 published, 3 withheld
  pre-hunt catchable from the public record: 63 of 113 (55.8%)
  one researcher's log: no positive class, no target dimension. See docs/rejections.md.

triage outcome
  below_payout_floor       3
  duplicate                43
  informational            24
  intentional_design       16
  out_of_scope             26
  spam                     1
...
provenance: source sha256 8dc5ea43ee90382919a15a0a76cc8a063685320d75cc9f5d47a5798e08991825, dataset v0.6.1
licence: data CC BY 4.0, code MIT. See data/rejections/LICENSE-DATA.
```

`--json` wraps the unchanged `summary.json` under `dataset` and adds
`provenance`, with schema id `priorart.rejections-summary/v1`.

## Privacy

The private classifier, the ledger, the blocklists and the private inputs stay
out of this repository. Only the two-stage public export enters priorart, and the
release checklist in [CONTRIBUTING.md](../CONTRIBUTING.md) requires the hygiene
and blocklist scans before a push.

## Licence

Data and prose: CC BY 4.0, see `data/rejections/LICENSE-DATA` and
[licensing.md](licensing.md). Code: MIT. Cite the dataset as
`data/rejections/CITATION.cff` says: the rejection taxonomy by KhomDev,
version 0.6.1, CC BY 4.0.
