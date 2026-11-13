# priorart public benchmark

The benchmark behind "has this been found?". It measures one thing: when a
judged contest rejects a finding because the same thing was already known,
does priorart rank the right prior item in the top five? It also measures how
often priorart raises a prior item for a finding the judge paid and did not
duplicate (the false-alarm rate).

Everything here is re-derivable from committed files. The report bodies, issue
bodies and PoC sources are **not** committed: they are fetched into
`eval/cache/benchmark/`, which the repository gitignores. The repository holds
pointers (repo@commit + path), labels and the generated results.

Status: FROZEN, dated 2026-11-13, measured on the shipped priorart 0.1.0
binary, sha256
`dabfcc0ca197ee35b40772cb7db666f17105c5c36faa7f9da7ef783a9a78e12a`
(priorart-core gate: fmt --check 0, clippy -D warnings clean, cargo test
--locked 91 passed: 56 lib + 33 cli + 2 docs; README generated block current).
Matcher: symbol identifiers only, file paths shown but never counted toward the
>= 2 threshold, ties ordered by specificity/rarity/identifier order,
negation-aware preconditions, whole-sentence quotes, char-boundary fix. The
binary differs from `2ec4c3c9...` only in output wording (the retrieval-only
reading-list line) and README/test text; its matcher is identical and it
reproduced the same numbers. Earlier runs are recorded for provenance and are
NOT to be quoted: `22648e33...`
(before the matcher fix, FAR 30.0%), `a973818a...` (matcher fix, but it
panicked on one input), `d8ab81cd...` and `da176bc8...` (both FAR 25.0%). The
harness itself reported a bug between the first and second run; see the errata
paragraph. The harness also found a crash in an intermediate matcher (a
char-boundary panic on a curly apostrophe in one judged duplicate);
priorart-core fixed it and this run reports 0 tool errors. `freeze.json` holds
the manifest, labels and harness hashes; `results.json` records the binary hash
actually used for its per-query rows, the seed, k, the held-out flag and any
per-query tool error.

**Sha history (2026-11-13).** The deciding run was made on `2ec4c3c9...`.
While the tree was still being rebuilt, an intermediate build `a2dfd8e0...`
also produced the same numbers. priorart-core then declared `176f1986...`
(matcher identical to `2ec4c3c9...`, wording-only difference), and after one
last stale-prose sweep superseded it with `dabfcc0c...` (a NOTES sentence in
src/check.rs; no scoring, ranking, threshold or negation change). Both the
`176f1986...` rerun and the frozen `dabfcc0c...` rerun reproduced the same
numbers (26/60 = 43.3%, 39/64 = 60.9%, n=1 known-prior, 0 errors), so the frozen
sha in `results.json` and in this file is `dabfcc0c...`.

**Correction, 2026-11-13.** An earlier interim report by this harness said
duplicate recall@5 was 0/64. That number was wrong: the harness compared the
judged id `acc:001-M` with the dossier item `acc-001-M.md` without normalising
the colon to a dash, so every correct retrieval was scored as a miss. The
matcher had in fact returned the representative for 36 of the 64 queries on
the then-current binary; later matchers measured 33/64 and then 39/64 on the
final build. The bug is fixed in `run.py`; the corrected number is below. The diagnostic
(duplicate-diagnostic.json in the gitignored cache) is unaffected because it
normalised the id when it was written.

## Corpus

One judged contest pair on the same codebase, which is what makes a prior item
retrievable at all: the same protocol was audited by Code4rena (2023-07) and
then by Sherlock (2024-02). The prior audit is the document the later contest's
sponsor said makes findings ineligible.

| Role | Pointer | What it provides |
|---|---|---|
| prior audit | `code-423n4/2023-07-tapioca-findings` @ `4193111261e3cf8de27c15857babecaa2519d67c`, `report.md` | 60 High/Medium findings, each split into one file per item |
| known issues | `sherlock-audit/2024-02-tapioca` @ `dc2464f420927409a67763de6ec60fe5c028ab0e`, `README.md` | the sponsor's Q&A answer to "Please list any known issues/acceptable risks" (1 item: zero address check) |
| judging | `sherlock-audit/2024-02-tapioca-judging` @ `9e52685cd447417a1d9d5c96fb1492d8773eab8a` | 60 accepted finding directories and 26 excluded findings; 64 submissions grouped by the judge as duplicates |

Counts (from `manifest.yaml`, `fetch.py --manifest`):

- 61 prior items (60 audit + 1 known issue)
- 86 judged findings: 60 paid Medium/High, 26 excluded
- 64 judge-grouped duplicate submissions
- 1 finding with a defensible known-prior grounding

## Measurements and definitions

