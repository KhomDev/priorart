# Search log — Dup-Bench v1

Collected 2026-11-25 by the P2R researcher (Workstream R, Phase 3). `gh api`
calls were spaced >= 7 s; repo clones are the bulk and do not consume the API
budget. Pointers plus hashes only; the text lives in the gitignored
`.corpus/dupbench/`.

## Result, uncomfortable part first

- **111 contests pinned and all 111 labeled.** 1,102 paid-unique findings,
  3,454 judged duplicates, 780 invalid, 101 contests with at least one
  duplicate group; report to data join **1,102/1,102**. Span
  **2022-11-03..2024-12-10** (25 months).
- **The original split is unfillable.** Validation [2025-09-01, 2026-05-01) and
  FINAL [2026-05-01, inf) are empty: the newest `code-423n4/*-findings` repo is
  2024-12 — 66 C4 contest repos for 2025-2026 exist, **none with a public
  findings repo** — and the newest `sherlock-audit/*-judging` repo is 2025-07.
  Under the original dates all 111 contests are train.
- **Amendment 1** (`eval/P2R_PREREGISTRATION_AMENDMENT_1.md`, committed
  2026-11-26 before any PA-B4 work) fixes T=2024-03-01, T2=2024-09-01: train 58
  contests / 564 paid / 1,237 duplicates, validation 36 / 380 / 1,441, FINAL 17
  / 158 / 776. The FINAL slice meets both minima and is sealed.

## Sources searched

| Class | What was done | Result |
|---|---|---|
| Code4rena findings repos | Listed all 881 `code-423n4` repos; 376 are `*-findings`. Cloned 210. Parsed every `report.md` and `data/*.json`. | 2,457 H/M report sections; 111 contests selected and pinned. |
| Code4rena duplicate labels | GitHub issue labels per contest (`duplicate-N`, `primary issue`, `selected for report`, `2 (Med Risk)`, `3 (High Risk)`, `invalid`, `out of scope`). | 111 contests labeled; grouping verified on 3 contests. |
| Code4rena 2025-2026 | 66 contest repos (2025-01..2026-07) exist; **none has a public `-findings` repo**, so no duplicate labels. | 0 contests in the preregistered validation/FINAL. |
| Sherlock judging repos | Cloned `2024-11-hats-protocol-judging`; `comments.csv` is empty, issue files carry severity only, README lists final findings. Newest judging repo is 2025-07. | Not usable for Dup-Bench grouping; 0 contests. |
| Cantina / CodeHawks / Hats / Immunefi | Not reached for Dup-Bench (Cantina findings private; CodeHawks live Vercel-checkpointed; no bulk duplicate-label repos of this shape). | 0. |

## The join and the grouping (verified before counting)

`report.md` sections link to findings-repo issues (`/issues/N`). `data/*.json`
carries `issueId`, `risk`, `title`, `originalIssueId`.

- Report to data join: **1,102 / 1,102** sections across the 111 contests match
  a data `issueId`. Spot checks: Wildcat 9/9, Salty 37/37, Frankencoin 21/21.
- `originalIssueId` is the validation-repo issue, **not** a duplicate group
  (Wildcat: 103 groups for 107 High/Medium submissions, sizes 1-2, while the
  report has 9 unique findings and the labels show 46 duplicate-labelled
  issues). Counting it as grouping would be wrong.
- The judge's grouping is the findings-repo issue label `duplicate-N`, pointing
  at the `selected for report` issue N (Wildcat: `duplicate-11` x5,
  `duplicate-62` x5, 17 duplicates pointing at the nine H/M primaries).
  `build_manifest.py` uses exactly that rule and records
  `duplicates_pointing_at_qa_or_missing` for labels whose target is not an H/M
  primary.

## Counts by split (Amendment 1: T=2024-03-01, T2=2024-09-01)

| Split | Contests | Paid-unique | Duplicates | Invalid |
|---|---:|---:|---:|---:|
| train (< 2024-03-01) | 58 | 564 | 1,237 | 588 |
| validation [2024-03-01, 2024-09-01) | 36 | 380 | 1,441 | 176 |
| FINAL (>= 2024-09-01) | 17 | 158 | 776 | 16 |

The original T=2025-09-01/T2=2026-05-01 put all 111 contests in train
(1,102 / 3,454 / 780) and left validation and FINAL empty.

Date coverage: 2022-11 (1), 2023-01..2023-12 (2-7 per month), 2024-01..2024-12
(2-11 per month); 25 months, no month after 2024-12.

