# What the data says

Six patterns that recur across 113 logged rejections. Each one is something you can check for on
your next submission, and each is backed by rows you can pull with `scripts/query.py`. Every count
on this page is checked against `data/summary.json` by `tests/test_docs_numbers.py`.

---

## 1. Duplication is the largest single outcome, and some of it is self-inflicted

**43 of 113 rejections (38.1%) were duplicates.**

A duplicate does not mean you missed a bug. It usually means you found a *real* one that someone
else found first, or that the team already knew about.

```bash
python scripts/query.py --triage duplicate
```

**What to do differently.** Enumerate the known-issue families and the prior-audit surface *before*
writing the proof. Not the table of contents - the actual findings. A prior audit naming a file is
not the same as covering your function.

**Some duplicates are of your own work.** 12 rows repeat a family the author had already submitted,
across 9 families (the largest has 4 rows). 9 rejections could have been stopped by nothing but the
author's own submission log. An unlogged outcome is invisible to your own duplicate check: log
every submission, including the rejected ones, or you will re-file against yourself.

```bash
python scripts/query.py --repeats -v
python scripts/query.py --preventable-by own_log
```

---

## 2. "Valid but unpaid" is a real category, and it is the most expensive one

**3 rows (2.7%) were confirmed valid and paid nothing**, because the program's reward table had no
tier at that severity.

```bash
python scripts/query.py --triage below_payout_floor -v
```

That is a small share of the total and a large share of the cost. In each case the work was done
correctly: a real bug, a working proof, a team that agreed it was real. The structure of the
program made it worth zero.

**What to do differently.** Read the reward table first. Identify the lowest severity with a
non-zero reward, and ask whether your thesis can reach it. If the program is Critical-only and your
mechanism is a Medium-class value misallocation, stop - not "hunt carefully", stop.

The private record sometimes files these under the program's own wording, such as "out of scope".
The *semantic* outcome is a payout-floor miss, which is why this dataset buckets them separately.

---

## 3. Wrong analysis is the minority; scope, duplication and economics are the rest

Two definitions of "the analysis did not hold up":

- **strict** - the impact was fabricated: **1 row**.
- **broad** - no security impact, intentional design, or fabricated: **41 of 113 (36.3%)**.

An earlier version used the informational bucket alone and called everything else "targeting".
That is 100% minus one bucket, not a definition; the generated README section explains the
difference.

The direction is the useful part: most rejections were about *which* bug, on *which* program, and
*whether anyone already knew* - not about the analysis of the bug itself.

```bash
python scripts/query.py --family reentrancy        # 2 rows
python scripts/query.py --family accounting        # 18 rows
```

The exotic categories are thin. The mundane ones - value counted wrong, input trusted when it
should not be - dominate.

---

## 4. Privileged-role preconditions read broader than they are

Almost every program has a clause ruling out "attacks requiring access to privileged addresses". Two genuinely
different findings get filed under it:

- The attack **requires an admin to act maliciously** -> out of scope, correctly.
- A **reasonable admin action has a non-obvious destructive second-order effect** -> potentially in
  scope. The admin is not misbehaving; the code is.

Check which one you have before abandoning the finding.

```bash
python scripts/query.py --root-cause oos_privileged_role -v
python scripts/query.py --triage out_of_scope -v
```

---

## 5. Panic-and-revert is not an impact

**5 rows** are `panic`-family findings. A transaction that reverts rolls back every state change:
the persisted state is identical to the pre-call state, and the caller has only burned their own
gas.

```bash
python scripts/query.py --family panic -v
```

The one SPAM row in the dataset is a panic-family finding whose headline claimed more than its own
proof showed - the worst outcome in the dataset, worse than informational, because the report's
evidence disproved its title.

**What to do differently.** Ask what state exists *after* the revert that did not exist before. If
the answer is nothing, you have a robustness note, not a vulnerability. Never let a title claim
more than the proof shows.

---

## 6. Async ordering is where the subtle bugs live

`async_ordering` accounts for **7 rows**: state committed on one side of an asynchronous boundary
before the other side has settled, or a failure path that does not undo what the success path
assumed.

```bash
python scripts/query.py --family async_ordering -v
```

These are also the ones most likely to be closed as duplicates or design decisions, because the
protocol has usually thought hard about its own message flow. They are worth hunting and worth
pre-checking hard.

---

## What could have stopped them

**63 of 113 (55.8%) could have been stopped from the public record** - the program's pages, its
published audits and known issues, or its test suite. Most of those checks still need the finding
in hand. The full split, and the per-gate table, are in the README's generated section.

```bash
python scripts/query.py --preventable-by public_record --json | jq -r '.[].mechanism_class' | sort | uniq -c
```

---

## Reading this honestly

- **n = 113, one researcher.** These are proportions in a personal log, not base rates.
- **This is a log of failures.** It says nothing about how many submissions succeeded, so no success
  rate can be derived from it.
- **The gate assignments are self-assessed.** They are the most useful column and the least
  objective.
- **Selection bias is baked in.** These are the programs I chose to hunt, with my own habits -
  including the bad ones - shaping every number.
