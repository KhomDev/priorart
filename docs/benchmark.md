# Benchmark: corpus, protocol, three runs, and the release decision

priorart's benchmark is public and multi-researcher. It is not the author's
rejection log (that is the descriptive layer, see [rejections.md](rejections.md)):
it is a judged contest that published its prior audit and a later contest's
judging on the same code. Everything is stored as pointers and labels:
`repo@commit` plus a path or issue id, never finding text.

The committed source of truth is
[`eval/benchmark/BENCHMARK.md`](../eval/benchmark/BENCHMARK.md) and
[`eval/benchmark/results.json`](../eval/benchmark/results.json). This page
summarises them on 2026-11-13; if they disagree, the eval files win.

**Status: FINAL.** Measured on the shipped 0.1.0 build, binary sha256
`dabfcc0c...`. The deciding run was made on `2ec4c3c9...`; `176f1986...`
(matcher identical, wording only) and then `dabfcc0c...` (one last stale-prose
sweep) reproduced the same numbers. Only `dabfcc0c...` is quoted. Three matcher
revisions were measured with the same harness, seed, manifest and dossiers. All
three are printed below; only the third decides the release.

## Corpus

One judged contest pair on one codebase, which is what makes a prior item
retrievable at all: Tapioca was audited by Code4rena in 2023-07 and judged by
Sherlock in 2024-02.

| Role | Pointer | What it provides |
|---|---|---|
| prior audit | `code-423n4/2023-07-tapioca-findings` @ `4193111261e3cf8de27c15857babecaa2519d67c`, `report.md` | 60 High/Medium findings, one file per finding |
| known issues | `sherlock-audit/2024-02-tapioca` @ `dc2464f420927409a67763de6ec60fe5c028ab0e`, `README.md` | the sponsor's answer to "list any known issues" (1 item: zero-address check) |
| judging | `sherlock-audit/2024-02-tapioca-judging` @ `9e52685cd447417a1d9d5c96fb1492d8773eab8a` | 60 accepted findings, 26 excluded findings, 64 submissions grouped as duplicates |

Counts, from `manifest.yaml`: 61 prior items (60 audit + 1 known issue); 86
judged findings (60 paid Medium/High, 26 excluded); 64 judge-grouped duplicate
submissions; 1 finding with a defensible known-prior grounding. Dossier **A** is
built from the prior audit plus the sponsor's known issues, dossier **B** from
the judged primary submissions. Neither is committed; `fetch.py` and
`priorart build` recreate them.

## Protocol

- **Recall@5 on known-prior findings.** Positive = a finding the judge excluded
  *and* whose text repeats one named prior item. A hit = that item in the top 5
  of `priorart check --top-k 5 --json`. This is the plan's headline metric and it
  cannot be measured at adequate n here: the judging repo publishes no exclusion
  reasons, and only one excluded finding grounds in a prior item. **n = 1, 0
  hits. This is not a rate.**
- **False-alarm rate (FAR) on judged paid and unique findings.** Denominator =
  the 60 accepted Medium/High findings; none duplicates another accepted finding
  and none was called known. An alarm = `check` returns at least one prior item.
  By the matcher's own rule (>=2 specific symbol identifiers) any returned item
  is a claim of shared prior art, so the alarm definition is the tool's.
- **Duplicate recall@5 (supplementary).** Denominator = the 64 judge-grouped
  duplicates (54 in accepted groups, 10 in excluded groups). A hit = the group's
  representative in the top 5 from a dossier of the 86 judged representatives.
  This is retrieval of a judged label, not the plan's known-issue metric.
- **Intervals:** Wilson score, 95%, z = 1.959964, computed in `run.py`.
- **Seed and freeze:** seed `priorart-bench-2026-11-12` is committed in
  `sources.yaml`/`labels.yaml` and drives the dev/held-out split, the duplicate
  sample and the agreement sample. `run.py --split dev` writes `freeze.json`
  (manifest sha256 `554207452495d9d2...`, labels sha256, harness sha256, seed,
  k=5). `--split heldout` and `--split all` refuse to run without it and refuse
  if the manifest changed. The reported run used `--split all`, so the held-out
  half was opened after the freeze; `results.json.frozen.heldout_opened` says so.
  No rule was tuned on it.
- **Tool errors:** a per-query crash is recorded as a tool error. The final run
  reports 0.

## The three runs

| Run | Matcher revision | FAR on 60 paid+unique | Duplicate recall@5 | Known-prior | Tool errors |
|---|---|---|---|---|---|
| `22648e33...` | pre-fix: paths counted as identifiers; source-order ties | 18/60 = 30.0% [19.9-42.5] | 36/64 = 56.3% [44.1-67.7] | n=1, 0 hits | 0 |
| `da176bc8...` | symbol identifiers only; generic symbols (>20% of items) excluded; negation-aware | 15/60 = 25.0% [15.8-37.2] | 33/64 = 51.6% [39.6-63.4] | n=1, 0 hits | 0 |
| `dabfcc0c...` **FINAL** (matcher identical to the `2ec4c3c9...` deciding run) | paths never count toward >=2; ties by specificity/rarity/identifier order; tighter negation window; identifiers kept in original case | **26/60 = 43.3% [31.6-55.9]** | **39/64 = 60.9% [48.7-71.9]** | n=1, 0 hits | 0 |

Sha history for the same numbers: `22648e33...` (pre-fix) ->
`da176bc8...` (intermediate) -> `2ec4c3c9...` (the run that decided the release)
-> `176f1986...` (declared, then superseded after one stale NOTES sentence) ->
`dabfcc0c...` (frozen; the row above, and the only one quoted). The matcher was
identical across the last three; only output wording changed.

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

## What this benchmark does not claim

- It does not measure whether priorart finds bugs; the corpus is judged human
  findings.
- It does not measure precision outside this corpus, or on programs whose audits
  are not public.
- A top-5 hit is not necessarily the right prior item: the judged representative
  is one label.
- The duplicate-retrieval number is retrieval of judged group representatives,
  not known-issue detection, and is not comparable to recall@5 on a prior audit.
- n=1 known-prior observation is not a rate, in either direction.
- 43.3% is not a population estimate for "unique findings".
- No held-out tuning happened, but the matcher was not designed against this
  corpus either: this is a retrospective measurement.

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
python eval/benchmark/run.py --check       # recomputes the label agreement
python eval/benchmark/run.py --self-test   # fixture corpus + fake binary; no network, no priorart
```

`--check` recomputes labels and agreement, not the tool measurements: those need
the same binary, whose sha256 is in `results.json`.

**Durable provenance.** The measurement is pinned to the release commit/tag, not
to a binary hash: the published release-profile binary hashes differently again,
so its sha256 is not the one above. The debug-build sha256 in `results.json` is
recorded so a reader can reproduce the run with the same harness and the same
per-query rows; it identifies the measured artifact, not the one you install.