## Split proposal (adopted by Amendment 1)

All 111 labeled contests, candidates inside the available span. Format is
contests / paid-unique / duplicates.

| T | T2 | train | validation | FINAL | FINAL meets >=60 paid and >=300 dups |
|---|---|---|---|---:|---|
| **2024-03-01** | **2024-09-01** | **58 / 564 / 1,237** | **36 / 380 / 1,441** | **17 / 158 / 776** | **yes** |
| 2024-06-01 | 2024-10-01 | 78 / 792 / 2,022 | 20 / 202 / 977 | 13 / 108 / 455 | yes |
| 2024-01-01 | 2024-07-01 | 50 / 487 / 1,156 | 33 / 338 / 955 | 28 / 277 / 1,343 | yes |
| 2023-12-01 | 2024-06-01 | 46 / 450 / 1,035 | 32 / 342 / 987 | 33 / 310 / 1,432 | yes |
| 2023-09-01 | 2024-03-01 | 31 / 293 / 636 | 27 / 271 / 601 | 53 / 538 / 2,217 | yes |
| 2023-07-01 (fallback) | 2023-09-01 | 19 / 187 / 399 | 12 / 106 / 237 | 80 / 809 / 2,818 | yes |

**Adopted by Amendment 1:** T=2024-03-01, T2=2024-09-01. It gives a 58-contest
train (564 paid-unique, 1,237 duplicates) for PA-B4, a 36-contest validation
slice for hyperparameters and calibration, and a sealed FINAL of 17 contests
(158 paid-unique, 776 duplicates) — both FINAL minima met with margin. The
manager's fallback T=2023-07-01/T2=2023-09-01 also meets them but leaves a
19-contest train and puts 72% of the contests in FINAL; it was not chosen. The
dates were chosen from judging dates and label counts only, before any model
work; see the amendment for the full statement.

## Sealed pairs

| Pair | Sponsor | Prior report (sha256 prefix) | Later contest |
|---|---|---|---|
| basin-2023 | Basin | cyfrin-basin-audit.pdf (fcbc731d) | code-423n4/2023-07-basin-findings |
| revert-lend-2024 | Revert Finance | HYDN Revert Finance Vault Audit (37fe72b3) | 2024-03-revert-lend-findings |
| size-2024 | Size | LightChaser 2024-03-19 (77db9f0c), Solidified alongside | 2024-06-size-findings |
| ethena-2024 | Ethena Labs | Quantstamp final report (471417c5) | 2024-11-ethena-labs-findings |
| loopfi-2024 | LoopFi | LoopFi July Provisional Report (93f17b2d) | 2024-10-loopfi-findings |
| salty-2024 | Othernet Global / Salty | ABDK SaltyIO v2.0 (45f991b3) | 2024-01-salty-findings |

Six sponsors, none of Party, Wildcat, PoolTogether, Tapioca, Maia, Panoptic.
Same-codebase evidence is the sponsor's own "Previous audits" line in the
contest README and, for four of six, the report shipped in the contest repo;
the prior report's commit is not stated, so the drift is declared per pair
rather than proved.

## Excluded candidates

| Candidate | Why |
|---|---|
| 2025-2026 Code4rena contests (66 repos) | no public `-findings` repo, so no public duplicate grouping |
| Sherlock judging repos after 2025-07 | none exist |
| Sherlock 2024-11 Hats Protocol | no duplicate metadata in the files (empty comments.csv) |
| 61 contests with > 500 curated submissions in the window | excluded to bound the label-API budget (documented selection rule) |
| Wise Lending prior audit | `app.wiselending.com/omni-audit-v1.pdf` does not resolve |
| Superposition prior audits | `fluidity-money/long.so` audits/ holds only LFS pointer stubs |

## Blockers

- No public judged repository after 2024-12 (C4) / 2025-07 (Sherlock): the
  preregistered temporal split cannot be populated. This is the one blocker
  that a corpus cannot fix.
- The collection environment lacks PyYAML, so the bulk format is documented
  JSON instead of the YAML convention.
- Windows long paths break a full clone of some contest repositories
  (`discord-export/`); only `report.md` + `data/` are needed and were copied.
- Labeling all 111 contests took about two hours at the 7 s-per-call floor;
  it completed (exit 0), it did not time out.

## Verification

`build_manifest.py --check` matches `manifest.json`, `labels.json` and
`cache.lock.json`. `fetch.py --check` verifies every contest's report hash,
data listing hash and label-page hashes offline (111 pin sets). A random 10%
re-fetch is left to the orchestrator.
