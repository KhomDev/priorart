# Calibration — viability-gate against 97 logged rejections

> **Dated note (2026-10-21, updated 2026-11-06).** Every number in this document was measured on
> the **original 97** logged rejections — the in-sample corpus the 22 anti-patterns were derived
> from — **with the catalog as it stood when the figures were first published in 0.2.0
> (2026-10-09)**. Two things have changed since, and the study has not been re-run for either:
>
> - the published rejection dataset has grown to **113 rows** (rejection-taxonomy v0.6.0);
> - **the catalog has changed.** 0.3.0 reworked several rules, and 0.5.0 changed P2, P3, P12,
>   P13, P15, P17, P19 and P21 and the sentence and heading parser, after an audit reproduced
>   valid findings that the older catalog hard-killed.
>
> So the `59/97`, `14/59` (23.7%), `45/59` (76.3%), 52-point-gap and `48/59` (81%) numbers describe
> an older catalog on the original 97-row log, left exactly as measured. **The recall of the
> current catalog is unmeasured.** The original finding texts are private, so this study cannot
> be re-run from the repository; `tools/calibrate/calibrate.py` and its synthetic fixture show
> the method, not the result. Only the reachable-automation counts in the README track the
> 113-row dataset.

**Read this first: the measurement below is in-sample.** The 22 anti-patterns were
derived from the patterns in this same log, so the tool was built knowing these
outcomes. **23.7% is closer to a ceiling than to an unbiased estimate.** Every
number in this document is affected by that, and no number in it has been
measured out of sample.