**recall@5 on known-prior findings.** Positive = a finding the judge excluded
*and* whose text repeats one specific prior item. A hit is that prior item
appearing in the top 5 that `priorart check --top-k 5 --json` returns. This is
the plan's headline metric and it **cannot be measured at adequate n on this
corpus**: the judging repositories do not publish the reason an issue was
excluded, and after reading all 26 excluded findings only one grounds in a
prior item (the zero-address known issue). **n = 1, 0 hits. This is not a
rate.** Wilson interval for one observation is [0, 0.79], which is printed
because the arithmetic is honest, not because 0/1 means anything.

**false-alarm rate on judged paid and unique findings.** Denominator = the 60
accepted Medium/High findings, none of which duplicates another accepted
finding and none of which the judge called known. An alarm = `check` returns
at least one prior item for the finding. By the matcher's own rule (>= 2 strong
identifiers) any returned item is a claim of shared prior art, so the alarm
definition is the tool's, not ours. Result on the final build:
**26/60 = 43.3% [31.6%, 55.9]**, Wilson 95%, 0 tool errors. Earlier runs
measured 18/60 = 30.0% [19.9%, 42.5%] (pre-fix) and 15/60 = 25.0%
[15.8%, 37.2%] (intermediate matcher). The final build raised the alarm rate
substantially, and the whole final interval is above 30%. The denominator is
60 judged findings from one contest pair, so this is not a population estimate
for "unique findings" in general.

**The plan's rule, applied (quoted from CONSOLIDATION-PLAN §4, kill
criteria):** "If priorart's false-alarm rate on unique findings is above 30%, it
ships with retrieval only (no 'share identifiers' warning) until the matcher is
fixed."

## Runs

Three binaries were measured with the same harness, seed, manifest and dossiers.
Only the third is the number of record; the earlier two are provenance.

| Run | Matcher revision | FAR on 60 paid+unique | Duplicate recall@5 | Known-prior recall@5 | Tool errors |
|---|---|---|---|---|---|
| `22648e33...` (2026-11-12) | pre-fix: paths counted as identifiers; source-order ties | 18/60 = 30.0% [19.9%, 42.5%] | 36/64 = 56.3% [44.1%, 67.7%] | n=1, 0 hits, not a rate | 0 |
| `da176bc8...` (2026-11-13) | symbol identifiers only; generic symbols (>20% of items) excluded; negation-aware | 15/60 = 25.0% [15.8%, 37.2%] | 33/64 = 51.6% [39.6%, 63.4%] | n=1, 0 hits, not a rate | 0 |
| `dabfcc0c...` FINAL (2026-11-13) | + paths never count toward >=2 (shown only); ties by specificity/rarity/identifier order; negation window tightened; identifiers stored in original case | **26/60 = 43.3% [31.6%, 55.9%]** | **39/64 = 60.9% [48.7%, 71.9%]** | n=1, 0 hits, not a rate | 0 |

The deciding run on `2ec4c3c9...`, the intermediate build `a2dfd8e0...` and
the declared-then-superseded `176f1986...` produced the same numbers as the
frozen row; only `dabfcc0c...` is quoted. A
build `a973818a...` is not a row: it panicked on one input (a char-boundary
slice on a curly apostrophe) and that harness run is not comparable; it is
recorded in the provenance paragraph only.

**Why FAR moved from 25.0% to 43.3% (cause supplied by priorart-core).**
Three matcher revisions increased qualifying overlap, and contract/token names
are the most repeated identifiers in a single-repo dossier. (1) Identifiers now
keep their original case, so short PascalCase/camelCase contract and token
names (BalanceSheet, VaultEngine, PSM, USDC-A) that were previously lowercased
and then rejected by the length >= 14 rule now count as symbols. (2) The
specificity floor was relaxed from "named by at most one other item" to "at most
two other items", so a pair of findings that share a rare-looking contract name
qualifies. (3) The negation window was narrowed to pre-match cues plus explicit
post-"not" forms; this mostly affects exclusions, but a nearby "no ..." can no
longer suppress a genuine symbol. Judged-paid unique findings live in the same
repository as everything else, so more of them now share two contract symbols
and get a prior item (26/60 vs 15/60), while duplicate recall also rose (39/64
vs 33/64). The duplicate queries that now return nothing are the ones whose only
overlap was file paths, which no longer count toward the threshold (paths are
evidence/tie-break only).

**Application of the plan's rule (CONSOLIDATION-PLAN §4, kill criteria):** "If
priorart's false-alarm rate on unique findings is above 30%, it ships with
retrieval only (no 'share identifiers' warning) until the matcher is fixed."
On the final build the measured FAR is 43.3%, with a 95% interval of
31.6-55.9% that does not cross 30%. The trigger is therefore met on this
corpus: priorart 0.1.0 ships in retrieval-only mode - no share-identifier
warning and no implication of duplication - until the matcher is fixed. The next
release must fix the matcher and re-measure before the warning returns; a
release decision taken on the intermediate 25.0% run is superseded by this
build, and the benchmark does not soften that. The
denominator is one contest pair (60 judged findings), so the next release must
re-measure on a fresh held-out split before the threshold is treated as
settled. The FAR must always be quoted with n and the interval, never 43.3%
alone; this document supplies the numbers, and the release consequence follows
from the plan's own rule.

