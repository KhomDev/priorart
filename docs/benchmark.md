# Benchmark: corpora, protocol, measured runs, and the release decision

priorart's benchmark is public and multi-researcher. It is not the author's
rejection log (that is the descriptive layer, see [rejections.md](rejections.md)):
it is a judged contest that published its prior audit and a later contest's
judging on the same code. Everything is stored as pointers and labels:
`repo@commit` plus a path or issue id, never finding text.

There are six corpora on six protocols and two judging platforms (the last two were added for P1-C and are sealed). Judged by
Sherlock: Tapioca (Code4rena 2023-07 -> Sherlock 2024-02) and PoolTogether V5
(Code4rena 2024-03 -> Sherlock 2024-05). Judged by Code4rena: Maia (Code4rena
2023-05 -> Code4rena 2023-09) and Panoptic (Code4rena 2023-11 -> Code4rena
2024-04). This page used to say the Code4rena -> Sherlock pair was the only
public combination that publishes judge-grouped duplicates and excluded
findings. That was wrong: Code4rena publishes its judging as labels on every
findings-repository issue (`duplicate-N`, report ids, `unsatisfactory`,
`downgraded by judge`), and `fetch.py`'s `c4-issues` source reads them.

**Code4rena-judged corpora (2026-11-25, priorart 0.2.0 matcher, Wilson 95%):**

| Corpus | False-alarm rate (paid) | Duplicate recall@5 | Known-prior |
|---|---|---|---|
| Maia | 8/15 = 53.3% [30.1-75.2] | 87/289 = 30.1% [25.1-35.6] | 0/1 (count) |
| Panoptic | 6/11 = 54.5% [28.0-78.7] | 38/80 = 47.5% [36.9-58.3] | 0/3 (count) |
| Pooled C4 (preregistered) | 14/26 = 53.8% [35.5-71.2] | 125/369 = 33.9% [29.2-38.9] | 0/4 (count) |

Maia's interval and the pooled interval lie entirely above the preregistered
30% trigger, so the "shares identifiers" warning stays off and the matcher's
alarm rule is the thing to fix. A harness defect (the last High/Medium slice
of the prior report carries its Low/QA sections) and its post-hoc sensitivity
row are in `eval/benchmark/BENCHMARK.md`, "Third and fourth corpora".

The committed source of truth is
[`eval/benchmark/BENCHMARK.md`](../eval/benchmark/BENCHMARK.md) and the two
`results.json` files. This page summarises them on 2026-11-25; if they
disagree, the eval files win.

**Status: 0.1.0 final, 0.1.3 re-measured, second corpus measured on 0.2.0.**
The Tapioca 0.1.0 number of record is 26/60 = 43.3% on binary `dabfcc0c...`.
The deciding 0.1.0 run was made on `2ec4c3c9...`; `176f1986...` (matcher
identical, wording only) and then `dabfcc0c...` (one last stale-prose sweep)
reproduced the same numbers. 0.1.3 changes the specificity denominator and was
re-measured with the same seed, manifest, dossiers and measurement code:
**15/60 = 25.0% [15.8-37.2]** false alarms and **36/64 = 56.3% [44.1-67.7]**
duplicate recall@5. The held-out split was opened for 0.1.0, so the 0.1.3
number is a re-measurement on the full public corpus, not a fresh
out-of-sample result. Four measured Tapioca runs are printed below. The second
corpus was frozen and measured once on priorart 0.2.0:
**9/40 = 22.5% [12.3-37.5]** false alarms and **32/85 = 37.6% [28.1-48.3]**
duplicate recall@5, with 0/2 known-prior hits (not a rate).

## Corpus

Each corpus is one judged contest pair on one codebase, which is what makes a
prior item retrievable at all. Tapioca was audited by Code4rena in 2023-07 and
judged by Sherlock in 2024-02. PoolTogether V5 was audited by Code4rena in
2024-03 and judged by Sherlock in 2024-05.

