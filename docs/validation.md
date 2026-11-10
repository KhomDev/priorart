# Validation ledger

This page separates what this repository has **reproduced** from what it has only
**claimed**. It exists because the 2026-11-06 audit found that earlier releases
had shipped claims the code did not support, and the 2026-11-10 review found the
same pattern in the precision story. A claim is listed here only with a command
or a file that a reader can run in this checkout.

Status values:

- **validated** - a test or a committed result file reproduces the claim.
- **unvalidated** - the claim is stated as a limit, not as a result; no
  measurement exists yet.
- **retracted** - the claim was made in an earlier release and is no longer
  true or was never true.

## Claims

| # | Claim | Evidence (run in this checkout) | Date | Status |
|---|---|---|---|---|
| 1 | `CLEAR` is reachable when every gate runs and passes | `cargo test --test cli clear_is_reachable_when_every_gate_runs_and_passes`; D1 runs from `--known-issues` | 2026-11-06 | validated |
| 2 | A failed gate holds the finding; a failure outranks an unknown gate | `cargo test --lib a_failed_gate_holds_the_finding` | 2026-11-06 | validated |
| 3 | G1 compares the thesis severity with the floor | `cargo test --test rules`; `tests/rules.fixture.yaml` G1 cases | 2026-11-06 | validated |
| 4 | G2 runs each exclusion pattern over the finding's claim sections | `cargo test --test rules`; G2 fixture cases | 2026-11-06 | validated |
| 5 | G3 traces cited paths and file names into the scope list | `cargo test --test rules`; G3 fixture cases | 2026-11-06 | validated |
| 6 | G4 compares the finding with the ledger's prior-audit known issues | `cargo test --test rules`; G4 fixture cases | 2026-11-06 | validated |
| 7 | G5 looks for whole-word intent markers only in tests that name an identifier from the finding | `cargo test --test rules`; G5 fixture cases | 2026-11-06 | validated |
| 8 | G7 uses containment, not Jaccard, and an empty log passes | `cargo test --lib` G7 cases; `docs/duplicate-matching.md` | 2026-11-06 | validated |
| 9 | Fenced code blocks do not start sections; a heading needs `#` plus a space | `cargo test --lib hash_lines_inside_a_code_fence_are_not_headings` | 2026-11-06 | validated |
| 10 | A missing cited file or line is a hard `CITE` finding and the score cannot stay 100 | `cargo test --test cli a_fake_citation_does_not_score_100_and_does_not_pass` | 2026-11-09 | validated |
| 11 | On the public pair, a 0.7.0 hard kill was 12/25 (48%) precise and hit 5.5% of paid Medium/High findings | `eval/results/summary.json`; `eval/results/per-item.jsonl`; arithmetic in `docs/benchmark.md` | 2026-11-09 | validated |
| 12 | Since 0.8.0 the default outcome for a claim-heuristic hard rule is ROUTE-TO-VERIFY, and the default hard-kill count on the public pair is 0 | `cargo test --test cli claim_rules_route_by_default_and_strict_restores_the_kill`; `docs/benchmark.md` default table | 2026-11-10 | validated |
| 13 | `--strict` restores the pre-0.8.0 hard stop for claim rules; `KI:` and `CITE` kill in every mode | `cargo test --test cli claim_rules_route_by_default_and_strict_restores_the_kill` | 2026-11-10 | validated |
| 14 | The catalog has 26 rules, P1-P26, each with a positive and a negative fixture | `cargo test --test rules` | 2026-11-10 | validated |
| 15 | The false-positive rate on model-written hypotheses is unmeasured | no control set exists; `docs/control-set.md` | 2026-11-10 | unvalidated |
| 16 | The prospective protocol has been run | `docs/prospective-protocol.md` says NOT RUN | 2026-11-10 | unvalidated |
| 17 | The in-sample strict recall (14/59) has been re-run on the current catalog | `docs/calibration.md` says it has not | 2026-11-10 | unvalidated |
| 18 | A hard kill is a verdict that the finding is invalid | never claimed by the current README; the 0.7.0 default made it the practical reading | 2026-11-10 | retracted |

## The hard-stop policy

A claim-heuristic rule may be a **default** hard stop only when the public pair
shows all of:

1. precision at least 0.75 (the kill is right at least three times in four),
2. at least 20 fires on the paid + invalid/won't-fix pair, and
3. a paid hard-kill rate no higher than 1% of paid findings.

As of 2026-11-10 no rule meets that bar. The best-supported rule, P2, is 0.53
precise over 19 fires and accounts for 9 of the 12 paid hard kills. P19 is 0.00
over 2 fires, P15 is 0.67 over 3, and P14 is 1.00 over 1 (not enough support).
That is why 0.8.0 routes claim rules by default and keeps the hard stop behind
`--strict`.

The bar can be met later. If a rule reaches it on a larger, pre-registered
corpus, the policy allows it to become a default hard stop again; the change
must update this page, `docs/benchmark.md` and the changelog in the same
commit.

## What this page does not do

It does not validate the rules against model-written drafts, and it does not
turn the public contest corpus into a false-positive rate. Those are the two
measurements the project still owes; `docs/control-set.md` and
`docs/prospective-protocol.md` describe how to make them.