**supplementary: recall@5 on judged duplicate submissions.** Not the plan's
metric, and labelled as such everywhere. Denominator = the 64 submissions the
judge grouped with a representative (54 in accepted groups, 10 in excluded
groups). A hit = the group's representative appears in the top 5 from a dossier
of the 86 judged representatives. This exercises the same identifier matcher
against a relation the judge actually labelled, so it has power. Result on the
final build: **39/64 = 60.9% [48.7%, 71.9%]**, 0 tool errors. Earlier runs
measured 36/64 = 56.3% (pre-fix, [44.1%, 67.7%]) and 33/64 = 51.6%
(intermediate matcher, [39.6%, 63.4%]). The final build retrieves the judged
representative for about three in five judged duplicates; the headline caveat
is that it misses 25 of 64.

Diagnostic on the final build: the top-1 candidate was the expected
representative in 27 of 64 queries; 35 of 64 top-1 candidates shared at least
one file-level identifier (`*.sol`, a GitHub blob URL, or a path) with the
query; the median shared-identifier count was 7; and 6 queries returned no
prior item at all, because paths alone no longer qualify and those submissions
carry no qualifying symbol identifiers. Top-1 shared identifiers were mostly
symbols (375 symbol mentions vs 91 file-level and 1 generic across the 64
top-1s), so the remaining failures are ranking on shared symbols plus the
path-only queries with no symbol overlap. That is a defect report for the
matcher, not a property of the corpus.

All intervals are Wilson 95% with z = 1.959964, computed in `run.py`; `unclear`
does not exist in this label scheme. The sampling seed is committed in
`labels.yaml` / `sources.yaml` (`priorart-bench-2026-11-12`) and is used for
the dev/held-out split, the duplicate sample and the agreement sample.

**Freeze and held-out.** `run.py --split dev` writes `freeze.json` (manifest
sha256, labels sha256, harness sha256, seed, k). `--split heldout` and
`--split all` refuse to run without it and refuse if the manifest changed since.
The run that produced this file used `--split all`, so the held-out half was
opened after the freeze; `results.json.frozen.heldout_opened` says so. There was
no rule tuning on it: the tool's rule logic is priorart's, and this harness does
not alter it.

## Labels

The label file is `labels.yaml`; every label carries its pointer. Values:

- `known_prior` — excluded by the judge **and** the text repeats one named
  prior item (`prior_ref`). Provenance: the prior item's locator plus the
  overlapping claim. 1 row.
- `invalid_unmapped` — excluded by the judge but no prior item reproduces the
  claim. 25 rows. This is not a claim that the finding is wrong; it is the
  absence of a prior-item grounding. The nearest prior item and why it was
  rejected are in each row's provenance.
- `unique_valid` — platform label, applied to all 60 accepted directories. No
  reader involved.

Labels were written by one AI reader (bench-eval, 2026-11-12) reading the
cached bodies. They are one reader's judgement, not the judge's reason, and a
second reader will disagree on some.

**Agreement check.** A blinded second pass took a seeded 20-item sample of the
26 excluded findings and labelled each from the title and the prior-item titles
only, never the bodies. It agreed with the full-text label on **15/20 = 75%
[53.1%, 88.8%]**. The five disagreements are all the same failure: a title
mentions liquidation, solvency or TOFT balance theft, which shares a topic with
an audit finding, and the body then claims a different mechanism. This is label
noise, published as such (sample and blind labels are in `labels.yaml`).

## Terms check (2026-11-12)

What is stored: repository names, commit SHAs, file paths, issue-directory
names, and this repository's own labels and annotations. No finding text, no
report text, no full issue bodies are committed. Fetched text lives only under
`eval/cache/benchmark/`, which is gitignored.

