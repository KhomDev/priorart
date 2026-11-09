# Changelog

All notable changes to this repository. Format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versioning is
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

The published dataset is a derived artifact. A schema change - a column, or a
vocabulary value, added, removed or renamed - is a breaking change for anyone
consuming `data/`, so it gets a major version. A change to which rows hold which
values is a data release and gets a minor version.

## [Unreleased]

## [0.6.0] - 2026-11-06

A data and disclosure release. Still 113 rows, the same ten columns, and the same
vocabularies, so no schema change. Fifteen rows change value in derived or
family-level columns (listed below). Every triage, gate and root-cause count is
unchanged.

### Security

- **The classifier that reads private text is no longer published.** Until 0.5.1,
  `scripts/build_public_export.py` contained the rules that map private report
  titles and sponsor correspondence to vocabulary values. Those rules necessarily
  quoted that text, including code identifiers and ecosystem-specific terms, and
  `docs/patterns.md` described individual findings in prose. The published `data/`
  never contained private text; the code and docs around it did. The classifier
  now runs privately and hands the public build vocabulary values only. The public
  token-tripwire list, which itself said which ecosystems were being hidden, and the
  domain-specific part of the blocklist verifier's allowlist moved to private inputs
  as well.
- **Repository history was reset** to remove that material from every earlier
  commit and tag. The tags v0.3.0 to v0.5.1 and their GitHub releases no longer
  exist; their changelog entries are kept below. Copies taken before the reset
  cannot be recalled.
- **Takedown requests now go through private vulnerability reporting**, not a
  public issue, which would itself have said which program the requester works on.

### Changed

- **The build has two stages.** A private classifier writes a vocabulary-only CSV;
  the public `build_public_export.py --classified` validates it, applies the
  withholding rule, derives `family_id`, `repeat_of_own`, `duplicate_source` and
  `preventable_by` with rules in the new `scripts/vocab.py`, checks the invariants
  between columns, and writes `data/`. `--explain-classes` and `--source` /
  `--blocklist` moved to the private stage.
- **Rows are keyed by a stable hash, not by position.** The 0.5.1 per-row tables
  were indexed by row position in the private file. Rows had since been inserted
  into that file, and rebuilding with 0.5.1 would have passed validation while
  moving 382 cells onto the wrong rows. The private ledger now keys each row by a
  hash of its protocol and title; `R-###` ids are assigned once and never change.
- **The headline is the preventable share, not the earliest-gate share.** 63 of
  113 (55.8%) could have been stopped from the public record. The earlier headline,
  "78 of 113 (69.0%) catchable by six pre-hunt gates", restated the claim 0.2.0
  retracted: it counts duplicates whose signal the log does not record, a
  concurrent duplicate and the withheld rows, and the six gates all need the finding
  in hand. `prehunt_catchable` stays in `summary.json` for compatibility, documented
  as what it is.
- `summary.json` gains `published_rows`, `rows_in_repeated_families`,
  `public_record_preventable`, `public_record_preventable_pct`, `duplicate_unknown`,
  `root_cause_other`, `mechanism_classes_used`, `singleton_mechanism_classes` and
  `unique_category_tuples`. Every 0.5.x key is unchanged in name and value.
- New `data/provenance.json`: the SHA-256 of the private source, classifier and
  ledger the release was built from, and `removed_ids` for takedowns.
- The README's sample-size and identifiability sentences are generated, and every
  number left in the prose is checked by `tests/test_docs_numbers.py`.
- `query.py` checks filter values against the vocabulary, and its overview reports
  the preventable share instead of calling the six gates "runnable before reading
  any code".

### Fixed

- **`preventable_by` ignored repeats of the author's own family** (8 rows). All
  rows with `repeat_of_own = true` were catchable from the author's own log, yet
  R-047, R-048, R-081, R-086, R-087 and R-088 said `unknown` and R-056 and R-096
  said `none`. They are now `own_log`, and those six duplicates have
  `duplicate_source = own_prior`. `own_log` goes from 1 to 9.