| Corpus | Role | Pointer | What it provides |
|---|---|---|---|
| Tapioca | prior audit | `code-423n4/2023-07-tapioca-findings` @ `4193111261e3cf8de27c15857babecaa2519d67c`, `report.md` | 60 High/Medium findings, one file per finding |
| Tapioca | known issues | `sherlock-audit/2024-02-tapioca` @ `dc2464f420927409a67763de6ec60fe5c028ab0e`, `README.md` | the sponsor's answer to "list any known issues" (1 item: zero-address check) |
| Tapioca | judging | `sherlock-audit/2024-02-tapioca-judging` @ `9e52685cd447417a1d9d5c96fb1492d8773eab8a` | 60 accepted findings, 26 excluded findings, 64 submissions grouped as duplicates |
| PoolTogether | prior audit | `code-423n4/2024-03-pooltogether-findings` @ `87f1b837fc3af84b32433b3d82538694b490b9f9`, `report.md` | 9 High/Medium findings (H-01, M-01..M-08) |
| PoolTogether | known issues | `sherlock-audit/2024-05-pooltogether` @ `1aa1b8c028b659585e4c7a6b9b652fb075f86db3`, `README.md` | the sponsor's known-issues answer (1 item: bridged Witnet RNG) |
| PoolTogether | judging | `sherlock-audit/2024-05-pooltogether-judging` @ `88298eacec6f178fd0b5f9f13e4605c58aa58072` | 40 accepted findings, 38 excluded findings, 85 submissions grouped as duplicates |

Counts, from each `manifest.yaml`: Tapioca has 61 prior items (60 audit + 1
known issue), 86 judged findings (60 paid Medium/High, 26 excluded), 64
judge-grouped duplicate submissions and 1 known-prior grounding. PoolTogether
has 10 prior items (9 audit + 1 known issue), 78 judged findings (40 paid
Medium/High, 38 excluded), 85 judge-grouped duplicate submissions and 2
known-prior groundings (`inv:009` repeats M-03; `inv:invalid-010` repeats M-08).
The PoolTogether prior audit covers 6 contracts and 648 lines of the Vault, not
the whole judged codebase, so its 10 prior items are narrower as well as fewer.
Dossier **A** is built from the prior audit plus the sponsor's known issues,
dossier **B** from the judged primary submissions. Neither is committed;
`fetch.py` and `priorart build` recreate them.

## Protocol

- **Recall@5 on known-prior findings.** Positive = a finding the judge excluded
  *and* whose text repeats one named prior item. A hit = that item in the top 5
  of `priorart check --top-k 5 --json`. This is the plan's headline metric and it
  cannot be measured at adequate n on either corpus: the judging repos publish
  no exclusion reasons. Tapioca has **n = 1, 0 hits**; PoolTogether has **n = 2,
  0 hits**. Neither is a rate.
- **False-alarm rate (FAR) on judged paid and unique findings.** Denominator =
  the accepted Medium/High findings; none duplicates another accepted finding
  and none was called known. An alarm = `check` returns at least one prior item.
  By the matcher's own rule (>=2 specific symbol identifiers) any returned item
  is a claim of shared prior art, so the alarm definition is the tool's. The
  denominators are 60 for Tapioca and 40 for PoolTogether and are not
  comparable across corpora.
- **Duplicate recall@5 (supplementary).** Denominator = the judge-grouped
  duplicates (64 Tapioca: 54 in accepted groups, 10 in excluded; 85
  PoolTogether: 52 in accepted groups, 33 in excluded). A hit = the group's
  representative in the top 5 from a dossier of the judged representatives.
  This is retrieval of a judged label, not the plan's known-issue metric.
- **Intervals:** Wilson score, 95%, z = 1.959964, computed in `run.py`.
- **Seeds and freezes:** each corpus has its own committed seed. Tapioca's is
  `priorart-bench-2026-11-12`; PoolTogether's is
  `priorart-bench-pooltogether-2026-11-24`. `run.py --split dev` writes the
  corpus's `freeze.json` (manifest sha256, labels sha256, harness sha256, seed,
  k=5; the new freeze also records the priorart version, binary sha256, source
  commit and source-dirty flag). `--split heldout` and `--split all` refuse to
  run without it and refuse if the manifest or labels changed. Both reported
  full runs used `--split all`, so each held-out half was opened after its
  freeze; `results.json.frozen.heldout_opened` says so. No rule was tuned on
  either. Tapioca's `freeze.json` holds the 0.1.0 harness hash (`57524916...`);
  the 0.1.3 results came from harness `4d9d9bc0...`, which differs only in
  run-history/provenance text. The PoolTogether freeze records priorart 0.2.0,
  binary sha256 `06fdf195...`, source commit `eabd6b3...`, harness
  `7c7a7f16...`, manifest `36eac6fd...` and labels `2400932f...`.
