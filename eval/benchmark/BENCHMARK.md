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

A second corpus on a different protocol was added on 2026-11-24 and is
described under "Second corpus: PoolTogether V5" below. It uses the same
platform pair (Code4rena prior audit -> Sherlock judging) because it was
believed to be the only public combination that publishes both judge-grouped
duplicate submissions and excluded findings; the protocol and codebase are new.
(Correction, 2026-11-25: that was wrong. Code4rena publishes its judging as labels on every findings-repository issue; the third and fourth corpora below are judged on Code4rena.) Two Code4rena-judged corpora (Maia, Panoptic) were added on
2026-11-25 under "Third and fourth corpora". The Tapioca corpus, its manifest
and every number below are unchanged.

Status: 0.1.0 FROZEN, dated 2026-11-13, measured on the shipped priorart 0.1.0
binary, sha256
`dabfcc0ca197ee35b40772cb7db666f17105c5c36faa7f9da7ef783a9a78e12a`
(priorart-core gate: fmt --check 0, clippy -D warnings clean, cargo test
--locked 91 passed: 56 lib + 33 cli + 2 docs; README generated block current).
**0.1.3 re-measured on 2026-11-21** after the specificity denominator changed:
15/60 = 25.0% [15.8%, 37.2%] false alarms and 36/64 = 56.3% [44.1%, 67.7%]
duplicate recall@5, 0 tool errors, measured binary
`be3739736afdaee6de26a3a3dda50f5549714ce2a05a34d46da9626b538531de`. The
0.1.0 measurement detail follows.
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

**Release provenance.** `dabfcc0c...` is the debug binary this harness measured.
It is not the crates.io or GitHub-release artefact: release-profile builds have a
different hash. The durable provenance is the source commit and tag the release is
cut from: **v0.1.0, commit `2c343904a74a79a184fe9cebee3cc9b67367108d`**, recorded in
`results.json.release` together with the release executable hash
(`b5a8cbf8...`, the x86_64-unknown-linux-gnu binary inside
`priorart-v0.1.0-x86_64-unknown-linux-gnu.tar.gz`). The release was built from that
commit and the crate was published from it. Quoting `dabfcc0c...` without the
source commit is only valid for this benchmark run, not as a release identifier.

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

Four binaries were measured with the same seed, manifest, dossiers and
measurement code (the harness differs only in the 0.1.3 provenance text). The
third is the 0.1.0 number of record; the fourth is 0.1.3; the earlier two are
provenance.

| Run | Matcher revision | FAR on 60 paid+unique | Duplicate recall@5 | Known-prior recall@5 | Tool errors |
|---|---|---|---|---|---|
| `22648e33...` (2026-11-12) | pre-fix: paths counted as identifiers; source-order ties | 18/60 = 30.0% [19.9%, 42.5%] | 36/64 = 56.3% [44.1%, 67.7%] | n=1, 0 hits, not a rate | 0 |
| `da176bc8...` (2026-11-13) | symbol identifiers only; generic symbols (>20% of items) excluded; negation-aware | 15/60 = 25.0% [15.8%, 37.2%] | 33/64 = 51.6% [39.6%, 63.4%] | n=1, 0 hits, not a rate | 0 |
| `dabfcc0c...` 0.1.0 FINAL (2026-11-13) | + paths never count toward >=2 (shown only); ties by specificity/rarity/identifier order; negation window tightened; identifiers stored in original case | **26/60 = 43.3% [31.6%, 55.9%]** | **39/64 = 60.9% [48.7%, 71.9%]** | n=1, 0 hits, not a rate | 0 |
| `be373973...` 0.1.3 (2026-11-21) | generic share measured over the items that share a symbol with the finding, not every dossier item | **15/60 = 25.0% [15.8%, 37.2%]** | **36/64 = 56.3% [44.1%, 67.7%]** | n=1, 0 hits, not a rate | 0 |

The deciding run on `2ec4c3c9...`, the intermediate build `a2dfd8e0...` and
the declared-then-superseded `176f1986...` produced the same numbers as the
frozen 0.1.0 row; only `dabfcc0c...` is quoted for 0.1.0. A
build `a973818a...` is not a row: it panicked on one input (a char-boundary
slice on a curly apostrophe) and that harness run is not comparable; it is
recorded in the provenance paragraph only. The 0.1.3 row is a re-measurement
on the full public corpus after the held-out half was opened for 0.1.0; it is
not a fresh out-of-sample result. `freeze.json` holds the 0.1.0 harness hash
(`5752491659b1...`); the 0.1.3 run used harness `4d9d9bc0697f...`, which differs
only in the run-history/provenance text and the corrected `not_claimed` line,
not in the measurement code.

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

