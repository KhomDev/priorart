# Dup-Bench v1 — bulk judged-contest corpus (sealed temporal split)

Collected and pinned by the P2R researcher (§3 of
`_orchestration/PROMPT_PRECISION_REBUILD_2026-11-25.md`). This README, the
search log and the pins are committed before any `p2/*` code reads a contest.

## Status, with the blocker first

**111 Code4rena contests are pinned and all 111 have their judge duplicate
labels collected**, spanning **2022-11-03 to 2024-12-10** (25 months):
**1,102 paid-unique findings, 3,454 judged duplicates, 780 invalid**, across
101 contests with at least one duplicate group. The report↔data join is
**1,102/1,102**.

**The original split (T = 2025-09-01, T2 = 2026-05-01) is unfillable** because
no public judged repository exists after 2024-12 (C4; 66 contest repos for
2025-2026, none with a public findings repo) or 2025-07 (Sherlock).
**Amendment 1** (`eval/P2R_PREREGISTRATION_AMENDMENT_1.md`, committed
2026-11-26 before any PA-B4 work) fixes **T = 2024-03-01, T2 = 2024-09-01**:
train 58 contests / 564 paid / 1,237 duplicates, validation 36 / 380 / 1,441,
FINAL 17 / 158 / 776. The FINAL slice meets both minima (≥ 60 paid, ≥ 300
duplicates) and is sealed: no runner, matcher, ranker or reader has consumed it.

## What a contest row records

`manifest.json`: slug, repo, commit, report date, split, paid-unique,
duplicates, invalid, label counts, report↔data join. `labels.json`: the judge's
primaries and duplicate lists as issue numbers only. `cache.lock.json`:
per-contest report sha256, data listing hash, label-page sha256. No finding body
is committed.

## Bulk format (documented JSON, not YAML)

The collection environment has no PyYAML, so the machine format is JSON:
`sources.json` (contests, commits, dates), `manifest.json`, `labels.json`,
`cache.lock.json`, `pairs.json`. Field names and schemas are versioned in each
file and consumed by `eval/dupbench/build_manifest.py`; the PA-B3 builder can
read them with any JSON parser.

## The join, verified

`report.md` H/M sections link to findings-repo issues (`/issues/N`); each
`data/*.json` carries `issueId`, `risk`, `title` and `originalIssueId`. Across
all 111 contests **1,102 of 1,102 report sections match a data `issueId`**.
`originalIssueId` is **not** the duplicate grouping (on Wildcat it yields 103
groups for 107 High/Medium submissions, sizes 1–2); the judge's grouping is the
GitHub issue labels (`duplicate-N` points at the `selected for report` issue
N). The label grouping gives 3,454 duplicates for 1,102 paid-unique findings.

Sherlock: the judging repo files carry no duplicate metadata (`comments.csv`
empty, issue files carry a severity line only, the final report lists the
findings). The Sherlock source is therefore documented as unusable for
Dup-Bench without its GitHub issue labels.

## Codebase pins (input for PA-B3/PA-B4)

`codebases.json` pins the audited codebase for every contest, pointer-only:

| Split | Contests | repo+commit | repo only | missing |
|---|---:|---:|---:|---:|
| all | 111 | 106 | 0 | 5 |
| train | 58 | 54 | 0 | 4 |
| validation | 36 | 35 | 0 | 1 |
| FINAL | 17 | 17 | 0 | 0 |

The repo is the Code4rena sponsor snapshot (`https://github.com/code-423n4/<slug>`), which is the exact code under review; 108 of 111 report `# Scope` sections name it and the other 3 are inferred from the contest slug (recorded as `repo_source`). The commit is that repository's **HEAD at collection** — the findings README and the report front matter state no commit — and `source` says so. The 5 missing entries (`2022-12-Stealth-Project`, `2023-03-polynomial`, `2023-05-particle`, `2023-11-betafinance` in train; `2024-04-lavarage` in validation) record `commit: null` with the reason: the sponsor repo is not publicly reachable. Each entry also carries the findings-repo commit and the findings `README.md` sha256 (111/111); `fetch.py --check` verifies the README pins offline. The commit itself is re-verifiable with `git ls-remote` and is not claimed to be a commit stated by Code4rena.

## Sealed pairs

`pairs.json`: 6 pairs, 6 sponsors, none used before (Basin, Revert Finance,
Size, Ethena Labs, LoopFi, Othernet Global / Salty). Each pins the prior public
audit report by URL and sha256, the later contest's findings commit, and the
contest codebase snapshot repo+commit. The prior report's commit is not stated
in the READMEs, so same-codebase evidence is the sponsor's own "Previous
audits" statement and the drift is stated per pair.

