# Changelog

Notable changes to `viability-gate`. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Dates are the commit dates. Entries are written after the change, and a number
appears here only if it came from a command that can be re-run.

## [Unreleased]

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

[Unreleased]: https://github.com/KhomDev/viability-gate/compare/v0.4.4...HEAD
[0.4.4]: https://github.com/KhomDev/viability-gate/compare/v0.4.3...v0.4.4
[0.4.3]: https://github.com/KhomDev/viability-gate/compare/v0.4.2...v0.4.3
[0.4.2]: https://github.com/KhomDev/viability-gate/compare/v0.4.1...v0.4.2
[0.4.1]: https://github.com/KhomDev/viability-gate/compare/v0.4.0...v0.4.1
[0.4.0]: https://github.com/KhomDev/viability-gate/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/KhomDev/viability-gate/releases/tag/v0.3.0
