# Search log — Dup-Bench v1

Collected 2026-11-25 by the P2R researcher (Workstream R, Phase 3). `gh api`
calls were spaced >= 7 s; repo clones are the bulk and do not consume the API
budget. Pointers plus hashes only; the text lives in the gitignored
`.corpus/dupbench/`.

## Result, uncomfortable part first

- **111 contests pinned; 46 labeled.** 450 paid-unique findings, 1,035 judged
  duplicates, 523 invalid, 41 contests with at least one duplicate group.
- **All 46 labeled contests are in train.** Validation [2025-09-01, 2026-05-01)
  and FINAL [2026-05-01, inf) are **empty**.
- Two independent causes, both recorded:
  1. **No public judged repository exists in those windows.** The newest
     `code-423n4/*-findings` repo is 2024-12 (there are 66 C4 contest repos for
     2025-2026, but **zero have a public `-findings` repo**); the newest
     `sherlock-audit/*-judging` repo is 2025-07. The preregistered T/T2 cut
     slices that no public duplicate grouping fills.
  2. **Label collection stopped at the earliest 46 of 111** before the budget
     ran out (the API calls on this host averaged far above the 7 s floor).
     The fetcher is resumable; the remaining 65 contests are pinned but
     `labels_status: not-collected` and are excluded from every count.
- The FINAL minima (>= 60 paid-unique, >= 300 duplicates) are exceeded by the
  labeled set as a whole (450/1,035) but **not by any FINAL slice, because
  there is none**.

## Sources searched

| Class | What was done | Result |
|---|---|---|
| Code4rena findings repos | Listed all 881 `code-423n4` repos; 376 are `*-findings`. Cloned 210. Parsed every `report.md` and `data/*.json`. | 2,457 H/M report sections; 111 contests selected and pinned (span 2022-11..2024-12). |
| Code4rena duplicate labels | GitHub issue labels per contest (`duplicate-N`, `primary issue`, `selected for report`, `2 (Med Risk)`, `3 (High Risk)`, `invalid`, `out of scope`). | 46 contests labeled; grouping verified on 3 contests. |
| Code4rena 2025-2026 | 66 contest repos (2025-01..2026-07) exist; **none has a public `-findings` repo**, so no duplicate labels. | 0 contests in validation/FINAL. |
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

## Counts by split (labeled contests only)

| Split | Contests | Paid-unique | Duplicates | Invalid |
|---|---:|---:|---:|---:|
| train (< 2025-09-01) | 46 | 450 | 1,035 | 523 |
| validation [2025-09-01, 2026-05-01) | 0 | 0 | 0 | 0 |
| FINAL (>= 2026-05-01) | 0 | 0 | 0 | 0 |

The 46 labeled contests span 2022-11-03..2023-11-21 (label collection is in
source order, so the early contests were labeled first). The full pinned set of
111 spans 2022-11-03..2024-12-10 (25 months).

## Split arithmetic if the preregistration were reopened

With the 46 labeled contests, a split inside their span that gives all three
slices (computed, not applied):

| T | T2 | train (c/paid/dups) | validation | FINAL |
|---|---|---|---|---|
| 2023-01-01 | 2023-06-01 | 1 / 33 / 0 | 13 / 94 / 190 | 32 / 323 / 845 |
| 2023-02-01 | 2023-07-01 | 2 / 36 / 9 | 17 / 151 / 390 | 27 / 263 / 636 |
| 2022-12-01 | 2023-04-01 | 1 / 33 / 0 | 6 / 30 / 53 | 39 / 387 / 982 |

None has a usable train slice: the labeled 46 are the earliest contests and are
dominated by 2023. Collecting the remaining 65 labels (2023-11..2024-12) is the
prerequisite for any split with a real train and a real FINAL. That work is
resumable with `fetch.py`; it is not claimed here.

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
| 65 pinned contests without labels | `labels_status: not-collected`; not counted |
| Wise Lending prior audit | `app.wiselending.com/omni-audit-v1.pdf` does not resolve |
| Superposition prior audits | `fluidity-money/long.so` audits/ holds only LFS pointer stubs |

## Blockers

- No public judged repository after 2024-12 (C4) / 2025-07 (Sherlock): the
  preregistered temporal split cannot be populated.
- Label API pace on this host: 46 contests in roughly 90 minutes of wall clock
  (far above the 7 s-per-call floor); 100+ contests did not fit the budget.
- The collection environment lacks PyYAML, so the bulk format is documented
  JSON instead of the YAML convention.
- Windows long paths break a full clone of some contest repositories
  (`discord-export/`); only `report.md` + `data/` are needed and were copied.

## Verification

`build_manifest.py --check` matches `manifest.json`, `labels.json` and
`cache.lock.json`. `fetch.py --check` verifies every contest's report hash,
data listing hash and label-page hashes offline. A random 10% re-fetch is left
to the orchestrator.
