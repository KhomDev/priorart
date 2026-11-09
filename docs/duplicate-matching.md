# Duplicate matching (D1)

`--known-issues` is a regex corpus. It matches what you transcribed and nothing
else. That is unchanged.

`--index` is the other input. It is an `index.yaml` from `vg import`, or a
directory of markdown and text (one finding per file, up to 2000 files). D1
runs if either input is present. It is unknown only when both are absent.

```bash
vg match finding.md --index target/index.yaml
vg check finding.md --index audits/ --known-issues target/known-issues.yaml
```

## What is scored

Two sets, taken from the draft and from each prior finding:

1. **Strong identifiers.** Names with an underscore (`reduce_outflow`), a `::`
   (`limiter::reduce_outflow`), mixed case (`BorrowLiquidation`), a path
   (`src/Vault.sol`), or at least 14 characters. Language keywords and the
   short words that show up in every report (`function`, `amount`, `vault`,
   `transfer`, …) are dropped.
2. **Root-cause stems.** A fixed list: reentrancy, overflow, rounding, oracle,
   stale, signature, replay, liquidation, accounting, refund, segment, outflow,
   and others of that shape. They never match on their own.

A prior finding is reported only when at least **two strong identifiers** are
shared. The score is

```text
0.75 * (shared identifiers / size of the smaller identifier set)
+ 0.25 * (shared stems / size of the smaller stem set)
```

capped at 1. At **0.50 or above** the gate fails and `vg check` also emits a
soft finding `DUP:<id>` with the score and a quote. Below 0.50 the row is
shown and the gate does not fail.

`vg match` prints the top 5, each with the shared identifiers, the shared
stems, and up to two quotes from the prior finding that contain a shared
identifier.

## What is not scored

Ordinary words. The author measured Jaccard between a rejection description and
the report it describes at about 0.024, with containment 0.61. Overlap of the
whole vocabulary does not separate a duplicate from a neighbour. This matcher
does not compute it.

A concurrent duplicate is still invisible if it is not in the index. The index
is only what you imported or downloaded.

## The threshold is not a measurement

0.50 and "two identifiers" are a stated heuristic. They were not fit on the
author's duplicate log, and the public benchmark in
[benchmark.md](benchmark.md) scores the catalog rules, not this ranker. A hit
is a prompt to read the quoted lines. It is not a determination that the bugs
are the same.

The regex side of D1 is still unvalidated in the sense
[calibration.md](calibration.md) describes: single-term patterns cross-fired,
and two-term patterns missed. Identifier overlap is a different question. It
has not been given that test, and this page does not pretend it has.