- **Tool errors:** a per-query crash is recorded as a tool error. Both
  published full runs report 0.

## The four runs

| Run | Matcher revision | FAR on 60 paid+unique | Duplicate recall@5 | Known-prior | Tool errors |
|---|---|---|---|---|---|
| `22648e33...` | pre-fix: paths counted as identifiers; source-order ties | 18/60 = 30.0% [19.9-42.5] | 36/64 = 56.3% [44.1-67.7] | n=1, 0 hits | 0 |
| `da176bc8...` | symbol identifiers only; generic symbols (>20% of items) excluded; negation-aware | 15/60 = 25.0% [15.8-37.2] | 33/64 = 51.6% [39.6-63.4] | n=1, 0 hits | 0 |
| `dabfcc0c...` **0.1.0 final** (matcher identical to the `2ec4c3c9...` deciding run) | paths never count toward >=2; ties by specificity/rarity/identifier order; tighter negation window; identifiers kept in original case | **26/60 = 43.3% [31.6-55.9]** | **39/64 = 60.9% [48.7-71.9]** | n=1, 0 hits | 0 |
| `be373973...` **0.1.3** | the generic share is measured over the items that share a symbol with the finding, not over every dossier item | **15/60 = 25.0% [15.8-37.2]** | **36/64 = 56.3% [44.1-67.7]** | n=1, 0 hits | 0 |

Sha history for the same numbers: `22648e33...` (pre-fix) ->
`da176bc8...` (intermediate) -> `2ec4c3c9...` (the run that decided the release)
-> `176f1986...` (declared, then superseded after one stale NOTES sentence) ->
`dabfcc0c...` (0.1.0 final) -> `be373973...` (0.1.3, after the denominator
change). The matcher was identical across the middle three; only output wording
changed. The 0.1.3 run used the full public corpus after the held-out half had
already been opened, so it is a re-measurement, not a held-out result.

Label agreement is unchanged: a blinded second pass on a seeded 20-item sample
of the 26 excluded findings agreed with the full-text label on **15/20 = 75.0%
[53.1-88.8]**. A fourth build, `a973818a...`, is not a row: it panicked on one
input (a char-boundary slice on a curly apostrophe), so its harness run is not
comparable; it is recorded in the errata only.

Final-run diagnostics: the top-1 candidate was the judged representative in
27/64 queries; 35/64 top-1 candidates shared at least one file-level identifier
with the query; the median shared-identifier count was 7; 6 queries returned no
prior item at all, because paths alone no longer qualify and those submissions
carry no qualifying symbol identifiers. The remaining failures are ranking on
shared symbols plus path-only queries with no symbol overlap. That is a defect
report for the matcher, not a property of the corpus.

## The PoolTogether run (2026-11-24)

One corpus, one frozen measurement, on the priorart 0.2.0 release-profile build
(binary sha256 `06fdf195...`; 0 tool errors):

| Metric | Result | Wilson 95% |
|---|---|---|
| False-alarm rate, 40 judged paid+unique | **9/40 = 22.5%** | 12.3-37.5% |
| Duplicate recall@5, 85 judge-grouped duplicates | **32/85 = 37.6%** | 28.1-48.3% |
| Known-prior recall@5 | n=2, 1 hit (0.2.2; heading identifiers) | not a rate |

The two known-prior positives (`inv:009` repeats C4 M-03 on
`maxDeposit()`/`_depositAndMint()`, and `inv:invalid-010` repeats M-08 on
permit/DAI) both returned no prior item. `inv:invalid-010` shares only one
specific identifier with M-08 (`depositWithPermit`), and `inv:009` shares only
`maxDeposit` because M-03's other identifiers are in the report heading, which
the tool does not count as item text. Both are retrieval defects exposed by the
corpus, recorded as such, not label errors.