**0.1.3 re-measurement and the same rule.** The specificity denominator now
counts only items that share at least one symbol with the finding. On the same
corpus, seed, manifest and harness the measured FAR is 15/60 = 25.0%
[15.8%, 37.2%], below the 30% trigger, and duplicate recall@5 is 36/64 = 56.3%
[44.1%, 67.7%]. The 0.1.3 release still ships retrieval-only: the 30% line is a
release rule, not a mandate to turn on a warning, and the interval still reaches
37.2% on a corpus whose held-out half was opened for 0.1.0. A fresh held-out
split is still required before either threshold is treated as settled. Quote
the 0.1.3 FAR with n and the interval, never 25.0% alone.

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

## Second corpus: PoolTogether V5 (2026-11-24)

The second corpus is PoolTogether V5: Code4rena audited the PrizeVault in
2024-03 and Sherlock judged the wider V5 codebase in 2024-05. It is the same
platform pair as Tapioca (C4 prior audit -> Sherlock judged contest) because
it was believed to be the only public combination that publishes both
judge-grouped duplicate submissions and excluded findings. (Correction, 2026-11-25: that was wrong. Code4rena publishes its judging as labels on every findings-repository issue; the third and fourth corpora below are judged on Code4rena.) The
protocol and codebase are different; the platform pair is not.

**Ajna was tried first and rejected.** The planned pair was
`code-423n4/2023-05-ajna-findings` (25 High/Medium report headings) plus
`sherlock-audit/2023-09-ajna-judging`. The C4 report does yield prior items,
but the Sherlock judging repository on `main` (commit
`9de6bfb6aef1b4d3dd9065f09ba142d052d0d128`, checked 2026-11-24) has 18 accepted
directories (`001-M`..`018-H`), each containing exactly one `*-best.md` file,
and no
`invalid/invalid-NNN/` duplicate groups: zero judge-grouped duplicate
submissions reachable from the judge's own layout. Duplicate recall@5 would
have had n=0, so the corpus cannot satisfy the harness. The fallback named in
the work plan was used instead, and this paragraph is the recorded reason. (The
work plan's Ajna code commit `276942bc...` belongs to the audited code
repository `code-423n4/2023-05-ajna`, not to the findings repository; the
findings report's own commit is `2ec232ee...`.)

| Role | Pointer | What it provides |
|---|---|---|
| prior audit | `code-423n4/2024-03-pooltogether-findings` @ `87f1b837fc3af84b32433b3d82538694b490b9f9`, `report.md` | 9 High/Medium findings: H-01 and M-01..M-08 |
| known issues | `sherlock-audit/2024-05-pooltogether` @ `1aa1b8c028b659585e4c7a6b9b652fb075f86db3`, `README.md` | the sponsor's known-issues answer (1 item: the bridged Witnet RNG) |
| judging | `sherlock-audit/2024-05-pooltogether-judging` @ `88298eacec6f178fd0b5f9f13e4605c58aa58072` | 40 accepted finding directories and 38 excluded findings, including 10 excluded duplicate groups; 85 judge-grouped duplicate submissions |

Counts, from `eval/benchmark/pooltogether/manifest.yaml`
(`fetch.py --corpus-dir eval/benchmark/pooltogether --manifest`):

- 10 prior items (9 audit + 1 known issue)
- 78 judged findings: 40 paid Medium/High, 38 excluded
- 85 judge-grouped duplicate submissions (52 in accepted groups, 33 in excluded groups)
- 2 findings with a defensible known-prior grounding: `inv:009` repeats
  M-03 (`maxDeposit()` vs `_depositAndMint()`), `inv:invalid-010` repeats M-08
  (permit vs DAI)

The C4 audit covers 6 contracts and 648 lines of the Vault, not the whole
Sherlock contest scope, so the prior-item set is both smaller (10 vs 61) and
narrower than Tapioca's. The false-alarm denominators are therefore not
comparable across the two corpora.

**Freeze.** `run.py --corpus-dir eval/benchmark/pooltogether --split dev` was
run before any held-out query, and wrote `freeze.json`:
manifest sha256 `36eac6fdfec56a7a7169610d945913c7034ce3da09115d2a40ee6c441576fe0f`,
labels sha256 `2400932fca934b981e54226184bcb6a5232dee32e3ed21420e53cac8f65551d5`,
harness sha256 `7c7a7f167d11916e2d829825dc6970425c05855b24957cfb4a082f25eb06a015`,
seed `priorart-bench-pooltogether-2026-11-24`, k=5. The freeze records the
matcher: priorart 0.2.0, binary sha256
`06fdf195989e5c6e739285047e29dc3fe873252ee567e52e16e24f3804a85783`, source
commit `eabd6b3b6399466725cf83e1ff4bd558e0ec9000`, `source_dirty: false` (no
change under `src/`, `Cargo.toml` or `Cargo.lock`). The full run then used
`--split all`; `results.json.frozen.heldout_opened` is `true`. This is the
first measurement on this corpus and it opened the frozen held-out half to
produce the published number, so no held-out remainder is left; it is
out-of-sample with respect to matcher development.

**Measurements on the full corpus** (0 tool errors, Wilson 95%):

| Metric | Result | Wilson 95% |
|---|---|---|
| False-alarm rate on 40 judged paid+unique | **9/40 = 22.5%** | [12.3%, 37.5%] |
| Duplicate recall@5 on 85 judge-grouped duplicates | **32/85 = 37.6%** | [28.1%, 48.3%] |
| Known-prior recall@5 | n=2, 0 hits | not a rate; [0%, 65.8%] is arithmetic only |
| Label agreement (blinded sample = all 38 excluded) | **35/38 = 92.1%** | [79.2%, 97.3%] |

The known-prior positives are `inv:009` (M-03) and `inv:invalid-010` (M-08).
Both returned no prior item at all. The diagnostics show why, and the reasons
differ: `inv:invalid-010` shares only one specific identifier with M-08
(`depositWithPermit`; `PrizeVault` appears in too many items to count), so the
matcher's >= 2 rule cannot fire; `inv:009` shares only `maxDeposit` with M-03
because that prior item's other identifiers (`_depositAndMint`,
`yieldVault.mint`) sit in the report's heading line, which the tool records as
`section` and does not count as item text. Both are retrieval defects exposed by
the corpus, not label errors, and n=2 is not a rate in either direction.

