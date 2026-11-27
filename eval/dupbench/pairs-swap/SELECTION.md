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
   findings repos; sha256 of the committed LF file `e7e7384d7a54125ce906a0a9e0054f1b7349f02d3792229031697ab28de359d0`).
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
## Option A (Yura, 2026-11-27): rule 6 dropped; 4 contests sealed

Yura chose **A**: drop rule 6 (the 500-issue budget). Everything else stays.
`screen_optA.py` is `screen_relaxed.py` without rule 6 and without the early
stop, so every one of the 91 relaxed-pool candidates has a record in
`screen_optA.json`. Without the cap, 8 candidates pass rules 1-7. That changes the
earlier count of 11 with no prior-audit file: the cap ran before that check, so
most capped contests were never checked. 43 now fail rule 5. Of the five
contests named in option A, `2023-07-axelar` and `2022-12-caviar` fail rule 5,
`2023-05-ajna` and `2022-10-traderjoe` fail rule 5a below, and only
`2022-11-paraspace` is sealed.

**Rule 5a (correction added after the metadata screen, before any finding was
read).** Four of the 8 passed rule 5 only through audit files of vendored
libraries (`lib/openzeppelin-contracts/audit/*`, `lib/solmate/audits/*`). Those
are audits of a dependency, not a prior audit of the same codebase, which is
what rule 5 is for. Rule 5a drops paths under `lib/`, `node_modules/`,
`dependencies/` or `vendor/`. This removes `2024-04-dyad`, `2022-10-traderjoe`,
`2022-12-gogopool` and `2023-05-ajna`. It only removes candidates and never
adds one. It is stated here because it was written after the screen output was
seen. `seal_optA.py` applies it and re-ranks by rule 8.

| Rank | Contest | Role | Issues | `duplicate-N` labels | Prior-audit file(s) in the code repo |
|---:|---|---|---:|---:|---|
| 1 | `2022-11-paraspace` | sealed | 516 | 59 | `paraspace-core/audits/CERTIK-V1.pdf` |
| 2 | `2023-02-ethos` | sealed | 860 | 38 | `Ethos_Audit_Report-2B885.pdf` (two copies, inside the sponsor's `discord-export/`; weakest evidence of the four) |
| 3 | `2023-09-venus` | sealed | 684 | 36 | `audits/055_prime_peckshield_20230826.pdf`, `audits/056_prime_fairyproof_20230910.pdf` |
| 4 | `2023-07-arcade` | sealed | 573 | 64 | `audits/Omniscia2023.pdf` |

No backups remain: exactly 4 contests pass, so all 4 are sealed.

**Seal.** `sealed_optA.json` pins each contest's findings repo commit, the sha256 of
the findings tree listing (path and blob only), the `report.md` git blob, the
code-repo commit, and each prior-audit file's git blob, byte count and sha256.
The bytes were hashed and never parsed or printed. No issue body, `report.md`,
`data/*.json` or audit content was read. sha256 of the committed LF
`sealed_optA.json`: `d2af4796618a1ba48411f72017e8713fec35067217268e256236878eb97940a7`.

**Labelling these need before any pairs measurement:**
1. Judge grouping, collected the Dup-Bench way (`fetch.py`, pinned to the sealed
   findings commit): per issue, the `duplicate-N`, `selected for report`,
   severity and invalid/withdrawn labels. These give `labels.json`
   (`primaries` and `duplicates`) and the `<slug>/page-*.json` issue pages that
   `pairs_run.py --pair-labels DIR` reads. That is 2,633 issues across the four.
2. Prior-report pins in a `pairs.json` v2: for each pair, a public URL for the
   prior audit, checked against the sealed sha256, and the same-codebase
   evidence. For `ethos`, the only copy is a Discord attachment, so the
   evidence has to be stated as weak or the pair dropped.
3. The same for the kept `2024-03-revert-lend` and `2024-10-loopfi`, which have no
   judge labels collected yet.

No pairs measurement runs until (1) and (2) exist. The label collection itself
reads only labels, but it is the step that opens these contests, so it happens
after this seal and is logged.