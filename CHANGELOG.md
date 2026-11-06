# Changelog

Notable changes to `viability-gate`. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Dates are the commit dates. Entries are written after the change, and a number
appears here only if it came from a command that can be re-run.

## [Unreleased]

## [0.5.0] - 2026-11-06

The audit release. A 2026-11-06 audit ran the 0.4.4 binary against crafted findings and found
that the outcome rule could never reach `CLEAR`, that four gates never looked at the finding, and
that several rules hard-killed standard valid findings. Every counterexample it reproduced is now a
regression fixture (`tests/rules.fixture.yaml`) or an end-to-end test (`tests/cli.rs`).

**Outcomes change in this release.** Expect different results from 0.4.x on the same input.

### Changed - outcomes and gates

- **`CLEAR` is reachable.** D1 was hard-wired to `?`, so no input could produce `CLEAR`. D1 now
  runs from `--known-issues`: unknown without a corpus, fail on a match or an unusable entry,
  pass otherwise. `vg gates` accepts `--known-issues` too.
- **A failed gate holds the finding.** It used to have no effect on the outcome, while an
  unknown gate made it `INCOMPLETE`, so a failure counted for less than missing input. The
  decision rule is now KILL → HOLD (soft rule, failed gate, or score below threshold) → NOTE →
  INCOMPLETE → CLEAR.
- **G1-G5 compare their input with the finding** (in `vg check`, or `vg gates --finding`):
  - G1 compares the thesis severity (`--severity`, or a `Severity:` line) with the floor. A
    narrow floor with no stated severity reports unknown instead of failing. A missing
    `floor_status`, an invalid date and a future `retrieved` date now fail, as gates.yaml said.
  - G2 runs each exclusion `pattern` over the finding's claim sections. It used to check only
    that the clauses were well-formed.
  - G3 traces the paths and file names the finding cites into the scope list (exact path, path
    suffix, `dir/` prefix or glob). It used to pass whenever the scope list was non-empty.
  - G4 compares the finding with the ledger's prior-audit `known_issues` and reports which cited
    paths a prior audit covers.
  - G5 looks for whole-word intent markers only in tests that name an identifier from the
    finding, and reports `file:line`. "unintended" no longer matches "intended".

  Without a finding, G1-G4 still check only that their input is complete, and say so.
- **G7 uses containment instead of Jaccard.** Jaccard between a whole finding and a short family
  label can never reach the old 0.18 threshold, so the README's own example family went
  undetected on a realistic draft. A draft is now a probable repeat when it holds at least 60% of
  a family label's words, with at least two of them in common. An empty log passes.
- **`--program <id>`** selects one entry of a multi-program ledger. Without it, a ledger with
  several programs makes G1, G2 and G4 unknown instead of pooling every program.
- G6 accepts `none` / `no` in any case.

### Fixed - rules (each a reproduced false KILL or HOLD, or a dead rule)

- **P2** hard-killed "missing the onlyOwner modifier" - a missing-access-control finding.
  `missing` is now a negation cue, and a user-held token owner or approved operator suppresses
  the rule.
- **P3** hard-killed missing-slippage findings ("sandwich"). Slippage, minimum-amount-out and
  deadline wording now suppress it.
- **P12** matched "read-only reentrancy". "read-only" must now name a function, call or view.
- **P13** was effectively dead: under the catalog's case-insensitive flag its ticker
  suppression matched any number followed by a short word ("2 of"). The ticker is now
  `(?-i:...)`, and `check_tripwires.py` check E fails on any `[A-Z]` class outside one.
- **P15** hard-killed "breaks / contrary to the intended behaviour", the finding's own
  argument. The note mode the docs described now exists: a bare "documented behaviour" is a
  NOTE, an intent phrase escalates to hard.
- **P17** no longer holds a finding that names a permanent freeze of funds, a paid impact class.
- **P19** treated "total value locked" as a loss claim, read every section after the PoC
  (including the Recommendation) as proof, let one "storage" anywhere downgrade it, and read
  "epoch" or "latest" in a heading as a PoC section. It now needs a failure in a proof section
  and no positive state change there.
- **P21** held findings for saying "requires no misconfiguration" or "under the deployed
  configuration" - what its own fix asks for. Both patterns are removed.
- **P1** documentation claimed a bare hedge fires as a note; it never did. The docs are
  corrected and the no-op escalation list removed.
- **P4**: `Stub*` was listed and then dropped by the downgrade list, a dead alternative; removed.