**Label agreement for this corpus is stronger than Tapioca's by construction.**
The title-only pass was recorded before any excluded finding body was read, so
it is a genuine blind pass, not a post-hoc second look by the same reader who
had already read the bodies. It still measures label stability of one reader,
not correctness. The three disagreements are `inv:009` (the reader grounded it
in M-03 after reading the body; the title-only pass did not), `inv:085` and
`inv:118` (title-only guessed a prior item from a shared topic; the full text
did not reproduce that prior item's claim).

**The 9 false alarms, inspected.** Five returned M-01 (claimer fees), four
returned M-04 (withdraw/redeem slippage), and one returned both M-04 and M-06
(the counts sum to ten over nine queries). Reading the nine alarmed findings,
none states the returned item's claim: `acc:037-M` is a `maxRedeem` divide-by-
zero revert, not M-04 slippage; `acc:005-M` is a claimer receiving less than
expected because prizes were already claimed, not M-01's winner-griefs-claimer
fee theft. The FAR therefore counts them as alarms under the tool's own rule
(any returned item is a claim of shared prior art), and on inspection they are
semantic false alarms too. This inspection is a reader judgment and is not
part of the measured metric.

**Ship decision against the pre-registered 30% trigger.** The rule: above 30%
false alarms on judged unique findings, the release ships retrieval only, with
no "shares identifiers" warning, until the matcher is fixed. For this corpus
the point estimate is 22.5%, below 30%, but the 95% interval is
[12.3%, 37.5%] and it straddles 30%. An interval that straddles the trigger
does not establish that the rate is below it, so **the duplicate warning stays
off**. The Tapioca 0.1.3 interval [15.8%, 37.2%] also straddles 30%. Two
corpora now agree on the point direction (both below 30%) and on the
uncertainty (both intervals include 30%); neither settles the trigger.
Retrieval-only is unchanged, and no measurement here turns a warning on.

**Harness errata, 2026-11-24.** `--corpus-dir` was added to `fetch.py` and
`run.py`; the Tapioca directory is the default and its regenerated
`manifest.yaml` is byte-identical (sha256
`554207452495d9d2f14abb1ff794fa9782bdcfc792d638f8d9deda97333bf913`) after the
refactor. `freeze.json` now also records the priorart version, binary sha256,
source commit and source-dirty flag. `--check` now verifies the results corpus
hashes and counts against the manifest and labels, the freeze hashes, and the
Wilson intervals against their own numerators. On Windows, Python's
`subprocess` did not resolve a relative `.exe` path, so the measured run passed
an absolute binary path; `results.json.tool.path` records the repository-
relative path `target/release/priorart.exe`, never a host path.

## Third and fourth corpora: a second judging platform (preregistered 2026-11-25)

**Why.** Both earlier corpora are judged by Sherlock. The P1 statement that the
Code4rena -> Sherlock pair is "the only public combination found that
publishes both judge-grouped duplicate submissions and excluded findings" was
wrong: Code4rena publishes its judging as labels on every issue of the public
findings repository (`duplicate-N`, report ids `H-NN`/`M-NN`, `satisfactory`,
`unsatisfactory`, `downgraded by judge`, `withdrawn by judge`). These two
corpora measure the same frozen matcher on Code4rena judging.

**Selection rule, written before any measurement.** Candidates: protocols with
two public Code4rena findings repositories, the earlier one supplying the prior
audit (`report.md` High/Medium headings), the later one supplying the judging.
Eligible: (a) judging not on Sherlock; (b) >= 30 judge-grouped duplicate
submissions; (c) >= 20 excluded findings; (d) >= 50% of the later contest's
non-test Solidity file names also present in the earlier contest's repository.
Among eligible pairs, take the two with the most paid findings (the
false-alarm denominator). Counts on 2026-11-25:

| Pair (prior -> judged) | Paid | Excluded groups | Duplicates | Prior H/M | File overlap | Eligible |
|---|---:|---:|---:|---:|---:|---|
| Maia 2023-05 -> 2023-09 | 15 | 411 | 289 | 79 | 86% | yes |
| Panoptic 2023-11 -> 2024-04 | 11 | 373 | 80 | 7 | 56% | yes |
| Party 2023-04 -> 2023-10 | 9 | 323 | 132 | 20 | 78% | yes, not chosen (fewer paid) |
| Wildcat 2023-10 -> 2024-08 | 9 | 52 | 46 | 17 | 55% | yes, not chosen |
| PoolTogether 2023-07 -> 2024-03 | 9 | 97 | 169 | - | - | not chosen (same protocol as the second corpus) |
| Munchables 2024-05 -> 2024-07 | 6 | 28 | 278 | 6 | 94% | yes, not chosen |
| Basin 2023-07 -> 2024-07 | 4 | 16 | 56 | 14 | 81% | no: (c) |
| Reserve 2023-07 -> 2024-07 | 7 | 83 | 18 | - | - | no: (b) |
| DittoETH, LoopFi, Superposition, Badger | <= 7 | <= 19 | <= 25 | - | - | no: (b) or (c) |
| Particle 2023-05 -> 2023-12 | 19 | 13 | 26 | 10 | 0% | no: (c), (d) |

(The excluded-group counts in this table come from the selection script; the
manifest counts below, after the mapping rules, are the ones measured.)

**Label mapping (`fetch.py` `c4_layout`), frozen with the corpora.** In scope:
an issue with a High/Medium severity label, a report id, or `downgraded by
judge`; QA and Gas reports and `withdrawn by warden` issues are out. A group is
the transitive closure of `duplicate-N` labels. A group is **paid** when any
member carries a report id; its representative is that member. Every other
group is **excluded**, including H/M submissions the judge downgraded to QA. A
member whose root is not in scope (a QA report that absorbed it) is a single
excluded finding. Every duplicate-labelled member counts as a judge-grouped
duplicate, credited (`satisfactory`, `partial-*`) or not: the measurement is
retrieval of the judge's grouping, not the award.

**Mapping verification, before any label or run.** Each later contest's own
`report.md` lists every paid finding with the issue links of its submitter
and of everyone credited. Maia: 15 report items, 15 paid groups, 15 matched
one-to-one with the same severity. Panoptic: 11 of 11. Group membership: the
Panoptic groups hold 28 issues, all linked from their report sections; the
Maia groups hold 158, of which 133 are linked in the first 4,000 characters
of their section; the other 25 are 23 credited duplicates of #885 past that
window, one `partial-*` and one `unsatisfactory` duplicate, which the rule above
includes.

| Corpus | Prior audit | Known issues | Judging |
|---|---|---|---|
| `maia-c4` | `code-423n4/2023-05-maia-findings` @ `fa2bd134`, `report.md` (79 H/M) | `code-423n4/2023-09-maia` @ `1aa8a7b5`, README "Publicly Known Issues" (6) | `code-423n4/2023-09-maia-findings` issues (904), fetched 2026-11-25 |
| `panoptic-c4` | `code-423n4/2023-11-panoptic-findings` @ `85bd107f`, `report.md` (7 H/M) | `code-423n4/2024-04-panoptic` @ `a65595a1`, README known-issues list (21 lines) | `code-423n4/2024-04-panoptic-findings` issues (581), fetched 2026-11-25 |

Manifest counts: Maia 85 prior items, 402 judged findings (15 paid, 387
excluded), 289 judge-grouped duplicates. Panoptic 28 prior items (the
known-issues list is split per line, so wrapped bullets count more than once),
367 judged findings (11 paid, 356 excluded), 80 duplicates.

**Labels, preregistered.** The paid findings carry the platform label
`unique_valid`. Excluded findings are too many to read in full (387 and 356),
so the known-prior label is read on a seeded sample: the 20 excluded findings
per corpus with the smallest `sha256(seed + ":label:" + id)`. Each gets
`known_prior` (with `prior_ref`) or `invalid_unmapped`, by the PoolTogether
rubric; every other excluded finding stays `unlabelled` and is not a
known-prior query. A blinded second reader labels the same 40 from the title
and the prior-item list before the first reader's labels are visible to it;
agreement is reported as a count and a Wilson interval.

**Measurement, preregistered.** Matcher frozen at priorart 0.2.0, binary
sha256 `06fdf195989e5c6e739285047e29dc3fe873252ee567e52e16e24f3804a85783`, the
same binary as the PoolTogether run. `run.py --split dev` writes `freeze.json`
first; `--split all` produces the published numbers. Per corpus: false-alarm
rate on the paid findings, duplicate recall@5 on the judge-grouped duplicates,
known-prior hits as a count when n < 5. Pooled across the two Code4rena
corpora: the summed counts with a Wilson interval, labelled pooled. The 30%
trigger is applied to each interval and to the pooled one; a straddling
interval keeps the warning off.

**Freeze.** `run.py --split dev` wrote `freeze.json` for each corpus before
any held-out query (commit `6bce457`): priorart 0.2.0, binary sha256
`06fdf195989e5c6e739285047e29dc3fe873252ee567e52e16e24f3804a85783`,
`source_dirty: false`. The full run used `--split all`, so no held-out
remainder is left; the numbers are out-of-sample with respect to matcher
development (no matcher change since 0.1.3).

**Measurements** (0 tool errors, Wilson 95%):

| Corpus | False-alarm rate (paid) | Duplicate recall@5 | Known-prior | Label agreement |
|---|---|---|---|---|
| Maia (C4 -> C4) | **8/15 = 53.3% [30.1-75.2]** | **87/289 = 30.1% [25.1-35.6]** | 0/1 (count) | 19/20 = 95.0% [76.4-99.1] |
| Panoptic (C4 -> C4) | **6/11 = 54.5% [28.0-78.7]** | **38/80 = 47.5% [36.9-58.3]** | 0/3 (count) | 19/20 = 95.0% [76.4-99.1] |
| Pooled C4 (preregistered) | **14/26 = 53.8% [35.5-71.2]** | **125/369 = 33.9% [29.2-38.9]** | 0/4 (count) | 38/40 = 95.0% [83.5-98.6] |
| PoolTogether (C4 -> Sherlock), for comparison | 9/40 = 22.5% [12.3-37.5] | 32/85 = 37.6% [28.1-48.3] | 0/2 (count) | 35/38 |
| Tapioca 0.1.3 (C4 -> Sherlock), for comparison | 15/60 = 25.0% [15.8-37.2] | 36/64 = 56.3% [44.1-67.7] | 0/1 (count) | 15/20 |

Duplicate recall by the kind of group the duplicate belongs to: Maia paid
groups 44/143 = 30.8% [23.8-38.8], excluded groups 43/146 = 29.5%
[22.7-37.3]; Panoptic paid groups 13/17 = 76.5% [52.7-90.4], excluded groups
25/63 = 39.7% [28.5-52.0].

**The false alarms, inspected (reader judgment, not part of the metric).**
Panoptic: all six alarms return prior H-02 (partial transfers and incorrect
storage updates in the SFPM); none of the six paid findings states that claim
(for example `acc:497-H` is `settleLongPremium` deducting premium in the wrong
direction). Maia: the eight alarms return subsections of prior H-05 (decimal
scaling, "Issue #2/#3"), M-10, H-31, M-27, H-27 and M-29; reading them, none of
the paid findings restates the returned item's claim (`acc:710-M` is
`_performFallbackCall` not refunding excess gas, which shares only the
function family with H-27's unhandled return value).

**Harness defect found by these corpora (affects all four corpora).**
`write_dossier_a` cuts the prior report into one file per High/Medium heading,
and the last slice runs to the end of `report.md`, so it also carries the
report's Low/QA and gas sections. On Maia that slice (`M-44`) becomes 15
dossier items, and one Maia alarm (`acc:564-M`) returned only such an item
(`[L-08] Missing Event for initialize`). The measured numbers above use the
frozen harness and stand. A post-hoc sensitivity row, not preregistered:
without that one alarm Maia is 7/15 = 46.7% [24.8-69.9] and the pooled C4 FAR
13/26 = 50.0% [32.1-67.9]. Panoptic's last slice (`M-05`) leaks one QA summary
item that never appears without H-02, so Panoptic does not change. The fix
(stop the last slice at the next top-level section) is a harness change for a
new dated measurement of all four corpora, not for this one.

**Ship decision against the preregistered 30% trigger.** Maia's interval
[30.1, 75.2] and the pooled C4 interval [35.5, 71.2] lie entirely above 30%;
Panoptic's [28.0, 78.7] straddles it. This is the first corpus on which the
trigger is met, not merely not excluded: on Code4rena-judged contests with a
large prior report, the frozen matcher returns a prior item for about half of
the paid, unique findings. **The "shares identifiers" warning stays off**,
retrieval-only is unchanged, and the matcher's alarm rule (any returned item at
>= 2 strong identifiers) is the thing to fix before any warning ships.
Why the rate is higher here than on the Sherlock corpora is not established;
the prior dossier is larger (Maia 108 dossier items vs PoolTogether 11), and a
larger dossier gives the >= 2-identifier rule more chances to fire.