- **`preventable_by` ignored the duplicate signal off the D1 gate** (5 rows).
  R-021, R-043 and R-045 are known-issue duplicates recorded at another gate and said
  `none`; R-016 and R-044 record D1, are not duplicates, and have a known-issue signal
  in the log, and said `unknown`. All five are now `public_record`.
- **Two families spanned two classes.** A family is one bug. R-067 now carries its
  family's `bug_family` (`input_validation`, was `oracle`) and R-092 its family's
  `mechanism_class` (`signature_context_binding`, was `replay_scope`).
- Net effect: `public_record` 58 -> 63, `own_log` 1 -> 9, `unknown` 15 -> 7,
  `none` 36 -> 31; `duplicate_source` `unknown` 12 -> 6, `own_prior` 1 -> 7.
- **Windows clones failed the test suite.** A UTF-8 byte-order mark at the start
  of `.gitattributes` made git ignore `eol=lf` at checkout, so every file came out
  CRLF and `stats.py --check` failed. The BOM is gone and `check_hygiene.py` fails
  on a BOM in any tracked file. CI now also runs on Windows.
- **The wheel was empty.** `pyproject.toml` declared a console script whose module
  was never packaged. The repository is now declared as a dataset run from a
  checkout; `pip install .[dev]` installs only the pinned tools.
- `--aggregates-only` now deletes per-row files left in `--out` by an earlier build,
  and `summary.json` is written with LF line endings by its single writer.
- False or stale prose: the claim that privileged-role clauses were the largest
  out-of-scope sub-cause; 97-row arithmetic in `docs/gates.md`; "12 rows sit in 9
  families" (21 rows sit in them; 12 are repeats); "30 of its 32 classes ... across
  94 published rows"; a `jq` command reading a column removed in 0.2.0; "four months";
  a `docs/gates.md` gate list that did not match the code's.
- `check_vocabulary.py` always skipped in CI and checked only gate ids. It now runs
  against viability-gate at a pinned tag and checks every shared id's title. It found
  one collision: `P18_design_tradeoff` here, "known characteristic documented in a
  prior audit" in viability-gate. No row uses it; it is documented, the check fails if
  one ever does, and the value is retired at the next breaking release.
- `check_links.py` now checks same-file fragment links; `check_markdown.py` checks that
  `CITATION.cff` and the CHANGELOG agree with `pyproject.toml`.

### Added

- Tests for the build on a synthetic fixture, for every derivation rule, and for
  every way `--check` must fail; tests for the hygiene, tripwire, vocabulary, link and
  blocklist tools; and a test that pins every number in the prose to `summary.json`.
- `verify_blocklist.py --scan-repo`: fails if any tracked file contains an
  identifier from the private source or five consecutive words copied from it. It
  also harvests lowerCamel and snake_case identifiers, which it missed before.
- `check_hygiene.py` with the private inputs scans commit messages and tag messages
  as well as every blob in the history.
- CI: actions pinned to commit SHAs, tools pinned through `pyproject.toml`, a Windows
  job, a job that runs the full build on the fixture in both modes, and a job that
  checks the vocabulary against the pinned viability-gate tag.

## Repository history reset - 2026-11-06

History was reset to remove the published classifier described under 0.6.0. The
entries below describe releases whose tags no longer exist.

## 0.5.1 - 2026-11-05 (tag removed in the 2026-11-06 reset)

A maintenance release. The data is unchanged: still 113 rows, byte-identical
`data/` to 0.5.0.

### Fixed

- The package description in `pyproject.toml` still said 111 rejections; it now
  says 113, matching the dataset.

### Changed