### Fixed - parser and engine

- Lines inside fenced code blocks no longer start a section, and a heading needs `#` plus a
  space, so `# comment` in a shell block, `#[test]` and `#hashtag` are not headings.
- Headings are classified by whole word (`epoch` is not `poc`; `latest`, `contest` and
  `attestation` are not `test`). Recommendation and mitigation sections are their own kind.
- A '.', '!' or '?' ends a sentence only before whitespace or the end of the text, so
  `Vault.sol`, `assert!(x >= 0)` and `e.g.` stay whole. P4's `assert!(...)` pattern could never
  match before.
- Typographic apostrophes are folded before negation, so "doesn’t front-run" is negated.
- A rule's strong (`escalate_on`) hits are shown first and respect `ignore_sections` and
  sentence-scoped `none_of`.

### Fixed - command line and inputs

- `vg check` accepts several findings (the README's `vg check findings/*.md` example used to be
  a usage error). JSON is an object for one finding and an array for several; `--fail-on-kill`
  fails on any KILL. `--min-score` is limited to 0-100.
- `vg init` printed, and NOTES.md suggested, `vg gates --known-issues`, which did not exist.
- `--tests` crashed (`cli.rs:361`) when the 4 MB budget fell inside a multi-byte character. It now
  reads files in sorted order, skips VCS and build directories, does not follow symbolic links,
  and stops the whole walk at the budget.
- `vg log add --family null` (or `true`, `~`, `1e3`, ...) wrote an unquoted scalar that broke the
  log for every later read. Every value is now double-quoted; `--outcome` and `--date` are
  validated; an existing log is appended to, keeping its comments; the write goes through a
  temporary file and a rename.
- A `--log` path that does not exist is an error, not a silent `?`. `--program` must name a
  ledger entry, and `--severity` must be a severity.
- `vg log check` honours `--rules`, and every error is prefixed with the binary's own name.
- Rule, gate, known-issue and log files reject unknown keys, so a typo such as `none_off:` is a
  load error instead of a silently over-firing rule. `none_of_scope`, `ignore_sections` and
  `structural` are closed sets. The catalog `version` is checked. The ledger stays lenient,
  because its schema is owned by bounty-economics and new fields are additive.
- The deprecated `doctrine` alias for `doc` is removed (announced for one minor version in
  0.2.0). The unused `run_by_gates_command` gate field is removed.
- Text from a finding, corpus or log is stripped of control characters before it is printed, so
  it cannot drive the terminal.
- `vg bench --labels` now decides which set each file belongs to (it used to only count rows),
  and a rate over an empty set prints `n/a` instead of `0.0%`.
- Scores sum in 64 bits, so an extreme weight cannot wrap.

### Fixed - documentation and provenance

- `origin_rows` re-derived against rejection-taxonomy v0.5.1 (113 rows): P17 6 → 14, D1 34 → 41.
  `scripts/check_readme.py` now recomputes every rule's and gate's `origin_rows` from the dataset,
  checks the README catalog titles against the catalog, and pins the taxonomy tag
  (`PINNED_TAXONOMY_TAG`) instead of following its latest release.
- The README's "not 69.0%" sentence is removed: 72 reachable rows plus 41 D1 rows is all 113, so
  the figure it rebutted could not have counted D1.
- README and `docs/calibration.md` say which catalog the recall study measured (the 0.2.0-era
  catalog) and that the current catalog's recall is unmeasured.
- README catalog titles match the catalog; the rule example is the real P17.

### Security and CI

- `release.yml` grants write permissions per job instead of to every job, and pins third-party
  actions to commit SHAs. The merged `SHA256SUMS` is attested too.
- CI pins the Rust toolchain, runs tests with `--locked`, and runs `check_readme.py
  --require-dataset` against the pinned taxonomy tag.
- `.gitattributes` started with a UTF-8 byte-order mark, so its first line (`* text=auto
  eol=lf`) matched no file and Windows checkouts got CRLF, which broke the README example
  check there. The mark is removed.

## [0.4.4] - 2026-11-05

A maintenance release. The catalog, the scoring and the CLI output are unchanged.

### Fixed

- `docs/calibration.md`'s dated note still said the dataset had grown to 111 rows
  (rejection-taxonomy v0.4.0). It now says 113 rows (v0.5.0), 16 rows added since
  the 97-row study. The recall figures themselves are unchanged and still refer to
  the original 97 rows.