**Known-prior recall.** Four positives in the labelled samples (Maia M-04;
Panoptic M-02, known issues 16 and 7); none returned its prior item in the top
5. n=4 is a count, not a rate.

**What these corpora do not settle.** Two protocols, one judging platform per
corpus pair; one AI first reader with a blinded AI second reader on the
known-prior samples only (the paid and duplicate labels are the judge's own);
for Maia the first reader's sample labels were written after the blind labels
were available. The excluded findings outside the seeded samples are not
labelled, so known-prior recall covers 40 of 743 excluded findings.

**Harness errata, 2026-11-25.** `fetch.py` gained the `c4-issues` source kind
and per-corpus `known_issue_pattern`; `run.py` reads `platform`, `url` and
`known_issue_pattern` from the corpus block with the old values as defaults.
The Tapioca and PoolTogether manifests regenerate byte-identically and their
`--check` runs pass. After the freeze, `corpus_meta()` was made to fall back to
the defaults when a corpus has no `sources.yaml` (the `--self-test` fixture);
the self-test output is identical to the pre-change harness, and no scored
code path changed.

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
7. **Ajna #2 does not publish judge-grouped duplicates in its judging
   repository.** Checked 2026-11-24 at
   `sherlock-audit/2023-09-ajna-judging` on `main` (checked 2026-11-24): 18
   accepted directories, one `*-best.md` each, and no
   `invalid/invalid-NNN/` groups. Do not build a duplicate-recall corpus on it.
   The C4 report itself is usable (25 High/Medium headings), so a future
   corpus that only needs one prior audit could revisit it.
8. **Judge comments that downgrade an item do not remove it from the report.**
   C4 M-03 in the PoolTogether report carries a judge comment decreasing it to
   Low, but the report still lists it under Medium Risk Findings. The manifest
   is generated from the report headings, so M-03 is a prior item; the nuance
   is recorded on the `inv:009` label. Do not silently filter such rows without
   a new corpus version.

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
- The PoolTogether numbers are not population estimates: 40 paid+unique
  findings, 10 prior items, one protocol, and a prior audit that covered only
  part of the judged codebase. Its FAR denominator is not comparable to
  Tapioca's, and a lower FAR there does not mean the matcher improved.
- The PoolTogether known-prior count (n=2, 0 hits) is not a rate. Both
  positives missed, which is a matcher recall defect on this corpus; it does
  not license a recall estimate.
- The two corpora share a platform pair (C4 prior -> Sherlock judging); neither
  measures behaviour on Code4rena judging or any other combination.

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

The second corpus uses the same commands with `--corpus-dir
eval/benchmark/pooltogether`; its cache lives in
`eval/cache/benchmark/pooltogether/` and its committed files are in
`eval/benchmark/pooltogether/`:

    python eval/benchmark/fetch.py --corpus-dir eval/benchmark/pooltogether
    python eval/benchmark/fetch.py --corpus-dir eval/benchmark/pooltogether --check
    python eval/benchmark/fetch.py --corpus-dir eval/benchmark/pooltogether --manifest
    python eval/benchmark/run.py --corpus-dir eval/benchmark/pooltogether --priorart target/release/priorart.exe --split dev
    python eval/benchmark/run.py --corpus-dir eval/benchmark/pooltogether --priorart target/release/priorart.exe --split all
    python eval/benchmark/run.py --corpus-dir eval/benchmark/pooltogether --check

`--check` verifies the corpus hashes, counts, freeze and Wilson arithmetic. It
does not re-run the tool measurements: those need the same binary (its sha256
is in `results.json`). The self-test is the way to check the harness without a
priorart binary at all; CI could run it on every change.

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
| `pooltogether/sources.yaml` | yes | the second corpus: pinned repositories, commits, paths, terms note, seed |
| `pooltogether/cache.lock.json` | yes | repo, commit and sha256 of every fetched PoolTogether file |
| `pooltogether/labels.yaml` | yes | reader labels, prior refs, provenance, blind agreement sample |
| `pooltogether/manifest.yaml` | yes | pointers + labels; no finding text |
| `pooltogether/freeze.json` | yes | the freeze written before the full run, with the matcher's version, binary sha256 and source commit |
| `pooltogether/results.json` | yes | the PoolTogether numbers, per query and with the binary hash |
| `eval/cache/benchmark/` | no (gitignored) | fetched finding bodies and built dossiers (Tapioca) |
| `eval/cache/benchmark/pooltogether/` | no (gitignored) | fetched finding bodies and built dossiers (PoolTogether) |

## P1-C matcher revision and fresh corpora (preregistered 2026-11-25)

Written before the revised matcher is run on any corpus. The P1 numbers above
are unchanged; the new numbers are added beside them with their dates.

**What changes in the matcher (0.3.0).**

1. The alarm rule requires a **shared claim component**: at least one shared
   specific symbol with document frequency 1 in the relevant set, or a shared
   file path. Two shared identifiers alone no longer alarm.
2. Specificity is **absolute**: a shared symbol may appear in at most
   `MAX_SPECIFIC_DF = 2` items. Before this, the threshold was also a share of
   the relevant items, so a larger dossier made more identifiers look specific
   and raised alarms by itself.
3. A prior item's **heading identifiers** are counted (the PoolTogether
   `inv:009` / M-03 miss).