| Repository | Licence / terms relied on | Why including it is acceptable |
|---|---|---|
| `code-423n4/2023-07-tapioca-findings` | no LICENSE file in the repository; GitHub public repository, Code4rena Terms of Service (https://code4rena.com/terms) | a link index and locators only; no report text redistributed |
| `sherlock-audit/2024-02-tapioca` | no LICENSE file; Sherlock public contest repository, Sherlock Terms of Service (https://sherlock.xyz/terms) | pointers and derived labels only |
| `sherlock-audit/2024-02-tapioca-judging` | no LICENSE file; Sherlock Terms of Service | pointers, judge-derived structure (accepted/excluded, duplicate grouping) and labels only |

If either platform changes its terms, the corpus is a pointer list and can be
retired without touching the tool. Re-check the terms before publishing a
benchmark page that embeds any text from these repositories.

## Dead ends (do not repeat these)

1. **Code4rena `invalid` labels do not mean "known issue".** On
   `2024-03-taiko-findings` and `2023-07-tapioca-findings`, sampled `invalid`
   issues were almost all warden withdrawals ("Withdrawn by ..." from a bot),
   plus QA and Gas reports. There is no label that distinguishes known-issue,
   acknowledged and out-of-scope rejections, and the judge's reason is not
   stored in the issue.
2. **Sherlock judging repositories do not store exclusion reasons.** The
   `invalid/` directory holds the excluded submissions and their duplicate
   groups but no reason. `comments.csv` is a 20-byte header-only file. The final
   `README.md` documents accepted findings only.
3. **Code4rena `report.md` is not a judging record.** It contains accepted High/
   Medium/Low findings and their mitigation discussions; it has no invalid or
   known-issue section, so it cannot supply labels.
4. **Automatic identifier-overlap labelling fails.** Matching excluded findings
   to audit items by shared identifiers picks up the contest-wide vocabulary
   (`yieldBox`, `toShare`, `bigBallance`-level names); dozens of findings
   "match" every audit item. This is the same weakness the tool's own matcher
   shows. Topic titles are not enough either (see the 75% agreement noise).
5. **C4 automated findings (4naly3er) are a known-issues document, but no
   labelled rejections.** They are published (e.g. `4naly3er-report.md` in the
   contest repo) so a future corpus could use them as a rich prior-item set;
   what is missing is a judge-labelled set of findings rejected against them.
6. **A larger n needs another mechanism, not another scrape.** The tractable
   options are (a) a contest where the sponsor publishes a substantive
   known-issues list and the judge publishes reasons (none found in this pass),
   or (b) a platform export of judging decisions (not public).

## What this does NOT claim

- It is not a measurement of whether priorart finds bugs. The corpus is judged
  human findings.
- It is not a false-positive rate on model-written hypotheses.
- The duplicate-retrieval number is retrieval of judged group representatives,
  not known-issue detection, and it is not comparable to recall@5 on prior
  audits.
- n = 1 known-prior observation is not a rate, in either direction.
- No held-out tuning happened; but the tool's matcher was not designed against
  this corpus either, so this is a retrospective measurement.

## The older, secondary set

`eval/dataset.manifest.yaml` (with `eval/run.py`, `eval/sources.yaml`,
`eval/results/`) is the previous viability-gate harness: 220 paid Medium/High
contest findings and 120 invalid/won't-fix findings from 16 Sherlock and 8
Code4rena judging repositories, selected by label rules (the script) from
repositories the author chose. It measures the old P-rule catalog's fire rates
and hard-kill precision on judged writeups - not prior-art retrieval, and not
this tool. Its selection is one researcher's choice of contests and label
rules, and its negative class is platform "invalid" which, per dead end 1,
conflates reasons. It is kept for provenance of the published 0.7.0/0.8.0
figures; it is not re-run for priorart 0.1.0.

## Reproduce

Network is needed once, by `fetch.py`; everything else is offline.

    cd priorart
    python eval/benchmark/fetch.py            # fetch the pinned inputs, write cache.lock.json
    python eval/benchmark/fetch.py --check    # cache matches the committed lock
    python eval/benchmark/fetch.py --manifest # regenerate manifest.yaml from labels.yaml
    cargo build                               # priorart 0.1.0
    python eval/benchmark/run.py --priorart target/debug/priorart.exe --split dev   # writes freeze.json
    python eval/benchmark/run.py --priorart target/debug/priorart.exe --split all   # writes results.json
    python eval/benchmark/run.py --self-test  # fixture corpus + fake binary, no network, no priorart
    python eval/benchmark/run.py --check      # results.json still recomputes

`--check` recomputes the label agreement, not the tool measurements: those need
the same binary (its sha256 is in `results.json`). The self-test is the way to
check the harness without a priorart binary at all; CI could run it on every
change.

## Files

| File | Committed | What it is |
|---|---|---|
| `sources.yaml` | yes | pinned repositories, commits, paths, terms note, seed |
| `fetch.py` | yes | network fetch + cache lock + manifest generation |
| `cache.lock.json` | yes | repo, commit and sha256 of every fetched file |
| `labels.yaml` | yes | reader labels, prior refs, provenance, agreement sample |
| `manifest.yaml` | yes | pointers + labels; no finding text |
| `freeze.json` | yes | the freeze the held-out run required |
| `run.py` | yes | scoring, Wilson intervals, three measurements, self-test |
| `results.json` | yes | the numbers above, per query and with the binary hash |
| `eval/cache/benchmark/` | no (gitignored) | fetched finding bodies and built dossiers |