The 9 false alarms returned M-01 (claimer fees) five times, M-04
(withdraw/redeem slippage) four times and M-06 (missing transfer check) once;
one query returned both M-04 and M-06. Reading the nine alarmed findings, none
states the returned prior item's claim (for example `acc:037-M` is a
`maxRedeem` divide-by-zero revert, not M-04 slippage), so they are semantic
false alarms too. That inspection is a reader judgment, not part of the metric.

Label agreement is **35/38 = 92.1% [79.2-97.3]**. Unlike Tapioca's post-hoc
second look, this title-only pass was recorded before any excluded finding body
was read. It measures label stability of one reader, not correctness.

Ajna was the planned second corpus and was rejected after verification: its
Sherlock judging repository has 18 accepted directories with one file each and
no judge-grouped duplicate submissions, so duplicate recall@5 would have had
n=0. The fallback was used instead; the verification is recorded in
[`eval/benchmark/BENCHMARK.md`](../eval/benchmark/BENCHMARK.md) under "Second
corpus" and in its dead ends.

**Dead ends for this round.** (1) Ajna #2 cannot supply duplicate recall (above).
(2) The work plan's Ajna code commit `276942bc...` belongs to the audited code
repository `code-423n4/2023-05-ajna`, not to the findings repository; the
findings report's own commit is `2ec232ee...`. (3) A C4 report heading is the
prior-item definition even when the report's comments record a judge
downgrading the item: PoolTogether M-03 is listed under Medium Risk Findings
but its comment says Low, so it stays a prior item and the `inv:009` label
carries the note. (4) Do not label excluded findings by identifier overlap:
the same dead end that applies to Tapioca applies here.

## Errata, 2026-11-24