- CI runs with a read-only `GITHUB_TOKEN` (`permissions: contents: read`).
- README "Related" links the new sibling repository,
  [realism-gate](https://github.com/KhomDev/realism-gate).

## 0.5.0 - 2026-10-28 (tag removed in the 2026-11-06 reset)

A data release: **+2 rejections, now 113 rows** (was 111). No schema change, so a
minor version - every column, vocabulary and withholding rule is unchanged, and
the export validates against the same allowlist.

### Added

- **2 newly triaged smart-contract rejections** classified onto the existing
  closed vocabularies (no new `bug_family` or `mechanism_class` was invented to
  fit a row). Both are `informational` with `earliest_gate = P17_no_attacker_win`
  (a permanent-liveness wedge with no attacker-win scenario), landing in the
  `liveness` bug family and the `liveness_block` mechanism class.
- The headline split moves with the data: `informational` is now 24 (was 22),
  pre-hunt-catchable is 78 of 113 (69.0%, was 70.3%), and the effective sample is
  98 distinct families (was 96). The duplicate buckets are unchanged (43 by
  triage, 41 at `D1_duplicate_check`).

### Changed

- **The build is now reproducible from source.** *(Not so, as found in 0.6.0:
  the per-row tables were keyed by row position, and the private file later had
  rows inserted.)* `scripts/build_public_export.py`
  gained the `ROW_BUG_FAMILY` entries for rows 98-113 and a documented
  `ROW_OWNER_OVERRIDE` table, so re-running the build against the private source
  regenerates the published `data/` byte-for-byte. Previously the 0.4.0 data
  carried per-row classifications (the rows-98-111 families and the two
  owner-reclassified rows) that the committed script could not reproduce.
- **License wording reconciled.** `LICENSE-DATA` previously scoped CC BY 4.0 to
  `data/` only, while the README listed `docs/` and `README.md` as CC-BY-4.0 too.
  `LICENSE-DATA` now states the CC-BY-4.0 scope covers the published dataset and
  the prose, with code remaining MIT under `LICENSE`.

## 0.4.0 - 2026-10-20 (tag removed in the 2026-11-06 reset)

A data release: **+14 rejections, now 111 rows** (was 97). No schema change, so a
minor version - every column, vocabulary and withholding rule is unchanged, and
the export validates against the same allowlist.

### Added

- **14 newly triaged smart-contract rejections** classified onto the existing
  closed vocabularies (no new `bug_family` or `mechanism_class` was invented to
  fit a row). After bucketing the batch is 7 `duplicate`, 6 `informational` and
  1 `intentional_design`.
- **Two rows reclassified as no-attacker-win (owner decision).** The two rows
  flagged at review are `informational` with `earliest_gate = P17_no_attacker_win`
  and `duplicate_source = not_duplicate`, rather than `duplicate`.
- `other_mechanism` is **no longer empty**: 4 rows landed in the residue class,
  exactly as the schema said could happen. This is visible in `summary.json`
  rather than hidden by forcing a closer-but-wrong class.

### Changed

- Regenerated `data/summary.json` and the README generated block from the
  enlarged `data/rejections.csv`. Headline shifts: effective n 82 -> 96 distinct
  families; pre-hunt-gate catchable 71/97 (73.2%) -> 78/111 (70.3%); duplicate
  share 37.1% -> 38.7%; the broad "analysis did not hold up" split 33.0% -> 35.1%.
- Narrative row counts in `README.md` and `data/SCHEMA.md` updated from 97 to 111
  (and the derived per-bucket counts) to match the regenerated aggregates.

## 0.3.0 - 2026-10-14 (tag removed in the 2026-11-06 reset)

The first tagged release in this repository. It also contains the history-reset
fix below. The dataset and its schema are unchanged.

### Fixed

- **Eight code-fence lines in `README.md` were `` `@ `` instead of three
  backticks**, so four blocks (the CSV sample, two shell blocks and the BibTeX
  citation) rendered as plain text on GitHub.
- **The README told readers to pin `v0.2.0`**, a tag that does not exist after
  the history reset. The pin is now `v0.3.0`.
- **GitHub could not identify the licence.** `LICENSE` carried the data-licence
  section after the MIT text, which defeats licence detection. `LICENSE` is now
  the MIT text alone; the CC BY 4.0 data-licence section moved, unchanged, to
  `LICENSE-DATA`.

### Added

- **`scripts/check_markdown.py`**, run in CI: fails on a `` `@ `` fence line, on
  an odd number of fences in any tracked Markdown file, and when the tag the
  README pins differs from the version in `pyproject.toml`.
- **Dependabot** for pip and GitHub Actions, grouped.

### Changed

- CI actions moved to `actions/checkout@v7` and `actions/setup-python@v7`; the
  v4/v5 majors target Node 20, which the runners no longer support natively.
- `CITATION.cff` now carries `version`, and `date-released` is this release.

## Repository history reset - 2026-10-10 (the first)

Repository history was reset on 2026-10-10 to remove identifying material from
earlier commits; earlier versions are no longer available.

The tree itself is unchanged except for the tripwire fix recorded below.

### Fixed

- **The identity-token matcher used the wrong boundary.** It compared the
  character either side of a hit against a set that treated `-`, `_` and `.` as
  separators, so an ordinary English word that merely contained a token was
  reported as that token. The matcher is now a word boundary, the per-file
  allowlist it needed is gone, and `tests/test_token_scan.py` pins both
  directions: words that contain a token pass, and every declared token fails.
- The file walk now uses `git ls-files`, so the gitignored private redaction
  blocklist is not reported as a leak by the author's own local run.

## 0.2.0 - 2026-10-10 (untagged; predates the history reset)

### Breaking

- **`mechanism` (free text) is replaced by `mechanism_class` (closed
  vocabulary of 31 classes).** The old column was authored prose validated
  against a denylist. A denylist cannot catch a paraphrase, so the hole was
  structural. Every published value is now drawn from a vocabulary and validated
  by **allowlist**: a value outside the vocabulary is a build failure.
- **The `paid` column is removed.** It was constant (`no` in all 97 rows) and
  implied a positive class that does not exist. The dataset contains rejections
  only; that is now stated as a limitation instead of carried as a column.
- **The gate id `P2_duplicate_check` is renamed to `D1_duplicate_check`.** In
  [viability-gate](https://github.com/KhomDev/viability-gate), `P2` means
  "privileged-role-gated". One token meaning two things across two repositories
  was a documentation bug. The rename is applied to the data, the schema, the
  query tool, the tests and the docs.
- **Four columns added:** `family_id`, `repeat_of_own`, `duplicate_source`,
  `preventable_by`.
- **`below_payout_floor` rows are now heavily redacted.** They ship only
  `bug_family`, `triage_class`, `earliest_gate`, `root_cause_class` and
  `mechanism_class = withheld_valid`. The other four fields are the literal
  value `withheld`.

### Added

- `scripts/stats.py` computes every aggregate into `data/summary.json` and
  rewrites the generated block in the README. `--check` fails if either is
  stale, and the test suite runs it.
- `effective_n`: the number of distinct `family_id`s (82), reported alongside
  the 97 rows.
- Two explicit definitions of the "did the analysis hold up" split, because the
  published 16.5%/84% framing was not a definition. Strict (spam only) is 1;
  broad (spam + informational + intentional design) is 32 of 97.
- A `preventable_by` split replacing the "73% preventable before reading a line
  of code" claim, which counted a concurrent duplicate that nobody can pre-hunt.
- `scripts/query.py` filters for the new columns.
- `SECURITY.md`, `CONTRIBUTING.md`, this changelog, and issue templates for
  corrections and takedown requests.

### Fixed

- Mojibake (mis-decoded dashes and accents) removed from every tracked file, and a check added
  that fails on those byte sequences.
- Orphaned table rows after the blocklist paragraph, and a duplicated
  "Finding IDs" row.
- The "only 4% were about impact being wrong" sentence, which contradicted the
  repository's own table. Replaced with the two definitions above.

### Known limitations

Unchanged and restated: no positive class, one researcher, four months,
self-assessed gate assignments, in-sample rule derivation, and an `unknown`
bucket in `duplicate_source` (12 rows) and `preventable_by` (15 rows) that is
reported rather than guessed away.

## 0.1.0 - 2026-10-08 (untagged; predates the history reset)

### Added

- Initial release: 97 logged rejections, classified by triage outcome and
  earliest catchable gate.

[Unreleased]: https://github.com/KhomDev/rejection-taxonomy/compare/v0.6.0...HEAD
[0.6.0]: https://github.com/KhomDev/rejection-taxonomy/releases/tag/v0.6.0
