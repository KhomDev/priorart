# rejection-taxonomy

**113 logged bug-bounty rejections, classified by triage outcome and by the earliest gate that
could have caught each one.**

The dataset contains **rejections only**. It has no positive class: there is no row for a finding
that was paid, because the private log this is derived from is a log of failures. Every proportion
below is a proportion *within a failure set*, and none of them is a success rate. That is a real
limitation, not a footnote - see [Known limitations](#known-limitations).

## What this is

A derived, redacted dataset. One row per rejected submission, reduced to categorical values:

    (bug family, mechanism class, triage outcome, earliest catchable gate,
     root cause class, whether it repeats an earlier family,
     where the duplicate signal lived, what could have prevented it)

Every value is a member of a closed vocabulary. There is no free text in the published data and no
target dimension: no protocol, no chain, no ecosystem, no date, no severity.

## What this is not

- **Not a survey.** n = 113, one researcher, one target-selection process.
- **Not base rates.** It is a log of failures. It cannot yield a success rate, and it says nothing
  about how many submissions succeeded.
- **Not a filterable target dataset.** You cannot filter by chain, ecosystem, language or program,
  because those columns do not exist. That is deliberate.
- **Not a benchmark of anyone else's work.** The gate assignments are the author's own post-hoc
  judgement.

## Data dictionary

| File | What it is |
|---|---|
| `data/rejections.csv` | one row per rejection, ten columns, all closed-vocabulary |
| `data/rejections.json` | the same rows, typed (`repeat_of_own` is a boolean here) |
| `data/summary.json` | every aggregate, computed by `scripts/stats.py` |
| `data/provenance.json` | SHA-256 of the private source, classifier and row ledger this release was built from |
| `data/SCHEMA.md` | every column, every allowed value, and how each derived column is computed |

```csv
id,family_id,bug_family,mechanism_class,triage_class,earliest_gate,root_cause_class,repeat_of_own,duplicate_source,preventable_by
R-091,F-080,replay,signature_context_binding,duplicate,D1_duplicate_check,duplicate_known_family,false,known_issue,public_record
R-021,F-021,accounting,accounting_order_of_operations,duplicate,program_ceiling,duplicate_known_family,false,known_issue,public_record
```

## Headline numbers

Everything between the markers below is generated. `scripts/stats.py` rewrites it from
`data/rejections.csv`, and the test suite fails if it has drifted. Do not hand-edit it.

<!-- BEGIN:generated -->

### Headline numbers

| Triage outcome | Count | Share |
|---|---:|---:|
| Duplicate / previously reported | 43 | 38.1% |
| Out of scope - boundary or exclusion clause | 26 | 23.0% |
| Informational - no security impact / no attacker profit | 24 | 21.2% |
| Intentional design / not a bug | 16 | 14.2% |
| Valid, but below the payout floor | 3 | 2.7% |
| SPAM - fabricated impact contradicted by the PoC | 1 | 0.9% |
| **Total** | **113** | **100%** |

**63 of 113 (55.8%) could have been stopped from the public record**: the program's own pages, its published audits and known issues, or its test suite. Another 9 could have been stopped only by the author's own submission log. Most of these checks need the finding in hand: a clause, an audit or a known issue covers something only once you know what you found.

### What could have prevented each rejection

| `preventable_by` | Count | Share |
|---|---:|---:|
| `public_record` | 63 | 55.8% |
| `none` | 31 | 27.4% |
| `own_log` | 9 | 8.0% |
| `unknown` | 7 | 6.2% |
| `withheld` | 3 | 2.7% |

### Where they died

| Earliest gate | Count | Share |
|---|---:|---:|
| `D1_duplicate_check` | 41 | 36.3% |
| `G2_exclusion_clause` | 20 | 17.7% |
| `P17_no_attacker_win` | 14 | 12.4% |
| `design_review` | 13 | 11.5% |
| `G4_prior_audit` | 7 | 6.2% |
| `G1_severity_floor` | 5 | 4.4% |
| `G3_scope_tree` | 4 | 3.5% |
| `REALISM` | 3 | 2.7% |
| `P16_below_floor` | 2 | 1.8% |
| `P19_headline_mismatch` | 1 | 0.9% |
| `program_ceiling` | 1 | 0.9% |
| `P20_cooperating_party` | 1 | 0.9% |
| `G5_test_intent` | 1 | 0.9% |

The duplicate check plus G1-G5 - the six gates viability-gate runs before the proof of concept - are the earliest gate for 78 rows (69.0%). That is not the preventable share: it includes duplicate rows whose signal the log does not record, a concurrent duplicate nobody could have pre-empted, and the withheld rows. Use `preventable_by` above.

### What kind of bug each row was

| `mechanism_class` | Count |
|---|---:|
| `liveness_block` | 15 |
| `accounting_order_of_operations` | 8 |
| `parameter_bound_missing` | 8 |
| `oracle_bounds_or_staleness` | 7 |
| `accounting_bias_direction` | 6 |
| `privileged_path_bypass` | 5 |
| `token_classification_assumption` | 5 |
| `unused_state_flag` | 4 |
| `signature_context_binding` | 4 |
| `key_binding_missing` | 4 |
| `cross_boundary_message_ordering` | 4 |
| `missing_zero_check` | 4 |
| `other_mechanism` | 4 |
| `replay_scope` | 3 |
| `withheld_valid` | 3 |
| `role_asymmetry` | 2 |
| `reward_dilution` | 2 |
| `stale_cached_value` | 2 |
| `no_op_state_transition` | 2 |
| `panic_without_persisted_impact` | 2 |
| `missing_consent_or_authorisation` | 2 |
| `boundary_condition_off_by_one` | 2 |
| `missing_input_validation` | 2 |
| `config_dependent` | 2 |
| `rounding_direction` | 2 |
| `slippage_protection_missing` | 2 |
| `input_length_validation_missing` | 1 |
| `secret_material_exposure` | 1 |
| `approval_or_allowance_overwrite` | 1 |
| `self_harm_precondition` | 1 |
| `callback_reentrancy_window` | 1 |
| `fee_base_or_ordering` | 1 |
| `incorrect_formula_or_constant` | 1 |

### Where the duplicate signal lived

| `duplicate_source` | Count |
|---|---:|
| `not_duplicate` | 67 |
| `known_issue` | 25 |
| `own_prior` | 7 |
| `unknown` | 6 |
| `withheld` | 3 |
| `prior_audit` | 2 |
| `public_disclosure` | 2 |
| `concurrent_submission` | 1 |

### Did the analysis hold up?

| Definition | Numerator | Count | Share |
|---|---|---:|---:|
| **strict** - the impact was fabricated | `spam` | 1 | 0.9% |
| **broad** - no impact, intentional design, or fabricated | `spam` + `informational` + `intentional_design` | 41 | 36.3% |

An earlier version of this document used `informational` alone (21.2%) as the numerator and called the remaining 78.8% 'targeting'. That is not a definition: it is 100% minus one bucket, and it counts intentional-design rows as targeting failures when they are analysis failures. Both honest definitions are above.

### Sample size and identifiability

- **Effective n: 98 distinct bug families**, over the 110 rows that publish a family (3 rows are withheld and carry none). 12 rows repeat a family the author had already submitted; together with their first submissions that is 21 rows in 9 families (the largest has 4 rows). A family whose first submission predates the log cannot be detected, so this is a lower bound on self-duplication.
- **Unknowns are reported, not guessed.** 6 of the 43 duplicate rows record no duplicate source, 7 rows have `preventable_by = unknown`, and 11 rows have the residual root cause `other`.
- **Identifiability.** 31 of the 32 mechanism classes are used, 7 of them by a single row, and 91 of 113 rows are the only row with their combination of bug family, mechanism class, triage outcome, earliest gate and root cause. A reader who already knows a specific submission could recognise it. See the disclosure model below.

<!-- END:generated -->

## Querying it

```bash
# what kinds of bugs were rejected as duplicates?
python scripts/query.py --triage duplicate

# everything the public record could have stopped
python scripts/query.py --preventable-by public_record

# rows that repeat an earlier family of my own
python scripts/query.py --repeats -v

# duplicate rows where the log does not say where the signal lived
python scripts/query.py --duplicate-source unknown

# one mechanism class
python scripts/query.py --mechanism-class replay_scope -v

# any filter, as JSON, for piping
python scripts/query.py --family oracle --json
```

Filter values are checked against the vocabulary, so a typo is an error rather than an empty result.
There is no `--paid-only` flag and no `paid` column: every row was rejected, so a filter for paid
rows would return nothing by construction.

This is a dataset with scripts, not an installable package. Run the scripts from a checkout;
`pip install .[dev]` installs only the pinned test and lint tools.

## Known limitations

Read these before quoting any number above. The counts behind them are in the generated
*Sample size and identifiability* section.

1. **No positive class.** The dataset is a log of rejections. It contains no paid finding, so it
   cannot produce a success rate, a precision, or a base rate.
2. **`earliest_gate` is self-assessed.** It is the author's post-hoc judgement of what *would*
   have caught the finding, not a record of what any tool said at the time. A different analyst
   would assign some rows differently. It is the most useful column and the least objective one.
3. **"Earliest gate" is not "preventable".** The six gates viability-gate runs before the proof of
   concept (the duplicate check and G1-G5) are the earliest gate for more rows than the public
   record could actually have stopped. `preventable_by` is the column that answers "could this
   have been avoided", and it says so row by row. Most of those checks also need the finding in
   hand: you have to know what you found to ask whether a clause, an audit or a known issue covers
   it.
4. **Repeated families.** Rows are not independent observations: some families were submitted more
   than once, so the effective sample is the number of distinct families, not the number of rows.
   Cells with one or two rows are anecdotes, not rates.
5. **The rules that cite this dataset were derived from it.** The gate list and the
   `mechanism_class` vocabulary were built by reading these same rows. Any calibration against
   them is **in-sample**: closer to a ceiling than to an unbiased estimate.
6. **One researcher, one selection process.** These are the programs the author chose to hunt.
   Target-selection habits - including the bad ones - are baked into every proportion. There is no
   comparison group.
7. **Self-assessed labels throughout.** `bug_family`, `root_cause_class`, `duplicate_source` and
   `preventable_by` are classifications made by the same person who wrote the reports.
8. **`unknown` and `other` are populated, and reported.** Where the private log does not support a
   classification, the value is `unknown` (or the residual `other` root cause) rather than a guess.
9. **Rule drift.** Programs change their terms. A verdict reflects the rules in force at submission
   time, not today's.
10. **`below_payout_floor` rows are heavily redacted.** See the disclosure model below.

## Disclosure model - read this before citing

The dataset carries **no target dimension** and **no free text**.

An earlier draft of this repository published `chain` and `ecosystem`, on the theory that
withholding the protocol name was enough. It is not: across a few dozen targets, `chain` +
`ecosystem` + outcome + gate is a join key - a fingerprint, not an anonymisation. A later revision
replaced those columns with a free-text `mechanism` description checked against a denylist, which
still could not catch a paraphrase. Both are gone.

**Every published value is a member of a closed vocabulary**, defined in `scripts/vocab.py` and
documented in [data/SCHEMA.md](data/SCHEMA.md). A value outside the vocabulary is a build failure,
not a warning.

**The code that reads private text is not in this repository.** The build has two stages:

1. A **private classifier** reads the private log (protocol names, report titles, sponsor
   correspondence) and writes one vocabulary value per column per row. Its rules necessarily quote
   private text, so it is not published. It keys each row by a stable hash, so inserting rows into
   the private log cannot shift a classification onto the wrong row.
2. The **public build**, `scripts/build_public_export.py`, takes only those vocabulary values. It
   validates them, applies the withholding rule below, derives `family_id`, `repeat_of_own`,
   `duplicate_source` and `preventable_by` with rules you can read in `scripts/vocab.py`, checks
   the invariants between columns, and writes `data/`.

Until 0.5.1 the classifier was published in the public build script, and its rules quoted
fragments of private report titles. That was a disclosure, and it is why the repository history was
reset for 0.6.0 (see the CHANGELOG). Copies of the earlier history taken before the reset cannot be
recalled.

| Published | Never published |
|---|---|
| `id` - an ordinal, `R-###`, stable across releases | Protocol names, or any class that describes one |
| `family_id` - an opaque family grouping, `F-###` | Chain, ecosystem, or language |
| `bug_family` - a closed vocabulary of 19 families | Submission dates, or any date |
| `mechanism_class` - 32 classes, plus 2 sentinels (`other_mechanism`, `withheld_valid`) | Severity claimed or assigned |
| `triage_class` - a closed vocabulary of 6 outcomes | Verbatim sponsor correspondence |
| `earliest_gate` - 16 gates, plus the `UNCLASSIFIED` sentinel | Module names, function names, line numbers |
| `root_cause_class` - a closed vocabulary of 20 reasons | Finding IDs, commit hashes, repository paths |
| `repeat_of_own`, `duplicate_source`, `preventable_by` - derived from the above | Report titles, or the rules that classify them |
| Aggregate counts, and digests of the private inputs | Which programs were hunted at all |

### What is withheld, and why

**The `below_payout_floor` rows ship only five fields.** They are `bug_family`, `triage_class`,
`earliest_gate`, `root_cause_class` and `mechanism_class = withheld_valid`. Everything else is the
literal value `withheld`.

These are the dataset's **confirmed-valid** rows: a real bug, a working proof, a team that agreed
it was real, and a program whose reward table could not pay for it. They are the most interesting
rows and also the most identifying, because "a confirmed-valid finding on a program with a
Critical-only floor" narrows the candidate set far more than a rejection does. The aggregate counts
are unaffected.

`--aggregates-only` goes further: it publishes the summary tables and drops the per-row tuples
entirely. The default ships the tuples because they let others recompute the distributions
themselves rather than trusting the author's arithmetic.

### Residual risk, stated honestly

Category tuples with no target axis disclose nothing about any specific program. That is the
design argument, and it is an argument, not a proof. Two residual risks remain:

- **Most rows are unique.** The generated *Identifiability* line above counts the rows that are the
  only one with their combination of five categorical values, and the mechanism classes held by a
  single row. A reader who already knows a specific submission - its program and roughly what it
  claimed - could recognise its row. The mitigations are the withholding rule, `--aggregates-only`,
  and the takedown policy below.
- **The author is not anonymous.** The repository is published under a handle, and the aggregate
  shape is a personal log. That is disclosed, not hidden.

If you believe a row identifies you or a program you work on, see [Takedown](#takedown).

## Reproducing

The public corpus is a **derived artifact**, never hand-edited.

```bash
# the public build, on the synthetic fixture (no private input needed)
python scripts/build_public_export.py \
    --classified tests/fixtures/classified.example.csv \
    --provenance tests/fixtures/provenance.example.json \
    --out /tmp/rt-example

# verify the published data/ and every generated number, without any private input
python scripts/build_public_export.py --check --out data/
python scripts/stats.py --check

# the release build (the author only; needs the private classifier's output)
python scripts/build_public_export.py \
    --classified classified.PRIVATE.csv --provenance provenance.PRIVATE.json --out data/
python scripts/stats.py
```

`data/provenance.json` records the SHA-256 of the exact private source, classifier and row ledger
each release was built from. The author keeps a snapshot of each, so a release can be rebuilt
byte-for-byte and checked against those digests; nobody else can rebuild it, by design.

The triage bucketing in the private classifier is checked, row by row, against the private
workspace's canonical stats script before every build. Two rows were reclassified by the author
(duplicate to no-attacker-win, recorded in 0.4.0); those are the only intended differences.

### Gate rename: `P2_duplicate_check` -> `D1_duplicate_check`

In 0.2.0 the duplicate gate was renamed. `P2` means "privileged-role-gated" in the
[viability-gate](https://github.com/KhomDev/viability-gate) anti-pattern catalog, and one token
meaning two things across two repositories was a documentation bug. `scripts/check_vocabulary.py`
now checks every shared id against viability-gate in CI. One known collision remains unused: no row
uses `P18_design_tradeoff`, whose id means something else in viability-gate, and the check fails if
one ever does.

## Takedown

If you believe a row identifies you, your team, or a program you work on, report it **privately**:
use **Report a vulnerability** on this repository's Security tab (GitHub private vulnerability
reporting). Only the maintainer sees the report. A public issue would itself say which program you
work on, so please do not open one for this.

Rows are removed on request, without argument and without requiring you to justify it. A removed
row disappears from the next release; its `R-###` id is not reused and is listed in
`data/provenance.json` under `removed_ids`. Releases that were already published, and copies other
people took of them, cannot be recalled. That limit is stated here rather than hidden. See
[SECURITY.md](SECURITY.md).

## How to cite

```bibtex
@dataset{khomdev_rejection_taxonomy,
  title   = {Bug-bounty rejection taxonomy: 113 logged submissions classified by triage outcome and earliest catchable gate},
  author  = {KhomDev},
  year    = {2026},
  version = {0.6.0},
  url     = {https://github.com/KhomDev/rejection-taxonomy},
  note    = {Rejections only; no positive class. One researcher; see limitations.}
}
```

A machine-readable `CITATION.cff` is in the repository root.

## Related

- [bounty-economics](https://github.com/KhomDev/bounty-economics) - the taxonomies of program
  payout tiers and exclusion clauses that make `G1_severity_floor` and `G2_exclusion_clause`
  mechanical
- [viability-gate](https://github.com/KhomDev/viability-gate) - the CLI that runs these checks
  against a draft finding, using *your* private ledger
- [realism-gate](https://github.com/KhomDev/realism-gate) - a static checker for Foundry PoC
  tests: does the proof of concept show what an unprivileged attacker can do

Pin to a tag, not to `main`:
[rejection-taxonomy v0.6.0](https://github.com/KhomDev/rejection-taxonomy/tree/v0.6.0).

## License

| Path | License |
|---|---|
| `data/` (the published dataset) | CC-BY-4.0 |
| `scripts/`, `tests/` (code) | MIT |
| `docs/`, `README.md` (prose) | CC-BY-4.0 |
| the private classifier, row ledger and redaction inputs | not distributed |

The [`LICENSE`](LICENSE) file in the repository root is the MIT text and nothing else, so tools
that detect licences read it correctly. The CC-BY-4.0 terms covering the dataset and the prose
(`docs/`, `README.md`), and their scope, are in [`LICENSE-DATA`](LICENSE-DATA), and are also
stated here and in `CITATION.cff`.