The second thing to read first: **the tool's false-positive rate has never been
measured at all.** The log contains no paid findings. See
[What the numbers do not show](#what-the-numbers-do-not-show) and
[`docs/control-set.md`](control-set.md).

**Question:** of the rejections the tool would have flagged, how many were rejected for the
reason it flagged?

---

## Method

1. Parse the 97 rows of the rejection log for report title and recorded gate.
2. Match each row to the original finding text by content-token overlap. **59 of 97 rows
   matched at ≥0.50 confidence**; 13 matched weakly and 25 not at all, and all 38 are excluded
   from the headline, because a wrong row-to-file pairing produces a meaningless score.
3. Run `vg check --json` on each matched finding.
4. Map each fired anti-pattern to the gate it satisfies, and compare with the gate the log says
   actually decided the outcome.

A rule firing does not mean the tool "caught" the rejection. It means the tool would have told
you to hold, for the same reason the program closed it.

**The matched subset is not a random sample of the 97 rows.** A row had to be
matchable to its finding text to be scored at all, and matchability correlates
with how much prose the row's description carries — which is not independent of
which gate closed it. The 59-row subset's gate mix may therefore differ from the
97-row mix, and this document does not know by how much. The full log's gate mix
is published in the vendored snapshot
[`data/rejection-taxonomy/data/summary.json`](../data/rejection-taxonomy/data/summary.json)
(source: [rejection-taxonomy](https://github.com/KhomDev/rejection-taxonomy) v0.6.0);
the matched subset's mix is not published, so the two cannot be compared here.

## Result

| Scoring | Result |
|---|---:|
| **Strict** — named the gate that actually decided it | **14/59 — 23.7%** |
| **Lenient** — named *any* gate-mapped reason to stop | **45/59 — 76.3%** |
| Miss — no gate-mapped rule fired | 14/59 — 23.7% |

The 52-point gap between the first two rows is the finding. The tool is good at telling you to
stop and bad at telling you why.

A further 3 findings fired only report-quality patterns (defective PoC, self-contradiction,
triager-simulation tables), which correspond to no gate, so they score as misses.

### Why the strict metric is harsh

Scoring a mismatch as a failure was a deliberate choice: if the tool says "hold, no
attacker-win" and the program closed it as out of scope, that counts against it.

On the evidence, that choice is too strict. **Most mismatches name a different true reason to
stop, not a wrong one.** The strict figure is a lower bound and the lenient figure an upper
bound; the truth depends on whether you want a tool that stops you for the right reason or just
stops you.

## The structural ceiling

Not every rejection is expressible as a pattern in a finding's prose. A duplicate is a fact
about an **external corpus** — the program's known-issues list and its published audits — not
about the finding's text.

| Class | Pairs | Hit rate |
|---|---:|---:|
| Gates the catalog can reach | 26 | 53.8% |
| Duplicate / prior-audit (needs an external corpus) | 28 | 0% |
| Mixed or unmapped | 5 | 0% |

**Read those with their sample sizes.** 53.8% is 14 of 26 — reclassify three rows and it moves
eleven points. The 5-pair class is smaller still. These are directions, not stable estimates.

### The duplicate class, with both denominators

The 28 corpus-only pairs are the largest single class, and the number that
describes them depends on which denominator you use. Both are true; they answer
different questions:

| Statement | Fraction | Percentage | Question it answers |
|---|---:|---:|---|
| Corpus-only pairs as a share of **all matched pairs** | 28/59 | **47.5%** | How much of the scored set is this class? |
| Corpus-only pairs as a share of **all non-hits** (59 − 14 strict hits = 45) | 28/45 | **62.2%** | Of the findings the strict metric missed, how many are this class? |

The first figure describes the set; the second describes the failure. Quoting
either one without its denominator overstates or understates the case, which is
why both are here.

**"0%" does not mean silence.** Of the 28 corpus-only pairs, the tool still fired a gate-mapped
rule on **20**. It found *a* reason to stop almost every time; it never found the duplicate,
because the duplicate was never in the text it was reading.

That distinction matters: a tool that goes silent needs better rules, a tool that finds the
wrong reason needs a better input.

## The proposed fix, tested — and it failed

The obvious response to the ceiling is to add the missing input, which is what
`--known-issues` does: a user-supplied corpus of the program's known issues and prior-audit
findings, matched against the draft finding.

That claim was asserted from the rejection log and never demonstrated. So it was tested:
patterns were generated from each of the 28 corpus-only mechanisms, then the matcher was run
over all 59 calibrated findings.

| Terms required | Recall | Total fires | Cross-fires |
|---|---:|---:|---:|
| 1 | 13/28 (46.4%) | 56 | **43 (76.8%)** |
| 2, proximity | 3–4/28 (10.7–14.3%) | 5–8 | 40–50% |

One-term patterns reach 46% recall, but **77% of everything they fire on is the wrong
finding.** Two-term proximity patterns cut false fires and collapse recall to 11–14%. There is
no useful operating point.

Mean vocabulary overlap between a rejection description and the bug report it describes:
**Jaccard 0.024**, and **containment 0.61**. Roughly 40% of a rejection's distinctive
vocabulary never appears in the finding. The two describe the same behaviour in different
registers, and a keyword matcher needs shared vocabulary.

### Why this is "not validated" rather than "disproven"

Three things flatter the result, and it fails anyway: the patterns were written with hindsight,
term selection used document frequency across the very findings being scored, and rarest-first
selection is a crude proxy for what makes a term discriminating.

But three things also make the test unfair to the approach:

1. The patterns came from *rejection summaries*, not from the *program's own known-issues
   wording*, which is what the real workflow uses.
2. The patterns were auto-generated, not written by someone who understood the issue.
3. Containment of 0.61 shows the vocabulary is *partly* there — the signal exists, and the
   construction was not using it.

**The test that would settle it is prospective:** capture the program's known-issues text before
hunting, then check at rejection time whether a hand-written corpus derived from it would have
fired. That has not been run. Treat the mechanism as a hypothesis with a stated test, not a
validated fix. [`docs/prospective-protocol.md`](prospective-protocol.md) is that test, written
down so it can be run on someone else's log.

## Where the remaining shortfall is

- **Most mismatches are a measurement artefact.** The log records the gate the triager cited and
  what was later judged to have been catchable. Those are not always the same gate.
- **Real misses: 14 of 59.** The genuine rule-coverage gap and the honest candidate list.
- **The clearest single gap is intentional design.** Five pairs, no hits. Reachable before
  writing anything — but note that across the entire 97-row log, only **one** row is recorded as
  decided by the test-intent gate, so widening the intent patterns is only half the fix; the
  other half is routing project documentation through the known-issues corpus.

## What the numbers do not show

- **This is an in-sample measurement.** The 22 anti-patterns were derived from the patterns in
  this same log, so the tool was built knowing these outcomes. 23.7% is closer to a ceiling than
  an unbiased estimate.
- **The false-positive rate is unmeasured, not low.** The log's `paid` column is `no` for all
  97 rows, so the tool has never been run against a finding that was accepted or paid. The
  number a user most needs — *how often does this hard-fail a real finding?* — does not exist.
  It fired on 48 of the 59 rejected findings (81.4%), which is a recall-side figure and says
  nothing about precision. That 48 is the 45 lenient hits plus the 3 findings that fired only
  report-quality patterns; it is derived from the two published rows above, not separately
  measured. [`docs/control-set.md`](control-set.md) is how to build the set that
  would measure it.
- **The matcher is a heuristic.** Content-token overlap, not ground truth.
- **The gate mapping is a judgement**, and it is not published with the result. The public
  restatement in [`docs/gate-mapping.yaml`](gate-mapping.yaml) was authored from the rules'
  semantics and the public gate vocabulary; it has not been validated against the private data.
- **The matched subset's gate mix may differ from the 97-row mix.** See Method.
- **The log's gate column is post-hoc and self-assessed.**
- **A hit is not a saved submission.** A HOLD verdict is advice; the researcher still has to act.
- **One researcher, four months, 97 rejections.** Not a survey, and its proportions are not base
  rates.
- **This is not reproducible.** The harness read findings under disclosure embargo. The method
  is published; the dataset is not.

## Reproducing the method, not the result

The dataset cannot be published. The method can, and now is:

```bash
cargo build --release

python tools/calibrate/calibrate.py \
  --log my-log.csv \
  --findings-dir my-findings/ \
  --vg target/release/vg \
  --dry-run          # first: inspect the row <-> file pairing
```

`tools/calibrate/calibrate.py` takes a labelled log (`--log`), the original finding
texts (`--findings-dir`), and the public gate mapping
([`docs/gate-mapping.yaml`](gate-mapping.yaml)), and reports the strict, lenient and miss
counts over the rows it could match. It is the same three-way scoring described above.

**It will not reproduce the numbers in this document**, and it does not claim to:

- The private log and the original findings are not published, so there is nothing to run it on.
- The mapping used for the published result lived in the private harness. The public mapping is
  a re-statement, not a re-derivation — see its provenance header.
- Matching rows to files is the step that decided which 59 rows were scored. That step is a
  heuristic on both sides of the fence.

What it does give you is the ability to run the same test on your own log, which is the only
place the test has any power. `examples/calibration/` holds a synthetic eleven-row fixture that
runs end to end in CI; its pinned result is a smoke test of the harness, not a result about the
tool.
