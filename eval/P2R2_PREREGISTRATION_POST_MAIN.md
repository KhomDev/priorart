# priorart P2R-2 pre-registration for the held-out rounds after main (committed before any pair or FINAL measurement)

Committed 2026-11-27 (Europe/Budapest). It supplements `eval/P2R_PREREGISTRATION.md`,
`eval/P2R_PREREGISTRATION_AMENDMENT_1.md`, `eval/P2R_AMENDMENT_2.md` and `eval/P2R2_ANALYSIS_PLAN.md`.
Changes are stated with their reason. No sealed pair and no FINAL contest has been run by any P2 binary.

## 0. What ships to main, and what is not claimed
- `main` gets **DEV-only numbers**: train-split figures, and the validation split reported as
  **model-selection data** (the alarm was calibrated on it). Both are labelled DEV-ONLY in `docs/benchmark.md`
  and `docs/methodology.md`. **No held-out recall, FAR or warning precision is claimed.**
- The 0.2.2 figures (Tapioca, and the former sealed pairs Party + Wildcat) are in-sample history
  (`docs/limitations.md`).
- `eval/sealed-multilang` (#39) is a case study outside every metric.

## 1. Code and frozen models
| Item | Value |
|---|---|
| Source tree at pre-registration | `p2/integration` `b440394948a13fed9fbe7e04c0291199eb40a3b2` |
| Ranker | `eval/dupbench/weights/v1.json` sha256 `32ea46ec2bdc110e1a4af415212dad9bb2f5c105c1437b7a431aacd8f40f5abe` (fitted on train) |
| Alarm | `eval/dupbench/weights/alarm-v1.json` sha256 `1e492e8b4a6c6f8a1da111d62bae8030ff7a2265febcea4e8789ec01cf7683fa` (calibrated on validation; threshold 0.9016, raw score ≥ 0.854897) |
| Round-1 binary | sha256 recorded in the round file before the first pair is run (`pairs_run.py --mode pairs` refuses placeholders) |

Neither model is refit or recalibrated after this file.

## 2. Dataset manifest (sha256 of the committed LF files)
| File | sha256 |
|---|---|
| `eval/dupbench/manifest.json` | `e3a0d99ab594db9b7836550ef6f57d332616e3a184e3d6ac0e43eba4a88a0d68` |
| `eval/dupbench/labels.json` | `3c6084e0d37ac3be25e2d30868bbffaffb17a0559efbbc05d9f8f1bfd6dcabf7` |
| `eval/dupbench/pairs.json` (v1, six pairs) | `81d19f1c157304491a5096939bb48217da349967b363b5f8839caa7145f3ec31` |
| `eval/dupbench/pairs-swap/sealed_optA.json` | `d2af4796618a1ba48411f72017e8713fec35067217268e256236878eb97940a7` |

| Dup-Bench split | Contests | Paid-unique | Judged duplicates | Status |
|---|---:|---:|---:|---|
| train (before 2024-03-01) | 58 | 564 | 1237 | DEV, ranker fitted |
| validation (2024-03-01 .. 2024-09-01) | 36 | 380 | 1441 | model selection, alarm calibrated |
| FINAL (after 2024-09-01) | 17 | 158 | 776 | sealed, refused by every runner |

## 3. Pairs: the V1 slice after the swap (Yura, 2026-11-27)
The four pair contests that are also Dup-Bench contests (`2023-07-basin`, `2024-01-salty`, `2024-06-size`,
`2024-11-ethena-labs`) leave the pairs slice. The swap used option A: the 500-issue budget was dropped,
and rule 5a, which drops audits of vendored libraries, was added after the metadata screen and before
any finding was read. The V1 pairs are:

| Contest | Source | Labels needed |
|---|---|---|
| `2022-11-paraspace` | `sealed_optA.json` | judge grouping (516 issues); prior-report URL |
| `2023-02-ethos` | `sealed_optA.json` | judge grouping (860 issues); prior-report URL (only copy is a Discord attachment, weak evidence) |
| `2023-09-venus` | `sealed_optA.json` | judge grouping (684 issues); prior-report URL |
| `2023-07-arcade` | `sealed_optA.json` | judge grouping (573 issues); prior-report URL |
| `2024-03-revert-lend` | `pairs.json` v1 | judge grouping (not collected) |
| `2024-10-loopfi` | `pairs.json` v1 | judge grouping (not collected) |

Judge grouping is collected the Dup-Bench way (`fetch.py` labels pinned to the sealed findings
commit). These are judge labels, so no human labeller decides a pair outcome. A pair whose labels cannot
be collected is reported as "not measured", never replaced after the fact.

## 4. Metrics (unchanged: analysis plan §2)
recall@5 (abstentions count as misses), FAR at the shipped alarm, warning precision (the alarm stays ON
only if FINAL's Wilson LB is at least 0.80), and abstention by cause. Wilson 95% on every k/n, n < 5 as a count, and the
contest-clustered bootstrap (10,000 draws) on pooled cells. Prose anchors stay off. Each pair is
reported per contest and never pooled with Dup-Bench.

## 5. Held-out plan (after main)
1. Collect the pair labels (§3), commit them with hashes, and freeze the Round-1 binary.
2. Round 1: run the six V1 pairs once (`pairs_run.py --mode pairs` with real round, version and sha).
3. There is no V2 slice; Round 2 is reported as "no fresh slice exists".
4. FINAL: Dup-Bench FINAL (17 contests), final frozen binary, once.
5. Never: tuning or recalibrating on V1 or FINAL, a `--check` with placeholders on a sealed split, or
   pooling pairs with Dup-Bench or DEV with held-out.