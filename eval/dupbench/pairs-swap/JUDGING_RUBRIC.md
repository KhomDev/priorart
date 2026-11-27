# Duplicate-judging rubric for human labellers (approved by Yura, 2026-11-27 18:19 Budapest)

**Status: approved.** Used for the priorart V1 human check (`judging_sheet.py`). Nothing in it changes the V1 judge-labelled
numbers in `eval/rounds/round-1-pairs.json`; the human labels are a second opinion on them.

## The task
A row has two findings from the same audit contest: finding 1 (a query) and finding 2 (the candidate priorart's matcher
returned as the best match). Question: **do the two describe the same underlying vulnerability (same root cause in the same
code path), so that a contest judge would group them as duplicates?**

- **yes**: same root cause, same affected code path or function. Different wording, severity, impact story or proof of concept do not matter.
- **no**: different root cause, different code path, or the same contract and a different bug. "Same area, different bug" is no.
- **unclear**: the text does not decide it (missing detail, one finding too vague). One line saying why is required. Use it sparingly.

## Rules for the labeller
1. Judge only from the two texts on the sheet. Do not search for the contest's published report, the findings repository, or any
   grouping; do not look up the project's code to settle a row (reading it to understand a term is fine).
2. You will not see any score, any alarm decision, which findings are in the same group, or which contest a row comes from.
3. Label alone. Do not discuss rows with anyone, including the other labeller of your batch.
4. A few rows are shared by everyone who gets a sheet from this batch plan (to measure agreement across people). You will not be told which.
5. Answers are exactly `yes`, `no` or `unclear`.

## What the labels are used for
- Agreement: Cohen's kappa between the two labellers of a batch, pooled over batches; Krippendorff alpha on the shared rows. kappa below
  0.70 makes the cell inconclusive.
- Comparison with the judge grouping, kept apart from it: among alarm firings, the share the humans call `yes`, next to the judge-labelled
  43/48; among quiet controls, the share called `yes`. No pooling of the human labels with the judge labels.
- Disagreements between the two labellers, and between the humans and the judges, get a written adjudication.