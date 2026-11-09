# Schema - data/rejections.csv / data/rejections.json

One row per rejected submission. **Generated - never hand-edit.** Rebuild with
`scripts/build_public_export.py`; the vocabularies are defined once, in `scripts/vocab.py`.

## Columns

Ten columns. Nine are closed-vocabulary categories; one (`id`) is an ordinal. **There is no target
dimension and no free text.** Validation is by **allowlist**: a value outside the vocabulary is a
build failure, not a warning.

| Column | Type | Source | Description |
|---|---|---|---|
| id | string | ledger | `R-###`. Assigned once, from a private ledger keyed by a stable hash of each private row, and never reused or renumbered. Carries no information about the target. |
| family_id | enum | derived | Opaque family grouping, `F-###`, numbered in first-seen order over the published rows. Rows in one family are submissions of one bug. `withheld` on withheld rows. |
| bug_family | enum | classified | Closed vocabulary of 19 families. |
| mechanism_class | enum | classified | 32 classes plus 2 sentinels. Assigned privately from the report title. |
| triage_class | enum | classified | Normalised triage outcome. Six values. |
| earliest_gate | enum | classified | The earliest check that would have caught this before submission. |
| root_cause_class | enum | classified | Closed-vocabulary reason the finding failed. `other` when unmapped. |
| repeat_of_own | bool | derived | Whether this row repeats a family the author had already submitted. |
| duplicate_source | enum | derived | Where the duplicate signal lived. |
| preventable_by | enum | derived | What could have stopped it. |

**classified** columns are written by the private classifier. **derived** columns are computed by
the public build from classified values only, with the rules in `scripts/vocab.py`, which are
restated below.

