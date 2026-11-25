# Dup-Bench v1 — bulk judged-contest corpus (sealed temporal split)

Collected and pinned by the P2R researcher (§3 of
`_orchestration/PROMPT_PRECISION_REBUILD_2026-11-25.md`). This README, the
search log and the pins are committed before any `p2/*` code reads a contest.

## Status, with the blocker first

**111 Code4rena contests are pinned** (findings repo at a commit, `report.md`,
`data/` listing) spanning **2022-11-03 to 2024-12-10**. **46 of them have their
duplicate labels collected**: 450 paid-unique findings, 1,035 judged duplicates,
523 invalid, across 41 contests with at least one duplicate group. Every one of
the 46 sits in the **train** window.

**Validation and FINAL are empty, and not only because label collection
stopped at the earliest 46 contests.** The last public `code-423n4/*-findings`
repo is 2024-12; the last `sherlock-audit/*-judging` repo is 2025-07. The
preregistered **T = 2025-09-01 and T2 = 2026-05-01 therefore cut two slices with
no public judged repository at all**. The FINAL minima (≥ 60 paid-unique,
≥ 300 duplicates) cannot be met on any corpus built from public duplicate
grouping today. A re-split inside the available span would need a new
preregistration; the numbers are in `SEARCH_LOG.md`.

## What a contest row records

`manifest.json`: slug, repo, commit, report date, split, paid-unique,
duplicates, invalid, label counts, report↔data join, and `labels_status`.
`labels.json`: the judge's primaries and duplicate lists as issue numbers only.
`cache.lock.json`: per-contest report sha256, data listing hash, label-page
sha256. No finding body is committed.

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
N). On three contests the label grouping gives 1,035 duplicates for 450 unique
findings across the 46 labeled contests.

Sherlock: the judging repo files carry no duplicate metadata (`comments.csv`
empty, issue files carry a severity line only, the final report lists the
findings). The Sherlock source is therefore documented as unusable for
Dup-Bench without its GitHub issue labels.

## Sealed pairs

`pairs.json`: 6 pairs, 6 sponsors, none used before (Basin, Revert Finance,
Size, Ethena Labs, LoopFi, Othernet Global / Salty). Each pins the prior public
audit report by URL and sha256, the later contest's findings commit, and the
contest codebase snapshot repo+commit. The prior report's commit is not stated
in the READMEs, so same-codebase evidence is the sponsor's own "Previous
audits" statement and the drift is stated per pair.

## Files

| Path | What it is |
|---|---|
| `sources.json` | 111 contests with findings-repo commit, report date, curated count |
| `manifest.json` | per-contest counts and split, with `labels_status` |
| `labels.json` | judge grouping: primaries and duplicate issue numbers |
| `cache.lock.json` | per-contest report/data/label-page pins |
| `pairs.json` | 6 sealed pairs with prior-report hashes and codebase pins |
| `build_manifest.py` | rebuilds the three corpus files offline; `--check` |
| `fetch.py` | fetches every pin; `--check` verifies offline |

## Reproduce

```bash
python eval/dupbench/fetch.py            # clones + label pages (network, >=7 s per API call)
python eval/dupbench/build_manifest.py   # writes manifest.json, labels.json, cache.lock.json
python eval/dupbench/fetch.py --check    # offline
python eval/dupbench/build_manifest.py --check
```
