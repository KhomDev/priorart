# Public-corpus benchmark

Measured 2026-11-09 with `vg` 0.6.0. Re-run with `python eval/run.py fetch` and
`python eval/run.py score`. The bodies are downloaded into `eval/cache/`, which
is gitignored. This repository commits the manifest and the results, not the
reports.

This is one corpus. It is not a property of the rules, and it is not the
false-positive rate on the author's own paid findings. That log has no paid
findings. See [control-set.md](control-set.md).

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

A **hard KILL** is the outcome that would have stopped a submission.

| Set | N | Hard KILL | Rate |
|---|---:|---:|---:|
| Paid Medium/High | 220 | 19 | 8.6% |
| Invalid + won't-fix | 120 | 19 | 15.8% |
| Code4rena QA | 60 | 14 | 23.3% |
| Sherlock Low/Info | 60 | 10 | 16.7% |

Of the 38 hard kills on paid or invalid/won't-fix, 19 are paid findings. That
is a precision of **19/38 (50%)** for "this hard kill marks a finding the
platform did not pay." The base rate of invalid/won't-fix in that pair of sets
is 120/340 (35%). A hard kill is more common on the unpaid side than that base
rate, and half of the hard kills are still findings a platform paid. Do not
read 8.6% as "the tool is safe 91% of the time." Nineteen paid findings were
hard-killed.

Which rules did the killing, on the paid set: **P2 on 9, P3 on 7, P19 on 2,
P15 on 1.** Paid contest reports describe admins and sandwiches inside findings
that were paid anyway. P2 and P3 are the catalog entries that do that, and on
this set they do not separate paid from invalid (P2 precision against
invalid/won't-fix is 10/19, P3 is 7/14).

P18 did not fire on any of the 220 paid findings or the 120 invalid/won't-fix
findings. Those writeups are the judged text, and the judged text of a paid
finding does not usually admit "same root cause as accepted issue M-1." The
CMP-04 regression is that admission. This benchmark does not measure it. The
fixture does.

## Per rule

A fire on an invalid finding is not evidence the rule named the reason the
platform closed it. There is no per-rule ground truth in the labels. Precision
here means: of the times the rule fired on paid or invalid/won't-fix, how often
the finding was in the invalid/won't-fix set. A rule that never fired has no
precision. Rates are `fires / N`.

| Rule | Paid fires (hard) | Invalid+won't-fix fires | Precision |
|---|---:|---:|---:|
| P2 | 9 (9) | 10 | 0.53 |
| P3 | 7 (7) | 7 | 0.50 |
| P15 | 1 (1) | 2 | 0.67 |
| P17 | 22 (0) | 9 | 0.29 |
| P19 | 2 (2) | 0 | 0.00 |
| P4 | 4 (0) | 2 | 0.33 |
| P12 | 3 (0) | 1 | 0.25 |
| P10 | 1 (0) | 0 | 0.00 |
| P14 | 0 | 1 | 1.00 |
| P18 | 0 | 0 | — |

P14's precision of 1.00 is one fire. That is not a measurement of P14.

### Claim checks (weight 0)

P23–P26 are notes. They do not kill. They also do not separate the classes.
P26 fired on **220/220** paid findings and **120/120** invalid/won't-fix
findings. Contest writeups state a severity label. They do not quote a program
severity sentence in the shape the check looks for. A 100% fire rate means the
check does not recognise that label. It is reported because hiding it would
make the note look like it had found something.

| Rule | Paid | Invalid+won't-fix | QA | Low/Info |
|---|---:|---:|---:|---:|
| P23 attacker | 168/220 | 89/120 | 53/60 | 51/60 |
| P24 preconditions / victim | 214/220 | 114/120 | 60/60 | 57/60 |
| P25 quantified impact | 187/220 | 100/120 | 59/60 | 58/60 |
| P26 severity rule quoted | 220/220 | 120/120 | 60/60 | 60/60 |

Outcomes on the paid set, with no ledger and no index supplied (every gate
unknown): KILL 19, HOLD 20, NOTE 5, INCOMPLETE 176. INCOMPLETE is the common
outcome because unknown gates outrank a zero-weight note. A paid finding with
no hard rule and no ledger is not CLEAR, and it should not be.

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

`fetch` uses the network. `score` does not. `check` recomputes the headline
from `eval/results/per-item.jsonl` and fails if `summary.json` has drifted.
The per-item file stores rule ids and outcomes, not the bodies.