`--corpus-dir` was added to `fetch.py` and `run.py`; the Tapioca directory is
the default and its regenerated `manifest.yaml` is byte-identical (sha256
`55420745...`) after the refactor. The new corpus's `freeze.json` also records
the priorart version, binary sha256, source commit and source-dirty flag
(Tapioca's historical freeze is unchanged). `run.py --check` now verifies the
results corpus hashes and counts against the manifest and labels, the freeze
hashes, and each Wilson interval against its own numerator and denominator. On
Windows, Python's `subprocess` did not resolve a relative `.exe`, so the
measured run passed an absolute path; `results.json.tool.path` records the
repository-relative `target/release/priorart.exe`, never a host path.

## Errata, 2026-11-13

An earlier interim report said duplicate recall@5 was **0/64**. That was the
harness, not the matcher: `run.py` compared the judged id `acc:001-M` with the
dossier item `acc-001-M.md` without normalising `:` to `-`, so every correct
retrieval was scored as a miss. The same binary scored 36/64 after the locator
fix; later matchers measured 33/64 and then 39/64 on the final build. The
`duplicate-diagnostic.json` was unaffected because it normalised the id when it
was written. The harness also found a crash in an intermediate matcher; that is
fixed and the final run reports 0 tool errors.

No benchmark number is printed by `priorart check` in 0.1.0; the numbers live
here and in `results.json`, so there is one place to update and one place to be
wrong.

## Release decision: retrieval only

The plan's kill criterion, quoted: *"If priorart's false-alarm rate on unique
findings is above 30%, it ships with retrieval only (no 'share identifiers'
warning) until the matcher is fixed."*

- Final point estimate: **43.3%**, above 30%.
- 95% interval: **31.6-55.9%**, which does not cross 30%.
- Intervening estimates: 30.0% (pre-fix) and 25.0% (intermediate matcher).

**priorart 0.1.0 therefore ships in retrieval-only mode.** It ranks prior items
and prints them as a reading list with their source, page, section, quote and
shared identifiers; it prints no duplicate warning and implies no duplication.
The 25.0% release decision taken on an intermediate build is superseded by this
one, and the benchmark does not soften that. The next release must fix the
matcher and re-measure before the warning can return; the denominator is one
contest pair (60 judged findings), so a fresh held-out split is required before
the threshold is treated as settled. Quote the FAR with n and the interval,
never 43.3% alone.

**0.1.3 re-measures below the trigger.** The denominator change produces
15/60 = 25.0% [15.8-37.2], below the plan's 30% line. The 0.1.3 release still
ships retrieval-only: the output contract and the reading-list framing are
unchanged, and whether to print a duplicate warning is a product decision, not
something a single re-measurement on an already-open corpus settles. The 25.0%
interval still reaches 37.2%, and it was measured on the same contest pair
whose held-out half was opened for 0.1.0. Quote it with n and the interval.

**PoolTogether against the same trigger.** The second corpus measures
9/40 = 22.5% [12.3-37.5]. The point estimate is below 30%, but the interval
straddles 30%, so it does not establish that the rate is below the trigger.
**The duplicate warning stays off.** Nothing in this corpus turns a warning on;
retrieval-only is unchanged. Tapioca's interval [15.8-37.2] straddles 30% too.
Two corpora now point below the line and neither interval excludes it, so the
criterion is still unsettled and the product decision stays retrieval-only
until a fresh, adequately powered held-out measurement says otherwise.

## What this benchmark does not claim

- It does not measure whether priorart finds bugs; the corpus is judged human
  findings.
- It does not measure precision outside this corpus, or on programs whose audits
  are not public.
- A top-5 hit is not necessarily the right prior item: the judged representative
  is one label.
- The duplicate-retrieval number is retrieval of judged group representatives,
  not known-issue detection, and is not comparable to recall@5 on a prior audit.
- n=1 (Tapioca) and n=2 (PoolTogether) known-prior observations are not rates,
  in either direction.
- 43.3% (0.1.0), 25.0% (0.1.3) and 22.5% (PoolTogether) are not population
  estimates for "unique findings".
- The 0.1.3 denominator change was made to satisfy the unrelated-item
  invariant, not tuned against the labels, but it was measured after the
  held-out split had been opened: it is retrospective, like the 0.1.0 number.
- The PoolTogether corpus has 10 prior items and 40 paid+unique findings; its
  false-alarm denominator is not comparable to Tapioca's 60, and its lower
  point estimate is not evidence that the matcher improved.
- Both corpora share the Code4rena-prior -> Sherlock-judging platform pair.
  Neither measures another judging platform.

## Reproducing

Network is needed once, by `fetch.py`; everything after the cache exists is
offline.

```sh
cd priorart
python eval/benchmark/fetch.py             # pinned inputs; writes cache.lock.json
python eval/benchmark/fetch.py --check     # cache matches the committed lock
cargo build
python eval/benchmark/run.py --priorart target/debug/priorart.exe --split dev   # writes freeze.json
python eval/benchmark/run.py --priorart target/debug/priorart.exe --split all   # writes results.json
python eval/benchmark/run.py --check       # hashes, counts, freeze and agreement
python eval/benchmark/run.py --self-test   # fixture corpus + fake binary; no network, no priorart

# second corpus
python eval/benchmark/fetch.py --corpus-dir eval/benchmark/pooltogether
python eval/benchmark/fetch.py --corpus-dir eval/benchmark/pooltogether --check
python eval/benchmark/fetch.py --corpus-dir eval/benchmark/pooltogether --manifest
python eval/benchmark/run.py --corpus-dir eval/benchmark/pooltogether --priorart target/release/priorart.exe --split dev
python eval/benchmark/run.py --corpus-dir eval/benchmark/pooltogether --priorart target/release/priorart.exe --split all
python eval/benchmark/run.py --corpus-dir eval/benchmark/pooltogether --check
```

`--check` verifies the corpus hashes, counts, freeze and Wilson arithmetic, not
the tool measurements: those need the same binary, whose sha256 is in
`results.json`.

**Durable provenance.** The measurement is pinned to the release commit/tag, not
to a binary hash: the published release-profile binary hashes differently again,
so its sha256 is not the one above. The debug-build sha256 in `results.json` is
recorded so a reader can reproduce the run with the same harness and the same
per-query rows; it identifies the measured artifact, not the one you install.


## P1-C re-measurement (2026-11-25, matcher 0.3.0)

The 0.3.0 alarm rule (a shared claim component; absolute document frequency;
heading identifiers; joined known-issues bullets) was measured on all six
corpora with the frozen binary `6bfc20d3…`. It **does not work**: the sealed
Code4rena corpora are at 3/9 = 33.3% [12.1-64.6] false alarms each (6/18 =
33.3% pooled), above the 30% trigger; Maia and Panoptic are unchanged at
53.3% and 54.5%; and duplicate recall@5 collapses (Maia 30.1% -> 2.4%,
Panoptic 47.5% -> 3.8%, Party 0/132). The harness fixes (last-slice, heading
identifiers, bullet joining) are keepers; the alarm gate is not. The 0.3.0 matcher was reverted; **0.2.2 ships the
harness fixes only** and re-measures the unchanged matcher: in-sample numbers
are materially the same as P1 (Tapioca 15/60 = 25.0% FAR, 35/64 = 54.7%
duplicate recall; Maia 8/15 and Panoptic 6/11 unchanged), and the two
formerly sealed pairs are 3/9 = 33.3% and 4/9 = 44.4%, 7/18 = 38.9% pooled.
The trigger remains met; the warning stays off. Full tables:
`eval/benchmark/BENCHMARK.md`, "P1-C re-measurement results" and "0.2.2
measurement".

## Dup-Bench v1 and the P2 measurement plan (2026-11-26)

The P2 rebuild adds a second benchmark, **Dup-Bench v1** (`eval/dupbench/`), to
measure code-anchored duplicate detection on a temporally sealed split:

- **111 Code4rena contests** with public judge grouping, pinned by findings-repo
  commit, `report.md` hash, `data/` listing and label pages. Span
  2022-11-03..2024-12-10.
- **All 111 labelled**: 1,102 paid-unique findings, 3,454 judged duplicates,
  780 invalid, 101 contests with at least one duplicate group. The judge's
  `duplicate-N` label pointing at the `selected for report` issue is the
  grouping; report↔data joins 1,102/1,102.
- **Temporal split, Amendment 1** (`eval/P2R_PREREGISTRATION_AMENDMENT_1.md`):
  the originally preregistered 2025-09-01/2026-05-01 cut is empty because no
  public judged repository exists after 2024-12 (C4) or 2025-07 (Sherlock).
  Amendment 1 fixes T=2024-03-01, T2=2024-09-01: train 58 contests / 564
  paid-unique / 1,237 duplicates, validation 36 / 380 / 1,441, FINAL 17 / 158 /
  776. FINAL is sealed and has not been read.
- **Six sealed pairs** (`pairs.json`): a prior audit report plus a later judged
  contest for six sponsors not used in P1, with prior-report hashes and contest
  codebase pins. The prior report's commit is not stated by the sponsors, so
  the same-codebase evidence is the sponsor's "Previous audits" statement and
  the drift is declared per pair.

Two earlier P2 measurements exist, and neither is a duplicate-detector number:

- **Code anchors** (`priorart anchors bench`): on the six DEV corpora,
  **1,296 of 1,299 findings that cite code resolve to at least one span
  (99.77%, Wilson 95% [99.32, 99.92])**. It counts a file-level span; symbol
  precision is what the ranker must learn.
- **Mechanism taxonomy**: on 1,997 DEV finding files, 961 (48.1%) assert at
  least one root-cause class and 24.9% abstain. This is cue coverage, not
  judged accuracy; per-class precision is unmeasured.

**Dup-Bench ranker (PA-B4), measured 2026-11-26; reported, not a verdict.** The
fixed-weight ranker (`eval/dupbench/weights/v1.json`; logistic regression
fitted by `eval/dupbench/train_ranker.py`, seed 20261126) was measured with
`eval/dupbench/build_ranker_data.py` over 83 contests whose checkout is at the
pinned commit: 3,380 queries (train 1,163 judged duplicates + 472 paid-unique
primaries against the other primaries; validation 1,392 + 353). The selection
grid chose `log1p` count transforms and lambda 10.0 on validation log-loss
(0.2193; validation AUC 0.9182). Train, in-sample: duplicate recall@5
631/1,163 = 54.3% [51.4, 57.1] with the ranker versus 627/1,163 = 53.9%
[51.0, 56.8] in the default order; FAR on paid-unique findings 34/472 = 7.2%
[5.2, 9.9] at score 0.5 (baseline, any candidate: 269/472 = 57.0%
[52.5, 61.4]); duplicate abstention 476/1,163 = 40.9% [38.1, 43.8].
Validation, model-selection data and never a fresh result: recall@5
990/1,392 = 71.1% [68.7, 73.4] versus 977/1,392 = 70.2% [67.7, 72.5]; FAR
48/353 = 13.6% [10.4, 17.6] at 0.5, 11/353 = 3.1% [1.8, 5.5] at 0.8
(baseline 211/353 = 59.8% [54.6, 64.8]); duplicate abstention 309/1,392 =
22.2% [20.1, 24.5]. The validation decile curve is non-decreasing within
sampling error (one near-inversion inside a large score tie). **No threshold is
shipped**: the calibrated alarm is PA-B5's item. FINAL is sealed and no FINAL
contest has been read; none of these numbers is a FINAL number.

## DEV-ONLY release table (P2R-2, the release on `main`, 2026-11-27)

> **DEV-ONLY. None of these is a held-out number.** Train is the ranker's fitting data. Validation is
> model-selection data: the ranker's grid was chosen on it, and the alarm was calibrated on it. The held-out
> rounds (the six V1 pairs, then Dup-Bench FINAL) come after main, under
> `eval/P2R2_PREREGISTRATION_POST_MAIN.md`.

Sources: `eval/dupbench/weights/v1.json` (`measurement`) and `eval/dupbench/weights/alarm-v1.json`.
The intervals are Wilson 95% with z = 1.959964.

| Cell | Train (in-sample, fitting) | Validation (model selection) |
|---|---|---|
| Duplicate recall@5, ranker | 631/1,163 = 54.3% [51.4, 57.1] | 990/1,392 = 71.1% [68.7, 73.4] |
| Duplicate recall@5, default order | 627/1,163 = 53.9% [51.0, 56.8] | 977/1,392 = 70.2% [67.7, 72.5] |
| Duplicate abstention | 476/1,163 = 40.9% [38.1, 43.8] | 309/1,392 = 22.2% [20.1, 24.5] |
| FAR on paid-unique, ranker score ≥ 0.5 | 34/472 = 7.2% [5.2, 9.9] | 48/353 = 13.6% [10.4, 17.6] |
| FAR on paid-unique, ranker score ≥ 0.8 | - | 11/353 = 3.1% [1.7, 5.5] |
| FAR on paid-unique, ranker score ≥ 0.9 | 2/472 = 0.4% [0.1, 1.5] | 2/353 = 0.6% [0.2, 2.0] |
| FAR on paid-unique, ranker score ≥ 0.95 | - | 0/353 = 0.0% [0.0, 1.1] |
| Alarm warning precision at the shipped threshold (calibrated ≥ 0.9016) | - | 149/162 = 92.0% [86.8, 95.3] (the data it was calibrated on) |

The 0.8 row's lower bound reads 1.8 in the PA-B4 paragraph above. Recomputed, it is 1.749, which is
1.7 to one decimal. The interval is otherwise unchanged.

**Pairs: not measured.** The pairs swap (option A) sealed `2022-11-paraspace`, `2023-02-ethos`,
`2023-09-venus` and `2023-07-arcade`, and kept `2024-03-revert-lend` and `2024-10-loopfi`. None of them
has judge labels collected yet, so there is no pairs number (`eval/dupbench/pairs-swap/SELECTION.md`).
The earlier PoolTogether / Ondo / Numoen pairs-DEV output only proved that the runner works and is not a
measurement.

**History (in-sample, 0.2.2):** Tapioca FAR 15/60 = 25.0% [15.8, 37.2], recall 35/64 = 54.7%
[42.6, 66.3]; Party + Wildcat (the former sealed pairs) FAR 7/18 = 38.9% [20.3, 61.4], recall 63/178 =
35.4% [28.7, 42.7]. See `docs/limitations.md`.