**Overlap with Dup-Bench (stated, not hidden).** Four of the six pair contests are
also Dup-Bench contests: `2023-07-basin` is in train, `2024-01-salty` in
validation, and `2024-06-size` and `2024-11-ethena-labs` in FINAL. Only
`2024-03-revert-lend` and `2024-10-loopfi` are outside Dup-Bench, and neither has
collected judge labels. A pairs measurement on train/validation contests is
in-sample for the ranker and alarm; FINAL contests are refused until the FINAL
freeze. `pairs_run.py` enforces both.

**Swap (Yura, 2026-11-27): not yet possible.** The four overlapping contests
are to be replaced by judged contests outside Dup-Bench that no run has opened,
picked from metadata only and sealed before any finding is read. The rule,
both screens and their per-contest records are in
[`pairs-swap/SELECTION.md`](pairs-swap/SELECTION.md). **No contest passes
(0 of 140 strict, 0 of 91 relaxed), so nothing is sealed yet**, and relaxing a
criterion is Yura's call (options A-C there). The pairs measurement does not
run until labels exist.

`pairs_run.py` runs `priorart candidates --top-k 5 --ranker weights/v1.json
--alarm weights/alarm-v1.json --json` (prose anchors off) per judged duplicate
and primary and reports recall@5 (abstentions count as misses), false-alarm rate
at the shipped alarm, warning precision and abstention, each with a Wilson 95%
interval, per group and never pooled across groups. `--mode dev` runs Dup-Bench
train contests only. `--mode pairs` needs `--round`, `--version` and
`--binary-sha256` equal to the sha256 of `--bin` (placeholders are refused),
refuses FINAL contests, reports train/validation contests as in-sample groups,
and needs `--pair-labels` for contests outside Dup-Bench. `--check FILE` requires
byte-identical output.

## PA-B4 learned ranker (train and validation only)

`build_ranker_data.py` builds the dataset: for each train/validation contest
with a checkout at its pinned commit, it builds (or reuses, only when the
recorded commit is the pin) the anchors index, prepares the prior corpus from
the contest's paid-unique primaries, and runs `priorart candidates` for every
judged duplicate (label 1 for its primary, 0 otherwise) and every primary
against the other primaries (label 0). Each row records the raw feature
vector, query id, candidate id, contest, split and label; an abstained query
is recorded with its reason and is never turned into a negative. The dataset
is written to a gitignored `target/ranker-data/` path; the issue texts never
leave the gitignored cache and output. FINAL is never opened.

`train_ranker.py` fits an L2-regularised logistic regression (pure standard
library, deterministic Newton/IRLS) on **train only**, chooses the
regularisation strength and the count transform on **validation only**, and
writes `weights/v1.json`: schema, feature names and transforms, means/scales,
coefficients, intercept, seed, train/validation contest ids, the selection
grid, the validation decile calibration table, the split measurements and the
code commit. Inference is one dot product per candidate
(`priorart candidates --ranker weights/v1.json`); a null `call_graph_distance`
maps to `MAX_CANDIDATE_DISTANCE + 1` with a missingness indicator feature,
never a silent zero. Without `--ranker`, output is unchanged.

## Files

| Path | What it is |
|---|---|
| `sources.json` | 111 contests with findings-repo commit, report date, curated count |
| `manifest.json` | per-contest counts and split under the amended T/T2 |
| `labels.json` | judge grouping: primaries and duplicate issue numbers |
| `cache.lock.json` | per-contest report/data/label-page pins |
| `pairs.json` | 6 sealed pairs with prior-report hashes and codebase pins |
| `build_manifest.py` | rebuilds the three corpus files offline; `--check` |
| `fetch.py` | fetches every pin; `--check` verifies offline |
| `clone_codebases.py` | clones the pinned audited codebases per split |
| `build_ranker_data.py` | PA-B4 dataset builder (train + validation; FINAL refused) |
| `train_ranker.py` | PA-B4 trainer, selection and measurement; writes `weights/v1.json` |
| `pairs_run.py` | sealed-pairs / DEV runner (FINAL refused; in-sample groups labelled) |
| `weights/v1.json` | the committed, versioned ranker table read by `--ranker` |

## Reproduce

```bash
python eval/dupbench/fetch.py            # clones + label pages (network, >=7 s per API call)
python eval/dupbench/build_manifest.py   # writes manifest.json, labels.json, cache.lock.json
python eval/dupbench/fetch.py --check    # offline
python eval/dupbench/build_manifest.py --check
```
