# Preregistration — P2 precision rebuild (priorart)

Committed 2026-11-25 on `p2/integration`, **before any `p2/*` source change and
before any Dup-Bench contest was fetched or labelled.** The verdict criteria in
§1 are copied from the session prompt
(`_orchestration/PROMPT_PRECISION_REBUILD_2026-11-25.md` §10) and are not
edited after this file is committed.

Working title: *Does a code-anchored, learned duplicate detector retrieve
judge-grouped duplicates on sealed later contests without flooding unique
findings?*

## 1. Verdict criteria (copied verbatim; do not edit)

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

The verdict is one of: **PRECISE AND VALUABLE** (every row holds),
**PARTIAL** (list which hold), **NOT YET** (core rows — recall@5, FAR — miss).

## 2. Dup-Bench v1

- **Population:** judged public smart-contract contests whose judge publishes
  per-finding duplicate grouping — Code4rena findings repositories (labels
  `duplicate-N`, `selected for report`, `sufficient quality report`,
  `satisfactory`, `unsatisfactory`, `invalid`) and Sherlock `*-judging`
  repositories (Valid / Invalid / Duplicate / Escalation outcomes). Target:
  ≥ 100 contests spanning ≥ 24 months.
- **Storage:** pointer plus hash only. A contest row records repo, commit,
  path and sha256; finding bodies are never committed. `fetch.py --check`
  verifies every pin offline before a round opens.
- **Labels:** the judge's own grouping. A finding is a *judged duplicate* when
  the judge grouped it with a representative; the representative's text is the
  retrieval target. A finding is *paid-unique* when the judge accepted it
  (paid severity) and did not group it with another. No reader label is used
  for either.
- **Temporal split, fixed before collection ends:** **T = 2025-09-01**,
  **T2 = 2026-05-01**.
  - train: contests whose judging closed before T;
  - validation: judging closed [T, T2);
  - FINAL: judging closed at/after T2 — sealed, opened once.
- **Sealed pairs for the alarm:** ≥ 6 new pairs (a prior audit of the same
  codebase plus a later judged contest with public duplicate grouping), ≥ 3
  sponsors not used before. The existing six corpora are DEV.

## 3. Model and alarm

- **Ranker:** logistic regression, or a small GBDT exported to a fixed data
  table (no runtime ML dependency). Features: anchor overlap at function and
  state level; call-graph distance (≤ 2); mechanism match; impact match;
  today's identifier score as one feature only.
- **Training:** only the Dup-Bench **train** split (contests before T).
  Hyperparameters chosen on the **validation** split (T to T2). Weights
  committed as a versioned data file together with the training script and its
  seed. Inference deterministic and offline; no LLM.
- **Calibration:** Platt or isotonic on the validation split. The "shares a
  root cause" warning turns ON only when the threshold's validation precision
  Wilson lower bound is ≥ 0.80. Otherwise it stays OFF and the report says so.
- **Abstention:** a finding with no resolved code anchor abstains with a
  reason. The abstention rate is reported.

## 4. Metrics (definitions fixed before the run)

Using the existing `eval/benchmark/run.py` semantics:

- **duplicate recall@5:** for each judged duplicate, the representative
  (or a member of its group) appears in priorart's top 5. k/n with Wilson 95%.
- **FAR:** for each paid-unique finding, the fraction on which priorart raises
  any prior item at the shipped threshold. k/n with Wilson 95%.
- **known-prior recall:** the judge excluded a finding as a repeat of a named
  prior item; the item is in the top 5. Reported as a count when n < 5.
- **abstention rate:** findings with no resolved anchor / all findings.

## 5. Analysis plan

- Every number is k/n with a Wilson 95% interval (z = 1.959964); cells with
  n < 5 are counts only.
- Every pooled cell gets a **contest-clustered bootstrap**: 10,000 draws
  resampling contests, seed `p2r-bootstrap-2026-11-25`, committed with the
  script.
- Never pool the six DEV corpora with Dup-Bench numbers; never pool two
  binaries. In-sample and fresh numbers are separate tables.
- Calibration curve and threshold selection are reported with the validation
  slice only; FINAL is never used to choose a threshold.

## 6. Per-round targets

| Round | Slice | Targets |
|---|---|---|
| 1 | sealed pairs assigned to V1 + Dup-Bench validation (model selection only) | recall@5 ≥ 40% point (exploratory), FAR reported, abstention reported |
| 2 | V2 | recall@5 ≥ 55% point, FAR ≤ 20% point (exploratory), calibration curve reported |
| Final | FINAL (after T2) + new sealed pairs | the §1 verdict table, once, by the frozen binary |

## 7. Stopping rule (copied from §8.4)

There is no further round after FINAL. If a validation round misses, you may
run at most **one** extra validation round (V3), carved from FINAL only if
FINAL still meets its minimum n afterwards; preregister this before carving.
The FINAL holdout is never used for anything but the final verdict.

## 8. Freeze record (filled at each round, never before)

| Round | Code commit | Binary sha256 | Weights file sha256 | Corpus commit |
|---|---|---|---|---|
| 1 | — | — | — | — |
| 2 | — | — | — | — |
| Final | — | — | — | — |