4. A wrapped **known-issues bullet is one item**; continuation lines join the
   bullet above them.

**Harness fix.** The last High/Medium slice of a prior `report.md` stops at the
next top-level section instead of running to the end of the file, so Low/QA/gas
sections no longer enter dossier A.

**Corpora.** Four in-sample corpora (their held-out halves were opened in P1):
Tapioca, PoolTogether, Maia, Panoptic. Two **sealed** corpora are the
out-of-sample headline, pinned before the matcher change:
`party-c4` (Party 2023-04 -> 2023-10) and `wildcat-c4` (Wildcat 2023-10 ->
2024-08). The paid-finding mapping was verified one-to-one against each later
contest's `report.md` before any label or run (Party 9/9, Wildcat 9/9, same
severity).

**Analysis plan.**

- False-alarm rate: paid, unique findings for which any prior item is
  returned; duplicate recall@5 against the judged representatives; known-prior
  recall@5 on the seeded sample of excluded findings.
- Every rate carries n and a Wilson 95% interval; pooled rates also carry a
  **source-clustered bootstrap** interval (resample sources, 10,000 draws,
  seed `priorart-bootstrap-p1c-2026-11-25`).
- The preregistered 30% trigger is unchanged: above 30% false alarms on judged
  unique findings, the release ships retrieval only and no "shares
  identifiers" warning.
