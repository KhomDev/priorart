# Methodology: how priorart's duplicate detection is measured

> **Status for the release on `main`: DEV-ONLY.** The numbers this release states are train
> (fitting) and validation (model-selection) numbers. No held-out recall, FAR or warning precision is
> claimed. The held-out rounds come after main, under `eval/P2R2_PREREGISTRATION_POST_MAIN.md`.

## The question
Given a new finding and a corpus of earlier findings for the same codebase, does priorart put the
finding's true earlier duplicate in its top five, and does its alarm stay quiet on findings that
have no duplicate?

## Ground truth: the judges, not us
Dup-Bench v1 (`eval/dupbench/`) has 111 Code4rena contests with public judge grouping. The judge's
`duplicate-N` label, pointing at the `selected for report` issue, is the label. No labeller of ours
decides an outcome. `fetch.py` pins the findings commit, the `report.md` hash, the `data/` listing
and every label page. Only issue numbers and labels reach `labels.json`, never a finding body.

## Splits (Amendment 1)
| Split | Window | Contests | Paid-unique | Judged duplicates | Use |
|---|---|---:|---:|---:|---|
| train | before 2024-03-01 | 58 | 564 | 1,237 | fitting the ranker (`train_ranker.py`, seed 20261126) |
| validation | 2024-03-01 .. 2024-09-01 | 36 | 380 | 1,441 | grid selection; calibrating the alarm |
| FINAL | after 2024-09-01 | 17 | 158 | 776 | sealed; every runner refuses it |

There is no V2 slice. The **pairs** are the V1 slice: a prior public audit plus a later judged contest
by the same sponsor. After the 2026-11-27 swap, the four contests that are also in Dup-Bench are gone. The
V1 pairs are paraspace, ethos, venus and arcade (sealed in `pairs-swap/sealed_optA.json`) plus
revert-lend and loopfi.

## Cells
- **recall@5:** the share of judged duplicates whose judged primary is in the top five. An abstention
  counts as a miss and is also reported on its own.
- **FAR at the shipped alarm:** the share of paid-unique primaries on which the alarm fires.
- **Warning precision:** of the queries where the alarm fires, the share whose top candidate is the
  judged primary. The alarm ships ON only if the FINAL Wilson LB is at least 0.80.
- **Abstention by cause.**
- Wilson 95% (z = 1.959964) on every k/n, n < 5 reported as a count, a contest-clustered bootstrap
  (10,000 draws) on pooled cells, and prose anchors off.

## What is never done
Refitting or recalibrating on V1 or FINAL; running `--check` with a placeholder round, version or sha
on a sealed split; pooling pairs with Dup-Bench, DEV with held-out, or the validation split with a fresh
number.

## Where the numbers are
[`benchmark.md`](benchmark.md) ("DEV-ONLY release table"), [`limitations.md`](limitations.md),
[`reproducibility.md`](reproducibility.md), and the selection record in
`eval/dupbench/pairs-swap/SELECTION.md`.