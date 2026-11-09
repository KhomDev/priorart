# CODE_GUIDE — a technical walkthrough of the codebase

This document is a line-level companion to [README.md](README.md): what each module
does, how the engine works, what every check in CI runs, and which number in the
docs comes from which file. Claims about counts were derived from the tree at the
commit named below, and are quoted with their source so they can be re-derived.

- Crate version: **0.5.0** ([Cargo.toml](Cargo.toml) line 3), the latest tag; crates.io
  reports 0.5.0 as the newest published version.
- Documented against main commit **08842ce56594e3fc265dc7e789a1d80b6eb5a241**
  ("chore: pin rejection-taxonomy v0.6.0"). Line references are to that tree.
- One-line purpose: an offline, deterministic, single-binary checker that scores a
  draft bug-bounty finding against 22 rejection anti-patterns and 7 pre-hunt gates
  plus the D1 known-issues gate, so a researcher can stop before writing a PoC that cannot be paid.

## Contents

1. [Workspace layout](#1-workspace-layout)
2. [The data model](#2-the-data-model)
3. [The CLI layer](#3-the-cli-layer)
4. [The matching engine](#4-the-matching-engine)
5. [The gates](#5-the-gates)
6. [The rule catalog](#6-the-rule-catalog)
7. [The gate catalog](#7-the-gate-catalog)
8. [Known issues, history, scaffolding, rendering, bench](#8-known-issues-history-scaffolding-rendering-bench)
9. [Rules, packs, examples, docs](#9-rules-packs-examples-docs)
10. [Tests](#10-tests)
11. [CI, release, provenance](#11-ci-release-provenance)
12. [Known limitations, as documented](#12-known-limitations-as-documented)

## 1 Workspace layout

A single crate (no Cargo workspace) with one library and two binaries:

| Path | Lines | Role |
|---|---:|---|
| [src/lib.rs](src/lib.rs) | 13 | Module declarations only: `bench`, `cli`, `engine`, `init`, `model`, `render` |
| [src/model.rs](src/model.rs) | 434 | Data model: rule, gate, ledger, known-issue, log, report types |
| [src/cli.rs](src/cli.rs) | 779 | Argument parsing, command dispatch, input loading, report building |
| [src/engine.rs](src/engine.rs) | 2581 | Matching engine: parsing, rule evaluation, gates, scoring |
| [src/init.rs](src/init.rs) | 682 | `vg init` templates and `vg log add` file handling |
| [src/render.rs](src/render.rs) | 332 | Human and JSON output |
| [src/bench.rs](src/bench.rs) | 287 | `vg bench`: per-rule fire and false-KILL rates |
| [src/bin/vg.rs](src/bin/vg.rs) | 5 | `vg` entry point |
| [src/bin/viability-gate.rs](src/bin/viability-gate.rs) | 8 | `viability-gate` entry point |

- Edition 2021, MSRV `rust-version = "1.89"` ([Cargo.toml](Cargo.toml) line 11).
- Both binaries run the same code: `vg` calls `cli::main_entry()`
  ([src/bin/vg.rs](src/bin/vg.rs) line 4), `viability-gate` calls
  `cli::main_entry_named("viability-gate")` ([src/bin/viability-gate.rs](src/bin/viability-gate.rs)
  line 7) so `--version` and error prefixes name the binary the user ran. Two names
  exist because `vg` collides with an unrelated bioinformatics tool
  ([docs/install.md](docs/install.md), "The `vg` name collision").
- Five direct dependencies ([Cargo.toml](Cargo.toml) lines 18-22): `clap`,
  `regex`, `serde`, `serde_json`, `serde-saphyr`. Pinned versions in
  [Cargo.lock](Cargo.lock): clap 4.6.7, regex 1.13.1, serde 1.0.229,
  serde-saphyr 2.0.0, serde_json 1.0.154. `cargo tree --edges normal` resolves
  49 packages ([docs/no-network.md](docs/no-network.md) line 38).
- Release profile is `opt-level = 3`, `lto = true`, `codegen-units = 1`,
  `strip = true` ([Cargo.toml](Cargo.toml) lines 24-28) - a deliberately
  slow build configuration: full LTO with a single codegen unit.
- Rule files are embedded with `include_str!` at [src/cli.rs](src/cli.rs) lines
  17-18, so the installed binary needs no data directory; `--rules DIR` overrides.

## 2 The data model

[src/model.rs](src/model.rs) defines everything the rest of the crate passes around.

- `CATALOG_VERSION: u32 = 1` (line 8) is the only rule/gate file format this build
  reads; [src/cli.rs](src/cli.rs) lines 270-280 reject any other `version:` value.
- `RuleFile` (line 12) and `GateFile` (line 236) are `deny_unknown_fields`, so
  a typo such as `none_off:` is a load error, not a silently ignored key
  ([tests/cli.rs](tests/cli.rs) line 445 pins this).
- `Verdict` (line 164): `Hard` = KILL, `Soft` = HOLD, `Warn` = note only.
- `Outcome` (line 193): `Kill`, `Hold`, `Note`, `Incomplete`, `Clear` -
  report-level, distinct from a rule's verdict. The doc comment (lines 183-190)
  states there is no `Submit`: the tool has no positive evidence, only the absence
  of a known blocker. `Outcome::meaning()` (lines 219-232) is the one-line gloss
  the renderer prints under the outcome word.
- `Rule` (line 100): id, title, group, verdict, weight, then matching fields
  (`any_of`, `all_of`, `none_of`, `escalate_on`, `downgrade_on`,
  `structural`), scoping fields (`none_of_scope`, `ignore_sections`,
  `negation_aware` - default true via `default_true()` at line 158), and
  provenance (`fix`, `doc`, `origin_rows`, `evidence`, `last_validated`,
  `fp_rate`). Every field is commented in place.
- `Evidence` (line 20): `in_sample` (derived from log rows) or `none` (a
  judgement, and says so). `NoneOfScope` (line 40): `document` (default) or
  `sentence`. `SectionKind` (line 60): `Poc`, `Claim`, `Recommendation`,
  `Other`. `Structural` (line 85): one variant, `P19HeadlineVsProof`.
- `Gate` (line 243): id, title, question, needs, blocks, weight, `checks`
  (each `GateCheck` is name/test/fail, line 266), evidence, origin_rows, and an
  informational `implemented_by` (D1 sets `--known-issues`).
- `LedgerEntry` (line 281) is deliberately NOT `deny_unknown_fields` (comment,
  lines 277-279): the ledger schema is owned by the sibling repository
  KhomDev/bounty-economics, whose contract is additive; an older reader ignores new
  fields. `floor_status` (line 289) is `documented`, `unstated` or
  `conflicting` - an unstated floor is "a page that has not been read", not a
  Low floor.
- `KnownIssue` (line 335): id, summary, case-insensitive regex `patterns`,
  source. `LogEntry` (line 354): family, outcome, note, date - free text, local,
  never published.
- Results: `Finding` (line 368) carries the matched `evidence` snippets and
  `matched_sentences` in parallel (comment, lines 374-376); `GateOutcome` (line
  386); `GateStatus` (line 403) is Pass/Fail/Unknown; `Report` (line 417) holds
  tool, version, subject, outcome, `viability_score`, threshold, findings,
  `passed`, gates, disclaimer, warnings.

## 3 The CLI layer

[src/cli.rs](src/cli.rs) is the only file that talks to the user.

- Constants: `EMBEDDED_ANTI_PATTERNS`/`EMBEDDED_GATES` (lines 17-18),
  `DISCLAIMER` (lines 22-26, quoted verbatim in every report), `DEFAULT_LOG =
  ".vg-history.yaml"` (line 29), `TEST_BUDGET = 4_000_000` bytes for `--tests`
  (line 32), `SKIP_DIRS` - 9 directory names the test walk never enters (lines
  35-45).
- Subcommands (lines 114-196): `check`, `gates`, `explain`, `rules`, `init`,
  `log` (`add`/`check`, lines 198-226), `bench`. `check` accepts multiple
  findings, `-` for stdin (once only, line 655), `--json` (object for one
  finding, array for several, lines 675-684), `--table`, `--min-score`
  (default 50, constrained to 0..=100, line 128) and `--fail-on-kill`.
- `TargetArgs` (lines 71-112) is shared by `check` and `gates`: `--ledger`,
  `--program`, `--severity`, `--scope`, `--prior-audits`, `--tests`,
  `--fork-of`, `--known-issues`, `--log`.
- `load_rules` (lines 251-290) parses both YAML files with serde-saphyr, checks
  `version == CATALOG_VERSION`, and compiles every rule through
  `CompiledRule::new` - an invalid regex is a load error, not a skipped rule.
- Input readers: `read_text` (line 292) handles `-` and strips a UTF-8 BOM
  (`strip_bom`, line 307 - Windows editors add one and it corrupts the first
  evidence line); `read_lines` (line 320) drops blanks and `#` comments;
  `read_ledger` (line 332); `read_known_issues` (line 342) accepts a bare list
  or a `known_issues:` mapping; `read_log` (line 360) errors on a missing path
  rather than reporting a silent unknown.
- `read_tests` (lines 379-435) walks a directory deterministically: sorted paths
  (line 412), symlinks not followed, VCS/build dirs skipped, a 4 MB budget cut on
  a UTF-8 character boundary (lines 422-425). [tests/cli.rs](tests/cli.rs) line
  468 drives a multibyte suite through it.
- `run` (lines 546-706) dispatches. `init` and `log add` run before the
  catalog is loaded (lines 548-554) so a first-time user cannot hit a catalog
  error. `build_report` (lines 498-544) evaluates rules, merges known-issue
  findings in BEFORE the outcome is computed (comment, lines 508-511 - a known
  issue is Not Applicable, not merely unpaid), sorts findings hard-first then by
  weight (lines 517-527), evaluates gates, and computes the outcome.
- Exit codes: 0 normal; 1 for a KILL under `--fail-on-kill` (line 699), a failed
  gate under `vg gates` (lines 639-645), or a probable repeat under `vg log
  check` (lines 767-770); 2 for any error returned from `run` (line 241).

## 4 The matching engine

[src/engine.rs](src/engine.rs). The module header (lines 1-24) documents the
matching semantics in four numbered points; this section shows where each is code.

### 4.1 Document parsing

- `split_sections` (lines 313-366) splits at ATX headings (`#` to `######`
  followed by a space), never inside fenced code blocks (`fence_marker`, line
  297 - a `# comment` in a shell block or `#[test]` stays part of the section).
  A new section starts AT its heading line (comment, lines 343-345) so the headline
  text belongs to the section it introduces.
- `section_kind` (lines 238-277) classifies headings by whole words: "proof of
  concept", poc/exploit/reproduction/test/steps make `Poc`;
  recommendation/mitigation/fix make `Recommendation`; impact/severity/headline/
  summary/description make `Claim`; the rest `Other`. Whole-word matching keeps
  "epoch" out of "poc" and "latest" out of "test" ([CHANGELOG.md](CHANGELOG.md),
  0.5.0 parser fixes).
- `split_sentences` (lines 386-421): a newline always ends a sentence; `.`, `!`,
  `?` end one only when followed by whitespace, a closing mark or end of text,
  and a `.` after one of the 10 `ABBREVIATIONS` (line 368) does not. So
  `Vault.sol`, `1.5`, `assert!(x >= 0)` and `e.g.` stay whole.

### 4.2 Rule evaluation

`CompiledRule` (line 41) pre-compiles `any_of`, `all_of`, `none_of`,
`escalate_on`, `downgrade_on` into `regex::Regex`. `compile_pattern`
(line 75) wraps every pattern in `(?i)` - case-insensitive unless the pattern
opts out with `(?-i:...)`.

`CompiledRule::matches` (lines 114-206), in order:

1. A `structural` rule bypasses the generic matcher entirely (lines 116-118).
2. Document-scoped `none_of` suppresses the rule on any match (lines 124-127);
   with `none_of_scope: sentence` each sentence is checked individually
   (lines 136-140).
3. `ignore_sections` skips whole sections (line 133).
4. `any_of` hits are found per sentence by `find_unnegated` (line 142) and
   dropped when a `downgrade_on` pattern matches the snippet (lines 143-145).
   `escalate_on` hits are collected separately (lines 149-153).
5. An `any_of` rule with no hits returns None (lines 157-159); `all_of` must
   hold somewhere, non-negated (lines 162-171); an all_of-only rule draws evidence
   from the all_of patterns (lines 174-184).
6. A strong hit raises the verdict to `escalate_to` (default Hard, lines
   186-191); strong hits are shown first (lines 192-193); duplicates removed by
   snippet/sentence pair (lines 194-195); the list is capped at 3 (line 199).

- Negation window: `NEGATION_CUES` (19 words, lines 32-35), `NEGATION_WINDOW =
  6` tokens (line 38). `is_negated` (lines 443-460) folds typographic
  apostrophes to ASCII (so "doesn't" negates like "doesn't"), lowercases the
  preceding tokens, and suppresses the match when a cue or an `n't` ending sits
  within the window. Rules that handle negation via `none_of` set
  `negation_aware: false` (line 154).
- `collapse` (lines 464-475) strips control characters, collapses whitespace and
  caps a snippet at 200 characters - the same discipline the renderer applies.
- `evaluate` (lines 580-603) runs every rule and splits the ids that did not fire
  into `passed`, which the renderer prints as a PASS line.

### 4.3 The structural rule (P19)

Four `LazyLock` regexes implement the SPAM-boundary rule
([rules/anti-patterns.yaml](rules/anti-patterns.yaml) line 642):
`LOSS_CLAIM` (line 479), `FAILURE_ONLY` (line 485), `POSITIVE_EVIDENCE`
(line 493), `PERSISTED_STATE` (line 500). `p19` (lines 525-575) requires: a
loss claim in a sentence before the first proof section (a match whose text
contains "value lock" is not a loss claim, lines 548-550), a proof section that
shows a revert/panic/expected failure, no positive state change in the proof
(line 561), and - if state is asserted to persist in the claim or proof - the
verdict drops to `Warn` instead of `Hard` (lines 564-569). Recommendation
sections are never read as proof (line 541).

### 4.4 Outcome and score

`outcome` (lines 618-640) is one decision rule, in this order:
KILL if any hard verdict fired; else HOLD if any soft verdict fired, any gate
failed, or the score is below the threshold; else NOTE if any warn fired; else
INCOMPLETE if any gate is Unknown; else CLEAR. `viability_score` is
`100 - sum(fired weights)` in u64 arithmetic, saturating at 0 (lines 619-620),
so extreme weights cannot overflow (engine unit test
`huge_weights_do_not_overflow`, [src/engine.rs](src/engine.rs) line 2139).
Gates do not deduct from the score - a failed gate holds the finding and
an unknown one makes the outcome INCOMPLETE (comment, lines 615-617).

"CLEAR is reachable" is pinned end to end by [tests/cli.rs](tests/cli.rs) line
183 (`clear_is_reachable_when_every_gate_runs_and_passes`): with a full input
set - ledger, scope, prior audits, tests, fork-of, log and known-issues corpus -
all eight gates run, pass, and the outcome is CLEAR. The complementary case - a
finding with no gate inputs is INCOMPLETE, never CLEAR - is pinned by
`unknown_gate_with_no_findings_is_incomplete_not_clear`
([src/engine.rs](src/engine.rs) line 2114) and stated in the README
([README.md](README.md) lines 60-63). Before 0.5.0 D1 was hard-wired to Unknown, so no
input could reach CLEAR ([CHANGELOG.md](CHANGELOG.md), 0.5.0).

### 4.5 Severity and dates

`Severity` (lines 644-686) orders Informational < Low < Medium < High < Critical,
parses abbreviations, and derives the floor a `tier_shape` implies
(`floor_of_shape`, lines 677-685). `claimed_severity` (lines 703-708) reads a
`Severity: High` line or a `| Severity | High |` table row from the finding.
Dates are strict ISO `YYYY-MM-DD` with a hand-written civil calendar
(`days_from_civil` line 712, `parse_iso_date` lines 724-746 - rejects
2026-02-29, accepts 2024-02-29); `today_days` (line 749) is UTC days since the
epoch. No date crate is used.

## 5 The gates

`GateInputs` (lines 761-777) carries every optional input; the module comment
(lines 759-760) and `evaluate_gates`'s doc (lines 839-844) state the fail-closed
rule: a gate whose input is missing reports `Unknown`, never Pass. Dispatch is a
match on the gate id (lines 849-858). With a finding, G1-G5 and D1 compare their
input against it; without one, G1-G4 check only that the input is complete and
well-formed, and their detail line says so.

| Gate | Function | What it does with a finding |
|---|---|---|
| G1 severity floor | `gate_floor` (line 881) | compares the thesis severity with the ledger floor; fails an uncited floor (no `source_url` or `retrieved`), a non-ISO or future date, a missing/`unstated`/`conflicting` floor status, a missing or `unknown` tier_shape; takes the higher of declared and implied floors; a severity below the floor fails |
| G2 exclusions | `gate_exclusions` (line 1060) | runs every compiled exclusion pattern over the finding's claim prose (`claim_prose`, line 1051 - PoC and recommendation sections are not read); a match is a Fail whose text says "a prompt to read the clause, not an automatic reject" (lines 1151-1159) |
| G3 scope tree | `gate_scope` (line 1232) | extracts path-like tokens (`extract_paths`, line 1171, URLs excluded) and traces each into the scope list by exact path, suffix, `dir/` prefix or glob (`in_scope`, line 1207; `glob_match` translates `*`/`**`/`?` to a regex, line 1188); a stem mention in prose also counts (lines 1264-1269) |
| G4 prior audits | `gate_prior_audits` (line 1299) | compares the finding with the ledger's prior-audit `known_issues` using the same containment test as G7 (lines 1354-1369), and reports which cited paths a prior audit covers |
| G5 test intent | `gate_tests` (line 1432) | scans the test suite for whole-word intent markers (`INTENT_MARKER`, line 1398 - "by design", "intended", "should preserve", ...), then ties them to the finding via code identifiers (`identifiers`, line 1413: backticked spans, calls, camelCase, PascalCase, snake_case); a marker in a test that names the finding's code fails with a `file:line` detail |
| G6 fork novelty | `gate_fork` (line 1518) | `none`/`no` in any case (or empty) passes; any other value names an upstream and fails |
| G7 own history | `gate_own_history` (line 1546) | containment similarity (`REPEAT_THRESHOLD = 0.6`, line 1660; `contained`, line 1662: at least two shared tokens, or one for a one-token label); a probable repeat fails; an empty log passes |
| D1 known issues | `gate_known_issues` (line 1602) | no corpus = Unknown; a match or an unusable entry fails; otherwise passes with the count |

`worst` (line 811) orders Pass < Unknown < Fail, so a failure outranks an
unknown in the same gate's accumulated status. G7's similarity is containment, not
Jaccard, since 0.5.0 (comment, lines 1651-1659: a 300-token finding and a 5-token
label share at most 5/300 under Jaccard). The tokeniser (lines 1684-1706)
lowercases, keeps alphanumeric tokens of 3+ characters, drops 40 stop words and a
trailing plural `s`.

`match_known_issues` (lines 1719-1785) is the corpus matcher behind D1 and
`vg check --known-issues`. A match becomes a `Finding` with verdict Hard,
weight 60 (line 1766) and rule id `KI:<id>` (line 1763). It reports warnings for
duplicate corpus ids, entries with no patterns, and invalid regexes instead of
silently passing - pinned by the engine unit tests
`known_issue_without_patterns_warns_rather_than_silently_passing` and
`known_issue_invalid_regex_and_duplicate_ids_warn` ([src/engine.rs](src/engine.rs)
lines 2559 and 2568); the D1 gate itself fails on an unusable entry
(`d1_runs_when_a_corpus_is_supplied`, line 2465).

## 6 The rule catalog

[rules/anti-patterns.yaml](rules/anti-patterns.yaml) is 653 lines of data, not
code. It carries exactly **26 rules, P1-P26** - asserted by
`catalog_has_22_rules` ([tests/rules_test.rs](tests/rules_test.rs) line 59) and
`every_rule_explains_and_the_catalog_lists` ([tests/cli.rs](tests/cli.rs) line
319, which drives `vg explain` for P1..P26 and asserts the rules list prints
"26 rules"). Header comments (lines 1-70) document the schema, the origin_rows
derivation and the case-insensitivity rule. Summing the pattern lists gives 158
individual patterns across the 22 rules; 10 rules carry `evidence: in_sample`
and 12 carry `origin_rows: 0` with `evidence: none`.

| ID | Title (as in the file) | Grp | Verdict | Wt | origin_rows | evidence |
|---|---|---|---|---:|---:|---|
| P1 | Admitted non-exploit | A | hard | 40 | 0 | none |
| P6 | Theoretical future-upgrade framing filed at Critical | A | hard | 40 | 0 | none |
| P12 | Panic or revert without persisted impact | A | soft | 30 | 5 | in_sample |
| P13 | "Inconsistent state" without a quantified fund-flow delta | A | soft | 25 | 0 | none |
| P2 | Privileged-role-gated against a program exclusion | B | hard | 40 | 3 | in_sample |
| P3 | Front-run-only or operator-delay dependency | B | hard | 35 | 0 | none |
| P5 | Out-of-scope file primitive | B | hard | 35 | 4 | in_sample |
| P11 | Testnet-only finding at Critical tier | B | hard | 35 | 0 | none |
| P20 | Requires a cooperating party | B | soft | 30 | 1 | in_sample |
| P7 | Best-practice or convention critique | C | soft | 30 | 1 | in_sample |
| P16 | Severity mismatch with the program payout floor | C | hard | 40 | 2 | in_sample |
| P18 | Known characteristic documented in a prior audit | C | hard | 40 | 1 | in_sample |
| P21 | Realistic-configuration mismatch | C | soft | 30 | 0 | none |
| P22 | Primitive-vs-product confusion | C | soft | 25 | 0 | none |
| P15 | Intended behaviour asserted in the project's own test suite | D | warn | 40 | 1 | in_sample |
| P4 | Defective PoC | E | warn | 40 | 0 | none |
| P9 | Wrong line numbers or unverified paths | E | hard | 35 | 0 | none |
| P8 | Self-contradicting severity or impact sections | E | soft | 30 | 0 | none |
| P10 | Hypothetical TVL-scaling tables | E | warn | 10 | 0 | none |
| P14 | Triager-simulation table embedded in the report | E | hard | 35 | 0 | none |
| P17 | No attacker-win scenario | E | soft | 30 | 14 | in_sample |
| P19 | Headline impact contradicted by PoC evidence (SPAM line) | E | hard | 50 | 1 | in_sample |

- Groups: A "the impact doesn't exist" 4 rules; B "the precondition is
  unreachable or disallowed" 5; C "the harm is bounded or unprofitable" 5;
  D "already known" 1; E "the bug is fine; the report fails" 7.
- Verdicts: 11 hard, 8 soft, 3 warn (P4, P10, P15). P15 and P4 are warn rules
  that escalate to hard on strong forms (`escalate_on`/`escalate_to` at
  [rules/anti-patterns.yaml](rules/anti-patterns.yaml) lines 466-475 and
  504-509); P19 is the one `structural` rule (line 642) and carries the heaviest
  weight, 50.
- Rule-specific mechanics worth reading in the file: P2's sentence-scoped
  `none_of` (line 205) with prose forms and user-held-owner suppressions
  (lines 191-215); P3's slippage/deadline escape hatches (lines 238-248); P5's
  `ignore_sections: [poc]` (line 271); P13's case-sensitive ticker suppression
  `(?-i:[A-Z]{2,6})` (line 171); P15's contradiction suppressions with
  escalation (lines 453-475); P18's missed/did-not-cover suppressions (lines
  375-383); P22's deployment-evidence suppressions (lines 426-432).
- `origin_rows` derivation: each value is a count over the public
  rejection-taxonomy dataset (113 rows) at the tag pinned in
  [scripts/check_readme.py](scripts/check_readme.py), using the rule's own token
  in a published column; the mapping is the `PROVENANCE` dict
  ([scripts/check_readme.py](scripts/check_readme.py) lines 64-82). A rule with
  no public token carries 0 and `evidence: none` - "the refusal to publish a
  number that cannot be reproduced" (header, lines 63-66).
- Every rule sets `fp_rate: null`: there is no accepted/paid control set, and
  `rule_provenance_is_declared_and_fp_rate_is_unmeasured`
  ([tests/rules_test.rs](tests/rules_test.rs) line 286) asserts no rule may claim
  one.

## 7 The gate catalog

[rules/gates.yaml](rules/gates.yaml) is 286 lines declaring **8 gates: G1-G7 plus
D1** - asserted by `gates_are_eight_and_well_formed`
([tests/rules_test.rs](tests/rules_test.rs) line 186), which also requires every
gate to declare at least one named check (25 checks in total across the eight
gates) and D1 to set `implemented_by: "--known-issues"` (line 203). The header
(lines 32-47) pins the shared gate vocabulary with the rejection-taxonomy
repository, so no id may mean two things across the two repositories.

Weights: G1 40, G2 35, G3 30, G4 30, G5 25, G6 20, G7 30, D1 0. These are shown
by `vg gates --verbose` and never subtracted from the score (header, lines
15-17). `origin_rows`: G1 5, G2 20, G3 4, G4 7, G5 1, G6 0, G7 12, D1 41.
G6 declares `evidence: none` with an inline note (lines 211-213) - no rejection
in the author's log is attributed to fork provenance; `gate_evidence_is_declared`
([tests/rules_test.rs](tests/rules_test.rs) line 231) pins that honesty. D1's 41
rows are "a count of rejections, NOT a measure of this matcher" (comment, lines
270-273), and the D1 entry carries the honest status note (lines 257-259): recall
was 11-14% at 40-50% cross-fire in the author's test, and it cannot catch a
concurrent duplicate.

## 8 Known issues, history, scaffolding, rendering, bench

- **Known-issues matching** is engine-level (`match_known_issues`,
  [src/engine.rs](src/engine.rs) line 1719) and reachable from both `vg check
  --known-issues` (merged into findings before the outcome, [src/cli.rs](src/cli.rs)
  lines 508-516) and gate D1. The module doc (lines 1710-1718) repeats the
  structural limit: it is a known-issue matcher, not a duplicate catcher.
- **Own submission history** (`vg log`): [src/init.rs](src/init.rs) implements
  the file handling. `log_add` (lines 242-309) validates `--family` non-empty,
  `--outcome` against the 6 allowed values (line 226), `--date` as a real ISO
  calendar date, always double-quotes YAML scalars (`yaml_quote`, lines 370-386
  - a bare scalar like `null` would silently change type), appends to an
  existing file without dropping its comments (`append_entry`, line 325), and
  writes through a temporary file plus rename (lines 303-308) so a failure never
  leaves a half-written log. `parse_log` (line 353) accepts a bare list or a
  `submissions:` mapping. `today_iso` (line 389) is a hand-written
  civil-from-days inverse; no date library is used.
- **`vg init`** (lines 181-223) writes five commented templates - `ledger.yaml`,
  `known-issues.yaml`, `scope.txt`, `history.yaml`, `NOTES.md`
  (`FILES`, lines 173-179) - and never overwrites without `--force`. The
  templates are structure only; unit tests (lines 436-471) assert they name no
  target, and the closing print (lines 205-221) tells the user the scaffold is
  intended to fail validation until filled in.
- **Rendering** ([src/render.rs](src/render.rs)): `text` (line 7) prints the
  outcome word and meaning, the score with its caveat, each finding with marker
  (HARD/SOFT/NOTE), matched snippet and sentence, wrapped fix, doc page and
  provenance, the PASS line, SKIP line for unknown gates, the gate marks
  (`G1 ?`), and the disclaimer under a 60-dash rule (`RULE`, line 5).
  `table` (line 104) prints one row per fired rule.
  `gates_text` (line 133) prints each gate with question, needs, detail and
  evidence; `--verbose` adds blocks, weight and every check.
  `sanitize` (line 127) drops control characters so finding text cannot drive
  the terminal ([tests/cli.rs](tests/cli.rs) line 483). `rules_list` (line 207)
  and `explain` (line 233) print the catalog with group headers and, per rule,
  its verdict, weight, evidence, fp_rate (or "UNMEASURED"), detection patterns
  with their scopes, negation window state, fix and doc link.
- **`vg bench`** ([src/bench.rs](src/bench.rs)): `run` (line 107) evaluates
  every rule over the rejected and accepted sets, per rule counting fires and,
  on the accepted set, hard fires - the false-KILL count. Rates over an empty set
  are `None` and print `n/a`, never `0.0%` (`rate`, line 53). `--labels`
  reassigns files between sets by filename (lines 119-165) and the report notes
  which files moved. The module header (lines 1-18) states the honest position:
  no number here is a validation; the false-KILL rate on an accepted set is the
  number that has never been measurable, because the author's log contains no
  paid findings.

## 9 Rules, packs, examples, docs

- [rules/packs/](rules/packs/) holds **8 ecosystem packs** (EVM/Solidity, Vyper,
  Move, Rust/Solana, Cairo, Cosmos/Go, TON, Polkadot/Substrate), each declaring
  `tokens`, `path_globs`, `test_markers` and `notes`. Each pack totals
  exactly 16 entries (tokens + globs + markers; computed from the shipped files),
  which satisfies both tripwire bounds: at least 7 packs, largest at most 2x the
  smallest ([scripts/check_tripwires.py](scripts/check_tripwires.py) lines 49-50).
  Nothing consumes packs at runtime: they exist so the shipped catalog carries no
  ecosystem token, making a rule's presence reveal nothing about which ecosystems
  the author works on ([rules/packs/README.md](rules/packs/README.md)).
- [examples/](examples/) - what ships and what consumes it:
  `examples/readme/pause-panic.md` is the input the README example block is
  generated from by [scripts/regen-readme-examples.sh](scripts/regen-readme-examples.sh);
  `examples/bench/` (4 rejected + 4 accepted findings plus a labels.csv) is the
  `vg bench` fixture the CI calibration job runs;
  `examples/calibration/` (11 findings, labels.csv, expected.json) is the
  synthetic calibration fixture `tools/calibrate/calibrate.py` scores;
  `examples/known-issues.example.yaml` is a fully fictional corpus showing the
  input format.
- [docs/](docs/) is the caveat apparatus: [docs/calibration.md](docs/calibration.md)
  (the in-sample measurement and its dated note), [docs/no-network.md](docs/no-network.md)
  (the offline claim and its limits), [docs/control-set.md](docs/control-set.md)
  (status PENDING - how a false-positive measurement would be built),
  [docs/prospective-protocol.md](docs/prospective-protocol.md) (status NOT RUN),
  [docs/bench.md](docs/bench.md), [docs/install.md](docs/install.md),
  [docs/licensing.md](docs/licensing.md), [docs/tripwires.md](docs/tripwires.md),
  [docs/rule-defect-report.md](docs/rule-defect-report.md),
  [docs/gate-mapping.yaml](docs/gate-mapping.yaml), and
  [docs/rules/](docs/rules/) - 23 generated pages (P1..P22 plus the index
  README.md) produced by [scripts/gen_rule_docs.py](scripts/gen_rule_docs.py)
  from the catalog plus [docs/rules/_annotations.yaml](docs/rules/_annotations.yaml),
  which holds the judgement half (intent, escape hatches, false-positive modes,
  examples, changelog) per rule. `gen_rule_docs.py --check` fails on drift.
- Calibration numbers, quoted as the repo states them (do not re-derive these):
  the study matched **59 of 97** logged rejections at 0.50+ confidence and scored
  them: **Strict 14/59 (23.7%)**, **Lenient 45/59 (76.3%)**, miss 14/59; it fired
  on **48 of 59 (81.4%)**. The class split is 26 reachable-gate pairs at 53.8%,
  28 corpus-only pairs at 0%, 5 mixed/unmapped at 0%. The tested fix for the
  corpus-only class - the known-issues matcher - failed: one-term patterns
  13/28 (46.4%) recall with 43/56 (76.8%) cross-fires; two-term proximity
  3-4/28 (10.7-14.3%) at 40-50%. Mean vocabulary overlap Jaccard 0.024,
  containment 0.61. All of these describe the 0.2.0-era catalog on the original
  97-row log; **the recall of the current catalog is unmeasured**
  ([docs/calibration.md](docs/calibration.md) dated note, lines 3-18). The
  synthetic fixture's pinned numbers are strict 8/10, lenient 9/10, miss 1/10
  ([examples/calibration/README.md](examples/calibration/README.md)) - a smoke
  test of the harness, not a result.
- Cross-repo contract with KhomDev/rejection-taxonomy: the README's "Related"
  table says the catalog reaches **72 of the 113 rows (63.7%)** measured against
  the `earliest_gate` column, with the bucket rows G2_exclusion_clause 20,
  design_review 13, G4_prior_audit 7, P17_no_attacker_win 14, G1_severity_floor 5,
  G3_scope_tree 4, REALISM 3, P16_below_floor 2, G5_test_intent 1,
  program_ceiling 1, P19_headline_mismatch 1, P20_cooperating_party 1, and
  reachable total 72; `D1_duplicate_check` is 41 rows and not reachable.
  [scripts/check_readme.py](scripts/check_readme.py) recomputes every
  `origin_rows`, every automation-table row and the reachable total from the
  dataset at the pinned tag and fails CI on drift. The tag is pinned in ONE
  place, `PINNED_TAXONOMY_TAG` (line 60, currently "v0.6.0"); the dataset is
  read from `$VG_TAXONOMY_CSV` or `../rejection-taxonomy/data/rejections.csv`
  (`dataset_path`, lines 137-141).

## 10 Tests

Three layers, all run by `cargo test --locked --release`:

1. **Unit tests** live in the modules (`#[cfg(test)] mod tests`):
   54 in [src/engine.rs](src/engine.rs) (line 1787 onward), 16 in
   [src/init.rs](src/init.rs) (line 409), 4 in [src/render.rs](src/render.rs)
   (line 304). They pin the matcher semantics (negation window, sentence-scoped
   none_of, escalation/downgrade, all_of, section classification, sentence
   splitting, P19's four regexes, the outcome order, every gate's fail-closed
   behaviour, strict ISO dates, known-issue matching) and the log file handling
   (hostile labels round-trip, comments survive appends, validation).
2. **Fixture tests** ([tests/rules_test.rs](tests/rules_test.rs), 9 tests) drive
   the REAL engine over [tests/rules.fixture.yaml](tests/rules.fixture.yaml):
   34 cases covering all 22 rules, each with a positive `fires` text and a
   negative `suppressed` text, plus **21 regression cases** holding the exact
   counterexample text of reproduced defects (5 of them expected to fire, the
   rest to stay silent; the test requires at least 8, line 175). The suite
   asserts the exact counts: 22 rules, 8 gates (G1-G7 plus D1, D1 carrying
   `implemented_by: --known-issues`), no rule or gate naming a target or a
   private path, and no rule claiming an fp_rate.
3. **End-to-end tests** ([tests/cli.rs](tests/cli.rs), 18 tests, 493 lines) drive
   the real `vg` and `viability-gate` binaries via `CARGO_BIN_EXE_vg` /
   `CARGO_BIN_EXE_viability-gate` (lines 13-14), covering: audit
   counterexamples that must not be killed (line 74), CLEAR reachable (line 183),
   a failed gate holding the outcome (line 200), D1 under `vg check` (line 220),
   multiple findings and `--fail-on-kill` exit code 1 (line 255), `init` and
   the scaffold's intended validation failure (line 286), all 22 explain pages
   (line 316), bench labels (line 328), the second binary's name (line 365), the
   log add/check repeat flow (line 376), input errors as exit code 2 (lines 408,
   430), a rule file with a typo rejected (line 445), a multibyte test suite
   (line 468), and control characters never reaching the terminal (line 483).

Total: **101 tests** (74 unit + 18 end-to-end + 9 fixture), as reported by
`cargo test --locked --release` on this machine. The README's Testing section
([README.md](README.md) lines 482-491) lists the full local pre-PR suite.

## 11 CI, release, provenance

[.github/workflows/ci.yml](.github/workflows/ci.yml) defines 9 jobs / 11 check
contexts. `RUST_TOOLCHAIN: "1.98.0"` is pinned (line 16) so a new compiler's
lints cannot turn main red overnight; `RUSTFLAGS: -D warnings` (line 13) and a
read-only token (lines 8-9).

| Job (context name) | OS matrix | What it runs |
|---|---|---|
| test (test (ubuntu/macos/windows)) | 3 | fmt --check, clippy --locked --all-targets --all-features -D warnings, test --locked --release, build --release, both binary names produced, smoke: `vg --version`, `vg rules`, `vg explain P18` |
| msrv | ubuntu | reads `rust-version` from Cargo.toml, installs that toolchain, `build --locked --release` and `test --locked --release` on it |
| cargo-deny | ubuntu | `cargo deny check --all-features` via the pinned action; [deny.toml](deny.toml) bans the network crates, duplicate versions, wildcard deps, unknown registries, and allows only a fixed licence set |
| cargo-audit | ubuntu | installs cargo-audit, runs `cargo audit` |
| no network code | ubuntu | three steps: the resolved dependency graph carries none of the denylisted network crates (cargo tree + grep, and a Cargo.toml grep); no socket or request code in src/ (grep for TcpStream, UdpSocket, std::net, the HTTP clients); the release binary mentions no HTTP client name (grep of the built vg) |
| tripwires | ubuntu | `python scripts/check_tripwires.py` and `--self-test` (11 boundary fixtures) |
| generated docs are current | ubuntu | reads the pinned taxonomy tag, checks out KhomDev/rejection-taxonomy at it into `../rejection-taxonomy`, `gen_rule_docs.py --check`, `check_readme.py --require-dataset`, `regen-readme-examples.sh --check` against a release build |
| calibration fixture | ubuntu | builds release, calibrate.py `--dry-run`, calibrate.py scored against `--expect examples/calibration/expected.json` (fails if a rule change moves strict/lenient/miss/per_gate), and `vg bench` over the bench fixture |
| shipped rules name no target | ubuntu | greps rules/ for platform and forge-host names; only this project's own repositories are allowed as provenance |

**Release** ([.github/workflows/release.yml](.github/workflows/release.yml)),
triggered by a `v*` tag: a `verify` job refuses a tag that does not match the
Cargo.toml version and re-runs fmt/clippy/test; a `build` job produces five
targets (x86_64/aarch64 linux, x86_64/aarch64 macOS, x86_64 windows) with
SHA-256 checksums and build-provenance attestations; a `release` job merges
the checksums, attests them, and publishes the GitHub release with both binaries.
Nothing in the workflow publishes to crates.io - the header (lines 5-7) calls
that a deliberate stop gate, because the crate name is claimed permanently and
publishing is a separate, manual decision. Per [CHANGELOG.md](CHANGELOG.md), the
crate has been on crates.io from 0.3.0 onward (the 0.3.0 entry), and the
published versions are 0.3.0, 0.4.0, 0.4.4, 0.5.0 (crates.io confirms 0.5.0 as
the newest).

**Where each enforced number lives**: the 22/8 counts in the test suite; the
origin_rows and the 72-of-113 reachable total in
[scripts/check_readme.py](scripts/check_readme.py) (recomputed from the pinned
dataset); the generated pages in [scripts/gen_rule_docs.py](scripts/gen_rule_docs.py);
the example output block in [scripts/regen-readme-examples.sh](scripts/regen-readme-examples.sh);
the strict/lenient fractions cross-checked between README and
[docs/calibration.md](docs/calibration.md) (lines 189-195 of the checker); the
calibration fixture's counts in [examples/calibration/expected.json](examples/calibration/expected.json);
the no-target discipline in the test suite, the tripwires and the
generic-rules job. [docs/rule-defect-report.md](docs/rule-defect-report.md)
records the 2026-11-06 audit: nine reported rule defects, all nine reproduced
and fixed, each with a permanent regression fixture.

## 12 Known limitations, as documented

The README's "Known limitations" section lists nine, quoted here in compressed
form with sources: (1) heuristic text matching - every rule is a regex over
prose; (2) it can be gamed by wording, in both directions; (3) in-sample, and of
an older catalog - the measurement predates the 0.3.0/0.5.0 rule changes; (4)
the false-positive rate is unmeasured - the log has no paid findings, fp_rate
stays null; (5) G6 has no supporting data; (6) origin_rows is provenance, not
validation - ten of the twenty-two rules have a public token, twelve carry 0;
(7) the gate mapping is a judgement and the log's gate column is post-hoc and
self-assessed; (8) a HOLD is advice the researcher still has to act on; (9) the
gates are heuristics too - a pass is the absence of a match, not a proof
([README.md](README.md) lines 431-460). [docs/rule-defect-report.md](docs/rule-defect-report.md)
adds: the negation window is a heuristic in both directions (a distant cue does
not suppress, a same-clause cue may over-suppress), and both behaviours are
asserted by tests because they are known and intentional. [docs/no-network.md](docs/no-network.md)
states what its verification does not prove: the absence of a network
dependency, not of a network call, and only for the recorded commit.