- Known-prior samples: the 20 excluded findings with the smallest
  `sha256(seed + ':label:' + id)` per corpus, labelled before the frozen run
  by one reader; a blinded second reader on titles and the prior-item list.

**Expected outcome, stated in advance.** The claim-component requirement
should lower the false-alarm rate on both platforms and may lower duplicate
recall, because a duplicate that shares only two common identifiers no longer
matches. If the fresh corpora still exceed 30%, the trigger stays met and the
warning stays off.

## P1-C re-measurement results (2026-11-25, matcher 0.3.0)

Frozen matcher 0.3.0 at commit `f983d03`; binary
`target/release/priorart.exe` sha256
`6bfc20d381350f7b8e2f6fbe534b5ca8724268faa80ce08494772fb8530b2d5b`.
All six corpora were re-measured in full (`--split all`; the two sealed
corpora got their dev-split `freeze.json` first). Every rate is k/n with a
Wilson 95% interval; pooled rows also carry a source-clustered bootstrap
(10,000 draws, seed `priorart-bootstrap-p1c-2026-11-25`).

| Corpus | Platform | False-alarm rate | Duplicate recall@5 | Known-prior |
|---|---|---|---|---|
| Tapioca | Sherlock | 14/60 = 23.3% [14.4-35.4] | 14/64 = 21.9% [13.5-33.4] | 0/1 (count) |
| PoolTogether | Sherlock | 9/40 = 22.5% [12.3-37.5] | 16/85 = 18.8% [11.9-28.4] | 1/2 (count) |
| Maia | Code4rena | 8/15 = 53.3% [30.1-75.2] | 7/289 = 2.4% [1.2-4.9] | 0/1 (count) |
| Panoptic | Code4rena | 6/11 = 54.5% [28.0-78.7] | 3/80 = 3.8% [1.3-10.4] | 0/3 (count) |
| **Party** (sealed) | Code4rena | **3/9 = 33.3% [12.1-64.6]** | **0/132 = 0.0% [0.0-2.8]** | 0/0 (unlabelled) |
| **Wildcat** (sealed) | Code4rena | **3/9 = 33.3% [12.1-64.6]** | **9/46 = 19.6% [10.7-33.2]** | 0/0 (unlabelled) |
| Sealed pooled | Code4rena | **6/18 = 33.3% [16.3-56.3]**; bootstrap [16.3-56.3] | 9/178 = 5.1% [2.7-9.3] | — |
| Code4rena pooled | Code4rena | 20/44 = 45.5% [31.7-59.9]; bootstrap [33.3-53.8] | 19/547 = 3.5% | — |
| Sherlock pooled | Sherlock | 23/100 = 23.0% [15.8-32.1]; bootstrap [22.5-23.3] | 30/149 = 20.1% | — |
| All six pooled | both | 43/144 = 29.9% [23.0-37.8]; bootstrap [23.9-48.6] | 49/696 = 7.0% | — |

