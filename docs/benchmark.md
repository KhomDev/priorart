# Public-corpus benchmark

Re-scored 2026-11-09 with `vg` 0.7.0 on the same manifest `eval/run.py fetch` wrote for 0.6.0 (`eval/dataset.manifest.yaml` did not change). Bodies stay in `eval/cache/`, which is gitignored. This repository commits the manifest and the results, not the reports.

**0.8.0 changes the consequence, not the hits.** The per-rule hits below are unchanged. Since 0.8.0 a claim-heuristic hard rule is ROUTE-TO-VERIFY by default; the hard KILL column is the `--strict` (pre-0.8.0) behaviour. The default table is in the next section. The 2026-11-10 review is why: a hard stop that is wrong 48% of the time and hits 5.5% of paid findings should not be the default.

This is one corpus of judged, human-written contest reports. It is not a property of the rules, and it is not the false-positive rate on model-written hypotheses. The author's own log has no paid findings. See [control-set.md](control-set.md).

## What was scored

`eval/sources.yaml` names 16 Sherlock judging repositories and 8 Code4rena
findings repositories. `eval/run.py` reads GitHub issue labels and bodies. The
selection rule:

| Class | Rule | Kept |
|---|---|---:|
| accepted | Sherlock: High or Medium, and Reward, and not Duplicate. Code4rena: selected for report, and a High or Med label, and not a duplicate, and not downgraded, and not invalid | 110 Sherlock + 110 Code4rena |
| invalid | Sherlock label Invalid, or Code4rena label invalid or out of scope | 80 |
| wontfix | Sherlock label Won't Fix | 40 |
| informative | Sherlock label Low/Info, and not Reward | 60 |
| qa | Code4rena QA, and not selected for report | 60 |

414 accepted issues matched before the per-platform cap of 110. The cap is
applied in repository order, then issue number, so it does not depend on API
order. Code4rena's "unsatisfactory" label is not used: one contest had over a
thousand of them, and they would have been the whole negative class.

Accepted severities in the capped set: 101 High, 119 Medium.

`vg` saw the body and not the label. Labels were applied after.

## Headline

A **hard KILL** is the outcome that would have stopped a submission. 0.7.0, default flags, no ledger, no target tree. A1 is unknown on every row. P26 does not run.

**0.8.0 default** (claim-heuristic hard hits are ROUTE-TO-VERIFY):

| Set | N | ROUTE-TO-VERIFY | Rate | Hard KILL |
|---|---:|---:|---:|---:|
| Paid Medium/High | 220 | 12 | 5.5% | 0 |
| Invalid + won't-fix | 120 | 13 | 10.8% | 0 |
| Code4rena QA | 60 | 11 | 18.3% | 0 |
| Sherlock Low/Info | 60 | 9 | 15.0% | 0 |

**`--strict` / 0.7.0** (the same hits, as hard stops):

| Set | N | Hard KILL | Rate | 0.6.0 |
|---|---:|---:|---:|---:|
| Paid Medium/High | 220 | 12 | 5.5% | 19 (8.6%) |
| Invalid + won't-fix | 120 | 13 | 10.8% | 19 (15.8%) |
| Code4rena QA | 60 | 11 | 18.3% | 14 (23.3%) |
| Sherlock Low/Info | 60 | 9 | 15.0% | 10 (16.7%) |

Of the 25 default routes on paid or invalid/won't-fix, 12 are paid findings and 13 are invalid/won't-fix. The paid share is **12/25 (48%)**; the precision of "this route marks a finding the platform did not pay" is **13/25 (52%)**. The base rate of invalid/won't-fix in that pair is 120/340 (35%). The route rate on the unpaid side divided by the route rate on the paid side is 10.8 / 5.5 ≈ **2.0**. On 0.6.0 that ratio was 15.8 / 8.6 ≈ 1.8, and the precision was 19/38 (50%).

Softening P3 moved paid hard kills from 19 to 12 and invalid hard kills from 19 to 13. The separator is still weak. A route is not evidence the finding is invalid, and a hard kill is not either. Do not read 5.5% as "the tool is safe 94% of the time." Twelve paid findings are routed, and under `--strict` they would have been hard-killed.

Which rules did the killing, on the paid set: **P2 on 9, P19 on 2, P15 on 1.** P3 fired on 7 paid findings and on 7 invalid/won't-fix findings, and **none of those fires were hard**. The 0.6.0 paid kills that were P3 (7) are holds now. P2 did not move: still 9 paid and 10 invalid/won't-fix, precision 0.53. The negation fix and the unprivileged-yield suppression are covered by fixtures. This corpus does not contain those wordings, so the bench cannot show them.