### Changed

- `serde_json` 1.0.151 → 1.0.154 (supersedes Dependabot PR #4).
- CI runs with a read-only `GITHUB_TOKEN` (`permissions: contents: read`).
- README "Related" links the new sibling repository,
  [realism-gate](https://github.com/KhomDev/realism-gate).

## [0.4.3] - 2026-10-28

### Changed

- **CI now enforces the reachable-automation total against the published
  taxonomy.** `scripts/check_readme.py` only checks the "catalog reaches N of M"
  number when the sibling dataset is on disk at `../rejection-taxonomy`; in CI it
  was never cloned, so the block was silently skipped and the number was only
  locally true. The `generated-docs` job now resolves the latest
  `KhomDev/rejection-taxonomy` tag, checks it out, and places it where
  `check_readme.py` expects — so the total is CI-enforced, not just asserted.
- **Docs: the `Related` / reachable-automation numbers now track
  [rejection-taxonomy](https://github.com/KhomDev/rejection-taxonomy) v0.5.0 (113
  rows).** The published dataset grew from 111 to 113 rows (both additions are
  `P17_no_attacker_win`), so the README automation table is recomputed:
  `P17_no_attacker_win` 12 → 14, reachable total **70 → 72**, and the headline
  becomes **the catalog automates 72 of 113 rejections (63.7%)**.
  `D1_duplicate_check` stays 41 and unreachable.

### Unchanged (reviewed, no change)

- **No 23rd anti-pattern.** The two new rejections were reviewed against the
  catalog. Both are permanent-liveness / DoS wedges with no attacker-win and map
  cleanly onto the existing **P17 (No attacker-win scenario)**; no genuinely new,
  prose-detectable class emerged. The catalog stays at exactly 22.
- **The 59/97 recall study is not re-run here.** It remains the original
  in-sample measurement; a calibration re-run against the current dataset is a
  separate, dedicated pass.

## [0.4.2] - 2026-10-21

### Changed

- **Docs: the `Related` / reachable-automation numbers now track
  [rejection-taxonomy](https://github.com/KhomDev/rejection-taxonomy) v0.4.0 (111 rows).** The
  published dataset grew from 97 to 111 rows, so the README automation table is recomputed from
  the new `earliest_gate` split: `design_review` 12 → 13, `P17_no_attacker_win` 6 → 12,
  `D1_duplicate_check` 34 → 41 (still unreachable), reachable total **63 → 70**, and the headline
  becomes "70 of 111 rejections (63.1%), not 70.3%". Dataset-size references elsewhere in the docs
  move to 111. The in-sample recall study (59/97 matched, 14/59 strict, 45/59 lenient) is **not**
  re-run and is left exactly as measured against the original 97 rows; a dated note in `README.md`
  and `docs/calibration.md` records that split. A docs-accuracy release — no code, rules or
  behaviour changed.

## [0.4.1] - 2026-10-20

### Fixed

- **`LICENSE` is now the plain MIT text only.** It previously carried an extra
  "No bundled data" section after the MIT body, which made GitHub's license
  detector classify the repository as "Other" instead of MIT. The MIT text is
  unchanged (byte-standard SPDX template); the "No bundled data" note moved to
  the License section of `README.md`, where it reads the same. No code, rules,
  dependencies or behaviour changed.

## [0.4.0] - 2026-10-13

### Changed

- **The YAML parser is now `serde-saphyr`** (pure Rust), replacing `serde_yaml`
  0.9 and its `unsafe-libyaml`. Both are unmaintained (RUSTSEC-2024-0449), so
  the `deny.toml` exception added in 0.3.0 is removed. The parse result is
  unchanged. I checked this by diffing every output of the 0.3.0 binary against
  0.4.0: `rules`, all 22 `explain` pages, `check --json` on every example
  finding, a `check` and a `gates --verbose` run with a populated ledger,
  known-issues corpus and history log, `bench`, and `init`. Apart from the
  version string, they are identical.
- **Breaking for source builds: the MSRV is now Rust 1.89** (was 1.85), because
  `serde-saphyr` declares 1.89. Release binaries are unaffected.
- **Malformed-YAML errors read differently.** They now come from the new
  parser, with the line, the column and a caret pointing at the problem. Any
  script that matched the old error text needs updating.
- The dependency graph grows from 39 to 49 packages. None of them is a network
  client (`docs/no-network.md`, re-verified).

### Fixed

- **`viability-gate --version` printed `vg 0.3.0`.** Each binary now reports
  its own name in `--version`, `--help` and error prefixes. The library gains
  `cli::main_entry_named`; `cli::main_entry` still exists and behaves as `vg`.

## [0.3.0] - 2026-10-13

The first tagged release in this repository, and the first on crates.io. It
contains the changes listed here, the history-reset fixes below, and everything
under "Changes since 0.2.0" further down.

### Removed

- **The deprecated JSON fields `odds` and `verdict`, and the `deprecated_fields`
  list that named them.** 0.2.0 announced their removal in 0.3.0. Use
  `viability_score` and `outcome`, which carry the same values.

### Fixed

- **Twenty code-fence lines in `README.md` were `` `@ `` instead of three
  backticks**, so ten code blocks rendered as plain text on GitHub. The defect
  came in with a shell rewrite and shipped in the initial public release.
- **The README told readers to pin `v0.2.0`, and the changelog linked
  `v0.1.0` and `v0.2.0`.** None of those tags exist in this repository, because
  the history was reset. The pin is now `v0.3.0`; the old versions are listed
  without links.

### Changed

- **Licence scope stated.** The README's License section now says that the MIT
  grant covers the whole repository, including the catalog in `rules/`, and
  `docs/licensing.md` records the question as resolved.
- **`docs/install.md` and the README** list `cargo install viability-gate` as the
  first install method.
- **`deny.toml` ignores RUSTSEC-2024-0449** (`unsafe-libyaml` unmaintained,
  pulled in by `serde_yaml` 0.9), with the reason recorded inline. The advisory
  is about maintenance, not a known vulnerability. Replacing `serde_yaml` is
  the next minor version's work, because a parser swap can change how the
  catalog parses.

### Added

- **`scripts/check_readme.py` checks code fences and the pinned tag.** It fails on
  a `` `@ `` fence line, on an odd number of fences, and when the tag the README
  pins differs from the version in `Cargo.toml`.

## Repository history reset - 2026-10-10

### Fixed

- **The declared MSRV was false.** `Cargo.toml` declared `rust-version = "1.74"`,
  but the dependency graph includes crates published under **edition 2024**, which
  Cargo 1.74 cannot parse, and `Cargo.lock` is lockfile **version 4**, which Cargo
  before 1.83 cannot read. The MSRV job therefore failed on every push. Verified
  directly: `cargo +1.74 build --release` fails with `this version of Cargo is older
  than the 2024 edition`; `cargo +1.85 build --release` and `cargo +1.85 test
  --release` both succeed against the committed lockfile. The declared MSRV is now
  1.85 and `docs/install.md` says so.

Repository history was reset on 2026-10-10 to remove identifying material from
earlier commits; earlier versions are no longer available.

The tree itself is unchanged except for the two fixes recorded here.

### Fixed

- **Five GitHub Actions expressions in `.github/workflows/ci.yml` were malformed**
  (a doubled brace in `runs-on`, the two MSRV toolchain references, and a
  `${{deny}` reference to a shell variable). They would have failed workflow
  parsing, so the job that was supposed to prove the repository clean could never
  have run. All five are corrected.
- **The token matcher now has fixtures.** `python scripts/check_tripwires.py --self-test`
  pins both directions: ordinary English that merely contains a token (suite,
  nearest, button) must pass, and the token itself must fail. CI runs it in the
  tripwires job, so a regression in the boundary rule cannot land silently.

## Changes since 0.2.0 - released in 0.3.0

### Added

- **Ecosystem token packs** (`rules/packs/`). Language-specific tokens move out
  of the shipped catalog and into eight balanced packs (EVM/Solidity, Vyper,
  Move, Rust/Solana, Cairo, Cosmos/Go, TON, Polkadot/Substrate). The *presence*
  of any one ecosystem's tokens now says nothing about which ecosystem the author
  hunts. The catalog refers to a pack; it carries no ecosystem token itself.
- **`scripts/check_tripwires.py`** — three repo-wide checks: no private-workspace
  path prefixes anywhere; at least 7 balanced ecosystem packs; no pack token in
  the embedded catalog. Wired into CI.
- **Generated rule pages** (`docs/rules/P1.md`–`P22.md` plus an index), produced
  by `scripts/gen_rule_docs.py` from the catalog and
  `docs/rules/_annotations.yaml`. `--check` fails on drift and runs in CI, so a
  page cannot disagree with the rule it documents.
- **The calibration harness, published** as `tools/calibrate/calibrate.py`,
  with the rule-to-gate mapping restated publicly in `docs/gate-mapping.yaml`
  and a synthetic eleven-row fixture under `examples/calibration/` that runs end
  to end in CI with pinned numbers.
- **`docs/control-set.md`** — how to build an accepted/paid set from public
  reports so the false-positive rate can finally be measured. Status:
  **PENDING**, because it has not been built.
- **`docs/prospective-protocol.md`** — the frozen, pre-registered test that
  would produce the first out-of-sample measurement. Status: **NOT RUN**.
- **`docs/bench.md`** — the specification for `vg bench`, plus a synthetic
  four-rejected/four-accepted fixture under `examples/bench/`.
- **`docs/no-network.md`** — the no-network claim, verified with `cargo tree`
  (39 packages, none a network client) and a source grep, with the limits of that
  verification stated.
- **`docs/install.md`** — install paths, the `vg` name collision with the
  bioinformatics tool of the same name, and the fact that the crate ships both
  `vg` and `viability-gate`.
- **`docs/licensing.md`** — the current licence state (MIT throughout), with the
  open question about `rules/*.yaml` listed for the owner rather than resolved.
- **`docs/tripwires.md`** — what the tripwire checks cover, and what they
  deliberately exclude.
- **CI**: format, clippy, release tests, an MSRV job, an OS matrix
  (linux/macos/windows), `cargo deny check`, `cargo audit`, the no-network
  checks, the tripwires, generated-doc staleness, a README staleness check, and
  the calibration fixture.
- **Release workflow** on tag `v*`: five targets, SHA-256 checksums and build
  provenance attestations. It does not publish to crates.io.
- **Dependabot** for cargo and GitHub Actions, with updates grouped.
- **Issue templates** for bug reports and rule proposals, both of which refuse
  target names.
- **`deny.toml`**, **`SECURITY.md`**, **`CONTRIBUTING.md`**.

### Changed

- **`docs/calibration.md` rewritten for honesty.** The in-sample caveat now comes
  first instead of last. The "47% of the shortfall" claim is replaced by both
  figures with their denominators: **28 of 59 matched pairs (47%)** and **28 of
  45 non-hits (62%)**. The unmeasured false-positive rate is stated as
  unmeasured rather than left implied. Reproduction commands no longer point at
  private paths.
- Rule pages are the source of the "escape hatch" and "known false-positive"
  documentation; the README's catalog table becomes a summary that links to them.

### Removed

- Private-workspace path prefixes from the shipped catalog, the docs and the
  reproduction instructions. Each rule's source is now `doc: docs/rules/Pn.md`.

## 0.2.0 — 2026-10-09 (untagged; predates the history reset)

### Added

- `--known-issues`: a user-supplied corpus of a program's known issues and
  prior-audit findings, matched against the draft finding. The honest status is
  **unvalidated** — see `docs/calibration.md`.
- `vg init`: scaffolds the four input files a new target needs, commented.
- The lenient calibration metric, reported alongside the strict one.
- A `viability-gate` binary alongside `vg`, because `vg` collides with an
  unrelated bioinformatics tool.

### Changed

- The scaffold no longer offers the removed `any` tier shape.

## 0.1.0 — 2026-10-08 (untagged; predates the history reset)

### Added

- The CLI: 22 anti-patterns (P1–P22) and 6 pre-hunt gates (G1–G6).
- `check`, `gates`, `explain`, `rules`.
- The rule catalog and gate definitions as data, embedded in the binary.
- The fixture suite: one positive and one negative case per anti-pattern.

[Unreleased]: https://github.com/KhomDev/viability-gate/compare/v0.5.0...HEAD
[0.5.0]: https://github.com/KhomDev/viability-gate/compare/v0.4.4...v0.5.0
[0.4.4]: https://github.com/KhomDev/viability-gate/compare/v0.4.3...v0.4.4
[0.4.3]: https://github.com/KhomDev/viability-gate/compare/v0.4.2...v0.4.3
[0.4.2]: https://github.com/KhomDev/viability-gate/compare/v0.4.1...v0.4.2
[0.4.1]: https://github.com/KhomDev/viability-gate/compare/v0.4.0...v0.4.1
[0.4.0]: https://github.com/KhomDev/viability-gate/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/KhomDev/viability-gate/releases/tag/v0.3.0
