# Pairs swap: selection rule, screen and outcome (2026-11-27)

Decision (Yura, 2026-11-27): swap out the four pair contests that are also
Dup-Bench contests: `2023-07-basin` (train), `2024-01-salty` (validation),
`2024-06-size` and `2024-11-ethena-labs` (FINAL). The replacements must be
judged Code4rena contests that are not in Dup-Bench (any split) and that no
earlier run has opened. They are picked from metadata only, then sealed before
anyone reads a finding. `2024-03-revert-lend` and `2024-10-loopfi` are outside
Dup-Bench and stay as long as they can be labelled the same way.

## Rule (fixed before the screen ran)

Metadata only: repository names, tree path names, label names, issue counts.
No issue body, `report.md` or `data/*.json` content is read.

1. Pool: public `code-423n4` repositories named `YYYY-MM-<slug>-findings` that
   also have a `<slug>` code repository (`code-423n4-repos.tsv`; 881 repos, 376
   findings repos; sha256 `edb183ad…c40a`).
2. Exclude the 111 Dup-Bench contests and the 6 `pairs.json` contests.
3. Exclude anything an earlier run may have opened. The Dup-Bench search log
   says 210 findings repos were cloned and their `report.md` parsed, but it
   kept no list. 207 findings repos are dated 2022-11..2024-12, so that whole
   window counts as opened, and the strict screen only takes contests dated
   2022-07 or earlier.
4. Judged, with a public grouping: the findings tree has `report.md`, and the
   labels include `selected for report` and at least one `duplicate-<N>`.
5. A prior audit of the same codebase: the code repository's tree has a file
   whose path matches `/audit/i` and ends in `.pdf` or `.md` (not `README.md`).
6. A labelling budget of at most 500 issues (the Dup-Bench rule).
7. The sponsor is not in `sponsors_used_before`, and its first slug token is
   not the first token of any Dup-Bench or pairs contest.
8. Order: `sha256('p2r2-pairs-swap-2026-11-27|' + slug)`, ascending. Take the
   first 4 that pass, plus 2 backups.

## Outcome: no contest qualifies, so nothing is sealed

| Screen | Pool | Accepted | Main reasons |
|---|---:|---:|---|
| Strict (`screen.py`, contests <= 2022-07) | 140 | **0** | 124 have no `duplicate-<N>` labels (the older contests use a single `duplicate` label, and their grouping lives only in issue comments, which would mean reading findings); 9 share a Dup-Bench sponsor token; 6 have a sponsor used before; 1 has no prior-audit file |
| Relaxed (`screen_relaxed.py`, 2022-08..2024-12, no mitigation reviews; this window was bulk-parsed earlier) | 91 | **0** | 40 have more than 500 issues; 11 have no prior-audit file; 15 have no grouping labels; 24 share a sponsor (16 by token, 8 used before); 1 has no `report.md` |

The two screens' per-contest records are `screen.json` and
`screen_relaxed.json`. Rules 4 to 7 together exclude every candidate, which is
also why these contests were left out of Dup-Bench in the first place.

To go on, one of these has to be relaxed. That is Yura's call:

- **A.** Drop rule 6, the 500-issue budget. Five candidates sit just above it
  (`2023-07-axelar` 506, `2023-05-ajna` 508, `2022-12-caviar` 509,
  `2022-11-paraspace` 516, `2022-10-traderjoe` 519). Their prior-audit files
  have not been checked yet. They fall in the window the researcher bulk-parsed
  for counts, but none was ever labelled or used by a matcher, ranker or
  measurement.
- **B.** Replace rule 5 with the sponsor's own "Previous audits" line in the
  contest README. That reads a sponsor README, not findings.
- **C.** Keep only `revert-lend` and `loopfi`, after collecting their labels
  the Dup-Bench way, and report the pairs as n=2 contests (count only).

No pairs measurement runs until labels exist.