P18 fired on **0** of 460 items, same as 0.6.0. The phrases added in 0.7.0 ("already been reported", "previously disclosed", "acknowledged as a known issue") are not in these bodies either. Judged contest writeups do not admit a prior issue in the words the rule keys on. Matching a bare "prior audit" was tried in an earlier catalog and removed, because "the prior audit missed this" is the opposite claim. The duplicate a report does not mention is `--index`, not P18. The CMP-04 fixture is the admission this rule does catch. This benchmark does not measure that fixture.

P26 is absent from the table below because the default score does not run it. On 0.6.0 it fired on 340/340 of paid plus invalid/won't-fix, and on the QA and Low/Info sets as well. A note that is true of every row is not a signal, so 0.7.0 leaves it off unless `--severity-notes`. That is a demotion, not a measured gain in precision. The bench was not re-scored with the flag on.

## Per rule

A fire on an invalid finding is not evidence the rule named the reason the platform closed it. There is no per-rule ground truth in the labels. Precision here means: of the times the rule fired on paid or invalid/won't-fix, how often the finding was in the invalid/won't-fix set. A rule that never fired has no precision. Rates are `fires / N`.

| Rule | Paid fires (hard) | Invalid+won't-fix fires | Precision |
|---|---:|---:|---:|
| P2 | 9 (9) | 10 | 0.53 |
| P3 | 7 (0) | 7 | 0.50 |
| P15 | 1 (1) | 2 | 0.67 |
| P17 | 22 (0) | 9 | 0.29 |
| P19 | 2 (2) | 0 | 0.00 |
| P4 | 4 (0) | 2 | 0.33 |
| P12 | 3 (0) | 1 | 0.25 |
| P10 | 1 (0) | 0 | 0.00 |
| P14 | 0 | 1 | 1.00 |
| P18 | 0 | 0 | — |
| P26 | not run | not run | — |

P14's precision of 1.00 is one fire. That is not a measurement of P14. P3's 0.50 is the same coin flip as 0.6.0, and it no longer hard-kills.

### Claim checks (weight 0)

P23–P25 are notes. They do not kill. They also do not separate the classes. P26 is the same kind of note and is opt-in; it is not in these counts.

| Rule | Paid | Invalid+won't-fix | QA | Low/Info |
|---|---:|---:|---:|---:|
| P23 attacker | 168/220 | 89/120 | 53/60 | 51/60 |
| P24 preconditions / victim | 214/220 | 114/120 | 60/60 | 57/60 |
| P25 quantified impact | 187/220 | 100/120 | 59/60 | 58/60 |
| P26 severity rule quoted | not run | not run | not run | not run |

Outcomes on the paid set, with no ledger and no target tree (every gate unknown, including A1): under 0.8.0 default, ROUTE-TO-VERIFY 12, HOLD 27, NOTE 5, INCOMPLETE 176, KILL 0. Under `--strict`, KILL 12 instead of the routes. INCOMPLETE is the common outcome because unknown gates outrank a zero-weight note. A paid finding with no hard rule and no ledger is not CLEAR, and it should not be. A1 cannot pass on this corpus: the bench does not supply a checkout or a commit, so the anchor is unmeasured here. The fixture for a missing `src/Vault.sol:214` is in `tests/cli.rs`.

The corpus is judged human writeups. A model hypothesis that says "owner can" and "front-run" next to a real bug is what the P2/P3 changes and `--funnel` are for. This table does not contain that text. Expect the bench to understate how often those rules would have fired on model output, and do not treat 5.5% as that rate.

## In-sample reference, not this measurement

The classified log this catalog was derived from is vendored at
[data/rejection-taxonomy/](../data/rejection-taxonomy/) (snapshot of
[rejection-taxonomy](https://github.com/KhomDev/rejection-taxonomy) v0.6.0,
113 rows, rejections only). Its own generated table, not this benchmark, is
the class mix of that log: 43 duplicate, 26 out of scope, 24 informational,
16 intentional design, 3 below the payout floor, 1 spam. There is no paid row
in it. The strict 14/59 recall was measured on an older catalog against a
private subset of that log and was not re-run. See
[calibration.md](calibration.md).

A separate private triage database of 125 rows (47 of them duplicate) and
`bounty_outcomes.csv` have no report bodies. They were not scored. Scoring a
triage summary would measure the summary.

## Reproduce

```bash
# needs gh auth, and a release binary
python eval/run.py fetch
cargo build --release
python eval/run.py score --vg target/release/vg
python eval/run.py check
```

The committed `eval/results/` are the 0.7.0 per-rule hits. The 0.8.0 default table is the same hits with the default outcome rule applied; `--strict` reproduces the 0.7.0 KILL column.

`fetch` uses the network. `score` does not. `check` recomputes the headline
from `eval/results/per-item.jsonl` and fails if `summary.json` has drifted.
The per-item file stores rule ids and outcomes, not the bodies.