**The redesign does not work.** The claim-component requirement barely moved
the false-alarm rate on the Code4rena corpora (Maia 53.3% and Panoptic 54.5%
are unchanged to the flag; the pooled Code4rena rate falls only from 53.8% to
45.5% because the two new corpora are lower), and the sealed corpora are
33.3%, above the 30% trigger. What it did do is destroy duplicate recall:
Tapioca 56.3% -> 21.9%, PoolTogether 37.6% -> 18.8%, Maia 30.1% -> 2.4%,
Panoptic 47.5% -> 3.8%, Party 0/132, Wildcat 19.6%. The requirement that a
shared symbol be unique to one prior item, or that the two findings name the
same file, rejects almost every judge-grouped duplicate, which by construction
share common function names with several representatives.

**Decision against the preregistered trigger.** The 30% trigger is met on the
sealed pooled interval (lower bound 16.3% is below 30%, upper 56.3% above; the
point estimate is above). The warning stays off, as it has since 0.1.0. The
0.3.0 alarm rule is **not fit to ship**: it is not better on false alarms and
it loses most of the duplicate retrieval. A release that keeps the harness
fixes and the recall-preserving parts, and either reverts the alarm gate or
ships it off by default, is the operator's decision; this file records the
measurement, not a release.

## 0.2.2 measurement: the harness fixes alone (2026-11-25)

