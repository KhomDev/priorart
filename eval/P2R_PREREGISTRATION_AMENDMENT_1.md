# Amendment 1 to the P2 precision-rebuild preregistration (priorart)

Committed 2026-11-26 on `p2/research-pa`, **before any PA-B4 training, model
selection, ranker inference, calibration or FINAL evaluation**. This is a new
file. The original `eval/P2R_PREREGISTRATION.md` is not edited; where the two
disagree, this amendment governs the Dup-Bench temporal split only. Every other
part of the original (the model, the alarm, the analysis plan, the FINAL
verdict criteria) stays in force unchanged.

## 1. Why the original dates cannot be used

The original split fixed **T = 2025-09-01** and **T2 = 2026-05-01**. Both
windows are empty because **no public judged repository with duplicate grouping
exists in them**:

- The newest public `code-423n4/*-findings` repository is 2024-12. Code4rena has
  66 contest repositories for 2025-01..2026-07, but **none has a public
  `-findings` repository**, so no duplicate labels are published.
- The newest `sherlock-audit/*-judging` repository is 2025-07, and the Sherlock
  judging files carry no duplicate metadata (`comments.csv` empty; issue files
  carry a severity line only).

The evidence, queries and counts are in `eval/dupbench/SEARCH_LOG.md`
(2026-11-25). Under the original dates all 111 labeled contests are train
(1,102 paid-unique, 3,454 duplicates), validation and FINAL are empty, and the
FINAL minimum could never be met. The original dates were fixed before
collection ended on the expectation that judged repositories would continue to
be published; that expectation is false.

## 2. New fixed dates

**T = 2024-03-01, T2 = 2024-09-01.**

- train: judging closed before 2024-03-01;
- validation: judging closed in [2024-03-01, 2024-09-01);
- FINAL: judging closed at/after 2024-09-01 — sealed (see §4).

The dates were chosen **only** from (a) the judging-close dates of the pinned
contests and (b) the label counts per candidate split. **No ranker output,
calibration result, model score or FINAL label was computed or consulted before
this choice**; no PA-B4 training or inference has run. The choice is a function
of the platform's judging dates and the judges' own labels, which are fixed
before any model work.

Judging close date is the `date:` field of each contest's `report.md` (the
report publication date), recorded per contest in `eval/dupbench/sources.json`.

## 3. Split counts at these dates (Dup-Bench, 111 contests)

| Split | Contests | Paid-unique | Judged duplicates |
|---|---:|---:|---:|
| train (< 2024-03-01) | 58 | 564 | 1,237 |
| validation [2024-03-01, 2024-09-01) | 36 | 380 | 1,441 |
| FINAL (>= 2024-09-01) | 17 | 158 | 776 |

These are the counts in `eval/dupbench/manifest.json` regenerated with the new
dates. The FINAL minimums in §1 of the original preregistration are **met by
the FINAL slice: 17 contests, 158 paid-unique findings, 776 judged
duplicates** (minimums: ≥ 60 paid-unique and ≥ 300 duplicates).

## 4. The FINAL slice is sealed

The 17 FINAL contests have been consumed by nothing: no runner, matcher,
ranker, model, calibration or reader has processed them, and no result on them
exists. Their labels are the judges' own duplicate grouping (GitHub
`duplicate-N` labels), not model outputs or reader labels. A contest added
after this amendment joins a later slice only; the FINAL slice is not enlarged
by later collection. Opening FINAL is a separate, preregistered step after the
final binary freeze, exactly as in the original `§8` and `§10`.

## 5. Verdict criteria (copied verbatim from §1; unchanged)

**priorart** (FINAL: Dup-Bench contests after T2 plus the new sealed pairs;
≥ 60 paid-unique findings and ≥ 300 judged duplicates):

| Criterion | Requirement |
|---|---|
| Duplicate recall@5 | ≥ 60% point, Wilson lower bound ≥ 50% |
| FAR on paid-unique findings, at the shipped threshold | ≤ 15% point, Wilson upper bound ≤ 25% |
| Warning | ships ON only if FINAL precision at its threshold has a lower bound ≥ 0.80; otherwise OFF, and the verdict says so |
| Abstention rate | reported; ≤ 25% |

**Engineering** (must all hold): CI green as in §9, coverage ≥ 85% on new
crates, zero unexplained mutants, fuzz weekly job green, no schema-breaking
change, offline default.

The verdict is one of:
- **PRECISE AND VALUABLE:** every row holds;
- **PARTIAL:** list the rows that hold and those that don't;
- **NOT YET:** the core rows (readability, pooled precision, recall@5, FAR) miss.

## 6. Deviations recorded

1. **Dates changed after collection.** The original file says the dates were
   fixed before collection ended. This amendment changes them afterwards. It is
   not tuning: the split is assigned by each contest's judging close date, the
   labels are the platform's own duplicate grouping, and the choice precedes
   every model, calibration and FINAL computation. The alternative was an
   unfillable FINAL slice, which would make the preregistered verdict
   impossible to measure.
2. **Platform concentration.** Dup-Bench v1 is 100% Code4rena (111 contests).
   This is reported in `manifest.json` and `SEARCH_LOG.md`; no Sherlock
   contests were usable because the judging repositories carry no duplicate
   metadata and the newest is 2025-07.
3. **Selection bias.** 61 contests in the window with more than 500 curated
   submissions were excluded to bound the label-API budget (documented rule in
   `sources.json`). The corpus therefore under-samples the largest contests.
4. **Bulk format.** The committed machine files are JSON, not YAML, because the
   collection environment has no PyYAML; the README documents the format.
5. **Sealed pairs.** The six pairs (Basin, Revert Finance, Size, Ethena Labs,
   LoopFi, Othernet Global / Salty) are unchanged by this amendment and do not
   depend on the split dates; they are assigned to V1 as in the original plan.