The literal value `withheld` appears in family_id, repeat_of_own, duplicate_source and
preventable_by for rows whose descriptive payload is withheld. See [What is withheld](#what-is-withheld).

### Why there is no free text

An earlier revision carried a free-text mechanism column, validated against a denylist. A denylist
cannot catch a paraphrase, so the hole was structural rather than a matter of a longer list. The
column is gone, and so is the code that reads free text: the private classifier is not part of
this repository.

### Deliberately absent - and why

These exist in the private source and are **structurally excluded from the export**. `--check`
fails if a column with any of these names appears.

| Excluded | Why |
|---|---|
| protocol / protocol_class | Naming the target, or using a class that describes exactly one target, is a disclosure. |
| chain | With this many rows, chain + ecosystem + outcome is a fingerprint. |
| ecosystem | Same reason. |
| language | Language narrows a candidate set sharply. |
| report_title | Describes the specific bug; too identifying. |
| mechanism (free text) | Replaced by mechanism_class. |
| rejection_reason | Verbatim sponsor correspondence. Never reproduced. |
| submission_date | Narrows the candidate set. |
| severity_claimed / severity_assigned | Combined with outcome, narrows the target set. |
| module / function / line | Real symbols identify a codebase. |
| paid | Constant (always no). Replaced by a stated limitation: the dataset has no positive class. |
| source_row | The private row key. It stays in the private ledger. |

## Allowed values

### bug_family

| Family | Meaning |
|---|---|
| access_control | A caller-supplied key, slot or identifier not bound to the caller's authority |
| accounting | Value counted, credited, or discarded incorrectly |
| async_ordering | State committed across an asynchronous boundary in the wrong order |
| config | A parameter, constant or interval with no bound, or one that contradicts its docs |
| fees | Fee base, ordering or bypass |
| governance | Privileged or governance-mediated authority |
| input_validation | Caller-supplied input trusted without a check |
| liquidation | Liquidation math, ordering or incentive |
| liveness | A path that can be permanently blocked or made unreachable |
| oracle | Price or rate freshness, sourcing or manipulation |
| panic | A revert, panic or abort with no persisted consequence |
| reentrancy | Reentry through a callback |
| replay | A signed or authorised message reusable beyond its intended scope |
| rewards | Reward accrual, distribution or dilution |
| rounding | Precision, truncation or rounding-direction asymmetry |
| secrets | Credential or key material exposure |
| signature_binding | A signature verified without binding the fields that matter |
| state_machine | A flag, guard or state transition that does not do what it claims |
| token_integration | Behaviour arising from a token or pair-classification assumption |

### mechanism_class

Assigned by the private classifier from the report title, first matching rule wins. Every row of a
family carries the class of the family's first row: a family is one bug.

The vocabulary has **34 members**: 32 mechanism classes, the `other_mechanism` residue, and the
`withheld_valid` sentinel. **33 are used** in the data, counting both sentinels; the generated
*Identifiability* line in the README says how many classes hold a single row.

accounting_bias_direction, accounting_order_of_operations, approval_or_allowance_overwrite,
boundary_condition_off_by_one, callback_reentrancy_window, config_dependent,
cross_boundary_message_ordering, fee_base_or_ordering, incorrect_formula_or_constant,
incomplete_state_predicate, input_length_validation_missing, key_binding_missing, liveness_block,
missing_consent_or_authorisation, missing_input_validation, missing_zero_check, no_op_state_transition,
oracle_bounds_or_staleness, other_mechanism, panic_without_persisted_impact, parameter_bound_missing,
privileged_path_bypass, replay_scope, reward_dilution, role_asymmetry, rounding_direction,
secret_material_exposure, self_harm_precondition, signature_context_binding,
slippage_protection_missing, stale_cached_value, token_classification_assumption, withheld_valid

**One class is a residue.** `other_mechanism` is the fallback when no rule matches. A row lands
there rather than being forced into a closer-but-wrong class.

### triage_class

Bucketed by the private classifier, **first matching rule** in this order. The order is
load-bearing; before every build the bucketing is compared, row by row, with the private
workspace's canonical stats script.

| Order | Value | Matched when the triage cell contains |
|---:|---|---|
| 1 | below_payout_floor | not paid, below payout, payout-floor, or a scope phrase of the form "out of scope for &lt;name&gt; payout" |
| 2 | spam | spam |
| 3 | duplicate | duplicate |
| 4 | out_of_scope | out of scope, or the word oos |
| 5 | informational | inform |
| 6 | intentional_design | *(fallback - also absorbs Held and Not applicable)* |

> **Rule 1 is literal.** A bare "unpaid" does **not** match. So "Duplicate / unpaid" buckets as
> duplicate, while "Valid - Medium, NOT PAID" buckets as below_payout_floor.
>
> **Rule 6 is slightly lossy.** "Held" (post-mortem) rows fall through to intentional_design.
> "Held" means unresolved, not "intended". It is folded in to keep one source of truth with the
> canonical script; held is not a distinct public value.
>
> **Owner overrides.** Two rows whose private record reads "duplicate" are published as
> informational / `P17_no_attacker_win`, by the author's decision recorded in 0.4.0: the finding
> has no attacker-win, so the duplicate framing is incidental. The override is keyed to the row's
> stable id in the private ledger.

### earliest_gate

The gate token must appear at the **start** of the private cell. Anything else becomes `REALISM`
or `UNCLASSIFIED`.

| Value | Meaning |
|---|---|
| D1_duplicate_check | Is this already a known issue, a prior audit finding or a public disclosure? **Renamed from P2_duplicate_check in 0.2.0.** |
| G1_severity_floor | Is there a payout tier at this severity? |
| G2_exclusion_clause | Does a program exclusion clause cover this? |
| G3_scope_tree | Is the root cause inside the scoped files? |
| G4_prior_audit | Is this surface already closed by a published audit? |
| G5_test_intent | Does the project's own test suite assert this is intended? |
| design_review | Intentional design / no impact review |
| program_ceiling | Program-level severity ceiling |
| P7_best_practice | Best-practice critique |
| P12_no_persisted_impact | Revert-only, no persisted impact |
| P16_below_floor | Confirmed mechanics, below the severity floor |
| P17_no_attacker_win | No attacker-win / self-harm / no net-positive economics |
| P18_design_tradeoff | Design tradeoff. **Unused, and retired at the next breaking release:** P18 means "known characteristic documented in a prior audit" in viability-gate. |
| P19_headline_mismatch | Headline contradicted by PoC evidence |
| P20_cooperating_party | Requires a cooperating party |
| REALISM | Post-mortem realism gates (attacker-model, configuration, concession, PoC) |
| UNCLASSIFIED | No gate token recognised - investigate |

There are **17 members** in the vocabulary: 16 gate values plus the `UNCLASSIFIED` sentinel. **13 are
used** in this dataset.

The ids `D1`, `G1`-`G5`, `P7`, `P12`, `P16`, `P17`, `P19` and `P20` are shared with viability-gate
and mean the same thing there; `scripts/check_vocabulary.py` checks it. D1 and G1-G5 are the six
gates viability-gate runs before the proof of concept is written (it calls them pre-hunt gates).
Each still needs the finding in hand, and being the *earliest* gate for a row does not mean the
public record could have stopped it: see `preventable_by`.

### duplicate_source

Where the duplicate signal lived. Set on duplicate rows only; every other published row is
`not_duplicate`.

The private classifier reads the log for a duplicate signal - `known_issue`, `prior_audit`,
`public_disclosure`, `concurrent_submission`, `own_prior` or `unknown` - on every duplicate row and
on every row whose earliest gate is D1. The public build then publishes it, with one rule: a
duplicate whose log names no source, but which repeats a family the author had already submitted,
is `own_prior`. The signal for that row lived in the author's own log.

| Value | Meaning | Rows |
|---|---|---:|
| not_duplicate | The row was not closed as a duplicate | 67 |
| known_issue | The team's own known-issues list, or an accepted-risk decision | 25 |
| own_prior | The author's own earlier submission, logged or not | 7 |
| unknown | The private log does not say, and guessing was not acceptable | 6 |
| withheld | The row's payload is withheld | 3 |
| prior_audit | A published audit covered it | 2 |
| public_disclosure | A contest result, a published report, or a paid bounty | 2 |
| concurrent_submission | Someone else found it at the same time | 1 |

**No audit firm is named by the classifier.** An earlier revision matched on the names of the firms
that had audited the author's targets, which is a disclosure: naming the auditor narrows the target
set.

### preventable_by

What would have stopped the rejection, from the record. The cheapest source wins:

1. a public duplicate signal (`known_issue`, `prior_audit`, `public_disclosure`), whatever gate the
   row records -> `public_record`;
2. an earliest gate answerable from the program's public record (G1-G5) -> `public_record`;
3. the row repeats a family the author had already submitted, or the log says it duplicated the
   author's own earlier submission -> `own_log`;
4. a concurrent duplicate -> `none` (nobody could have pre-empted it);
5. a D1 or unrecognised gate with no recorded signal -> `unknown`;
6. anything else needed the finding in hand and a judgement -> `none`.

| Value | Meaning | Rows |
|---|---|---:|
| public_record | Readable from the program's own pages, its published audits or known issues, or its test suite | 63 |
| none | Nothing on file would have caught it; it needs the finding in hand and a judgement about it | 31 |
| own_log | Only the author's own submission log could have caught it | 9 |
| unknown | The log does not support one of the above | 7 |
| withheld | The row's payload is withheld | 3 |

Until 0.5.1 rules 1 and 3 were missing: known-issue duplicates recorded at a non-D1 gate counted
as `none`, and rows repeating the author's own family counted as `unknown` or `none`. See the
CHANGELOG for the rows that moved.

### root_cause_class

Closed vocabulary. Anything unmapped becomes `other`, which is a signal to widen the map; the
generated README section reports how many rows are `other`.

duplicate_known_family, oos_privileged_role, admin_input_validation, oos_scope_boundary,
oos_exclusion_clause, intentional_design, no_attacker_win, no_fund_loss, below_severity_floor,
function_visibility, prior_audit_known, gas_or_dos_only, third_party_dependency, cooperating_party,
spam_fabricated_impact, config_dependent, auxiliary_accounting, best_practice, no_demonstrated_impact,
other

## What is withheld

Rows with triage_class below_payout_floor ship **only** bug_family, triage_class, earliest_gate,
root_cause_class and mechanism_class = withheld_valid. Everything else is the literal value
withheld.

These are the dataset's **confirmed-valid** rows: a real bug, a working proof, a team that agreed it
was real, and a program whose reward table could not pay for it. They are the most interesting rows
and also the most identifying, because "a confirmed-valid finding on a program with a Critical-only
floor" narrows the candidate set far more than a rejection does. They take no `family_id`, so the
family numbering has no gap that would point at them.

`--aggregates-only` goes further: it publishes the summary tables and drops the per-row tuples
entirely.

## Invariants

The build refuses to write, and `--check` fails, unless all of these hold:

- every value is in its allowlist, and no cell matches a URL, hash, date or path shape;
- row ids are dense from `R-001`, except ids listed in `provenance.json` as removed;
- withheld rows ship exactly the five fields above, and no other row carries a withheld value;
- family ids open in first-seen order, the first row of a family has `repeat_of_own = false` and
  every later row `true`, and all rows of a family share one bug_family and one mechanism_class;
- duplicate_source is `not_duplicate` exactly when the row is not a duplicate;
- preventable_by agrees with rules 1-3 above wherever the published columns determine it;
- rejections.json equals rejections.csv cell for cell, and provenance.json is well formed.

## Aggregates

Published as `data/summary.json`, generated by `scripts/stats.py`, which also rewrites the generated
block in the README. The keys present in 0.5.x are kept unchanged; 0.6.0 appends
`published_rows`, `rows_in_repeated_families`, `public_record_preventable(_pct)`,
`duplicate_unknown`, `root_cause_other`, `mechanism_classes_used`,
`singleton_mechanism_classes` and `unique_category_tuples`.

`prehunt_catchable` counts rows whose earliest gate is D1 or G1-G5. It is kept for compatibility and
it is **not** the preventable share: `public_record_preventable` is.

## Validation

    python scripts/build_public_export.py --check --out data/
    python scripts/stats.py --check