The 0.3.0 alarm rule was reverted (see above); 0.2.2 ships the benchmark
harness fixes with the matcher unchanged from 0.2.1. Re-measured on all six
corpora with binary
`b94598788be205483a3795b3097182564d2218e7a717857511e78d9865137128`
(priorart 0.2.2).

| Corpus | FAR | Duplicate recall@5 |
|---|---|---|
| Tapioca (in-sample) | 15/60 = 25.0% [15.8-37.2] | 35/64 = 54.7% [42.6-66.3] |
| PoolTogether (in-sample) | 9/40 = 22.5% [12.3-37.5] | 32/85 = 37.6% [28.1-48.3] |
| Maia (in-sample) | 8/15 = 53.3% [30.1-75.2] | 87/289 = 30.1% [25.1-35.6] |
| Panoptic (in-sample) | 6/11 = 54.5% [28.0-78.7] | 38/80 = 47.5% [36.9-58.3] |
| Party (in-sample now) | 3/9 = 33.3% [12.1-64.6] | 40/132 = 30.3% [23.1-38.6] |
| Wildcat (in-sample now) | 4/9 = 44.4% [18.9-73.3] | 23/46 = 50.0% [36.1-63.9] |
| Sealed-pair pooled | 7/18 = 38.9% [20.3-61.4] | 63/178 = 35.4% [28.7-42.7] |
| All six pooled | 45/144 = 31.2% [24.2-39.2] | 255/696 = 36.6% [33.1-40.3] |
| Code4rena pooled | 21/44 = 47.7% [33.8-62.1] | 188/547 = 34.4% |
| Sherlock pooled | 24/100 = 24.0% [16.7-33.2] | 67/149 = 45.0% |

**What the harness fixes did.** On the four in-sample corpora the numbers are
materially the same as P1 (Tapioca FAR 25.0%, recall 56.3% -> 54.7%;
PoolTogether and the two Maia/Panoptic rates unchanged). The last-slice fix
did not change a measured alarm on these corpora, and the heading/bullet
changes changed recall by at most one duplicate. The two sealed pairs, now
in-sample (their results were opened), are 33.3% and 44.4% false alarms,
7/18 = 38.9% pooled, and 63/178 duplicate recall. **The 30% trigger remains
met and the "shares identifiers" warning stays off.**

The harness fixes are still correct and worth shipping (they remove a known
way Low/QA text enters dossier A, and they make heading and wrapped-bullet
identifiers count), but they are not a precision lever. The next lever is a
different alarm rule. It needs a **new** sealed corpus pair first: the Party
and Wildcat results are now opened (they were the fresh measurement for 0.3.0
and are in-sample from this round on), so no fresh priorart evidence remains
in the workspace. A rule tuned on what is left would be measured in-sample and
could not support a precision claim.
