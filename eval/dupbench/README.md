# Dup-Bench v1 — bulk judged-contest corpus (sealed temporal split)

Collected and pinned by the P2R researcher (§3 of
`_orchestration/PROMPT_PRECISION_REBUILD_2026-11-25.md`). This README, the
search log and the source pins are committed before any `p2/*` code reads a
contest.

## Status

**Collecting.** No contest is sealed or labelled yet; the split is fixed in
`eval/P2R_PREREGISTRATION.md` (T = 2025-09-01, T2 = 2026-05-01).

## What a contest row records

- repo (for example `code-423n4/2024-01-foo-findings` or
  `sherlock-audit/2024-01-foo-judging`);
- the judging commit;
- the label source (C4 issue labels or Sherlock judging outcome);
- per-contest counts: accepted (paid), unique, duplicates, invalid;
- the pinned files (paths + sha256) in `sources.yaml` / `cache.lock.json`.

No finding body is committed. Bodies are fetched into the gitignored cache by
`fetch.py`.

## Target

≥ 100 contests with public duplicate grouping, spanning ≥ 24 months, with
enough paid-unique findings and judged duplicates after T2 to meet the FINAL
minimum n in the preregistration. A contest that publishes labels but no
grouping (no duplicate-N or equivalent) is excluded with a reason.

## Split

Fixed in `eval/P2R_PREREGISTRATION.md` §2: train before T; validation [T, T2);
FINAL at/after T2 (sealed). A contest's split is assigned by its judging close
date, never by its findings' content.

## Rules

- `fetch.py --check` verifies every pin offline before a round opens.
- The six existing corpora (`tapioca`, `pooltogether`, `maia-c4`,
  `panoptic-c4`, `party-c4`, `wildcat-c4`) are DEV and are not reused.
- A "nothing more exists" claim needs the search log behind it.
