# Changelog

One repository, two histories. **priorart 0.1.0 is the rename and rewrite of
viability-gate**; the old entries are kept unchanged below a clear header,
because a changelog that deletes its own history is not a changelog.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/);
versions follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
Dates are the commit dates. Entries are written after the change, and a number
appears here only if it came from a command that can be re-run.

## priorart

## [0.1.0] - 2026-11-13

First release under the name **priorart**, from the repository that was
viability-gate. One question, three answers, no verdict: *has this been found,
is it excluded, and can it be paid?* Everything is quoted evidence with a page
number, read from files the user saved. There is no score and no outcome enum.

### Added

- **The dossier.** `priorart build <program-dir>` reads `scope.md|html|txt`,
  `known-issues.md|yaml|txt`, `audits/*.md|txt|pdf` and an optional
  `program.yaml`, and writes one deterministic JSON file per program
  (`priorart.dossier/v1`). Sources carry a SHA-256 and page count; items carry a
  page, a verbatim quote, strong identifiers, a clause class and a mechanism
  hint. Two builds over one folder are byte-identical, and a test asserts it.
  Schema: `schemas/dossier.schema.json`.
- **`priorart check`** with the three sections, `--top-k` (default 5, 1-50),
  `--json` (`priorart.check/v1`), `--fail-on-excluded` and
  `--fail-on-unpayable`. Exit 0 is "analysis produced"; exit 1 is opt-in and
  only from those two flags; exit 2 is usage, IO or parse.
- **The identifier matcher** behind *has this been found*: >=2 shared symbol
  identifiers (function, error, contract, qualified names, backticked spans),
  a 20% generic-symbol filter (with a one-or-two-item exception), ranking by
  specificity, then rarity, then symbol count, then paths, then identifier
  order. File paths are shown and break ties but never qualify an item alone.
  Word overlap is not scored.
- **Gates G1-G4 as evidence**: payout (the claimed severity against the
  transcribed shape and floor), exclusion (the program's clauses against the
  finding's preconditions, negation-aware), scope (cited paths and contract names
  against the in-scope list), prior audits (named coverage plus ranked items).
  Each gate reports what it could not read as unread, never as a pass.
- **`priorart ledger validate`**, the Rust port of bounty-economics
  `scripts/validate.py`. The schema is unchanged at v1 and byte-identical to the
  bounty-economics v0.6.1 tag (sha256
  `403324A1CA205BE9521FE9F51FCCF45030F3BE9B7AA642A2C727DF5842220A51`). An
  uncited entry is an error, an unstated floor is not "Low pays", and the removed
  `any` shape gets a migration message instead of a bare enum failure.
- **`priorart vocab list|show` and `priorart explain`**: the 5 payout tier
  shapes, the 14 exclusion clause classes and the gate cross-reference, compiled
  into the binary with `include_str!`. Tier shapes and clause classes are
  byte-identical to bounty-economics v0.6.1; the gate cross-reference had its
  viability-gate rows removed.
- **`data/rejections/`**: the rejection-taxonomy v0.6.1 tree (CSV, JSON,
  `summary.json`, `SCHEMA.md`, `LICENSE-DATA`, `CITATION.cff`,
  `provenance.json`), 113 rows and 10 columns, summary keys unchanged, CC BY 4.0.
  `priorart rejections summary` reads its own compiled copy.
- **`priorart init`** scaffolds `program.yaml`, `scope.md`, `known-issues.md`
  and `audits/README.md`; the scaffold builds without edits.
- **Offline by construction.** No HTTP client in `Cargo.toml` or `Cargo.lock`,
  no socket in `src/`; a test fails the build if one appears. The vocabularies
  and the rejection dataset are compiled in, so there is no data directory and no
  update check.
- **Docs**: `docs/dossier.md`, `docs/matching.md`, `docs/gates.md`,
  `docs/cli.md`, `docs/ledger.md`, `docs/schema.md`, `docs/vocab.md`,
  `docs/rejections.md`, `docs/benchmark.md`, `docs/validation.md`,
  `docs/limitations.md`, `docs/install.md`, `docs/no-network.md`,
  `docs/licensing.md`, plus [MIGRATION.md](MIGRATION.md).

### Removed

- The prose anti-pattern engine: P1-P26, `rules/anti-patterns.yaml`,
  `rules/packs/`, `docs/rules/`, and the rule documentation generator. It did
  not separate paid from unpaid findings: 12 of the 25 hard-killed findings were
  paid, so the kill's precision was 13/25 (52%) that a finding was unpaid.
- The 0-100 score and the KILL / ROUTE-TO-VERIFY / CLEAR outcome enum. There is
  no replacement score; there are quotes.
- `--strict`, `--funnel`, `--min-score` and `--severity-notes`.
- The citation anchor (moved to realism-gate RG20, `--target` /
  `--target-commit`) and the claim notes P23-P26 / P19 (moved to
  `realism-gate check --report`).
- The old calibration, control-set, prospective-protocol, tripwire,
  negation-audit and rule-defect docs, and the stale `docs/demo.gif` /
  `docs/demo.tape`. `docs/import.md` and `docs/duplicate-matching.md` are
  replaced by `docs/dossier.md` and `docs/matching.md`.

### Changed

- Repository and crate name: viability-gate -> priorart. The GitHub repository
  was renamed (old URLs redirect, history and issues kept). The
  `viability-gate` crate cannot be renamed on crates.io, so it is frozen at
  0.9.0 with a deprecation README and a one-line stderr notice. It still installs
  and still works; **nothing was yanked**.
- One binary, `priorart` (`priorart.exe` on Windows), replacing `vg` and
  `viability-gate`.
- The ledger schema stays v1 and existing private ledgers need no migration.

### Measured

- **Dossier determinism:** two builds over the committed demo folder are
  byte-identical (`tests/cli.rs::build_writes_one_deterministic_dossier`,
  2026-11-13).
- **Public benchmark**, 2026-11-13, final build `dabfcc0c...` (matcher
  behaviour-identical to the `2ec4c3c9...` deciding run; only wording changed
  after it): false-alarm
  rate on 60 judged paid+unique findings **26/60 = 43.3%** (Wilson 95%
  31.6-55.9); duplicate recall@5 on 64 judge-grouped duplicates **39/64 = 60.9%**
  (48.7-71.9); known-prior recall **n=1, 0 hits, not a rate**; label agreement
  **15/20 = 75%**. Two earlier matcher revisions are recorded as provenance, not
  as the number of record: `22648e33...` measured 18/60 = 30.0% and
  `da176bc8...` measured 15/60 = 25.0%. An earlier 0/64 duplicate-recall report
  was a harness locator bug, corrected in
  [docs/benchmark.md](docs/benchmark.md#errata-2026-11-13).
- **Ledger golden files:** `priorart ledger validate` on
  `examples/ledger.example.yaml` + `examples/tier-shapes.example.yaml` prints
  `6 entries, 0 error(s), 10 warning(s)` and exits 0.

### Ship decision: retrieval only

The plan's kill criterion triggers above a 30% false-alarm rate on unique
findings. The final point estimate is **43.3%**, and the 95% interval
(31.6-55.9%) does not cross 30%, so the trigger is met: **priorart 0.1.0 ships in
retrieval-only mode.** The `HAS THIS BEEN FOUND?` section is a reading list with
quotes, sources and pages; it prints no duplicate warning and implies no
duplication. The warning can return only after the matcher is fixed and a fresh
held-out measurement puts the false-alarm rate at or below 30%. The intermediate
25.0% run and the release decision taken on it are superseded, and the benchmark
does not soften that.

### Not claimed

- Not evidence that priorart finds bugs; the corpus is judged human findings.
- Not a population estimate from 60 findings on one contest pair.
- Known-prior recall is n=1 and is not a rate in either direction.
- The matcher is not a duplicate detector, and a hit is not a duplicate finding.
- A gate result is not a verdict; an unrun gate is not clear.
- The rejection dataset is one researcher's log with no positive class, no
  target dimension and no paid comparison; it is a vocabulary, not a base rate.
- Behaviour on private reports, other platforms or scanned (image-only) PDFs is
  not measured.

# viability-gate history (0.1.0 - 0.8.1, this repository before the rename)

The entries below are the viability-gate changelog, unchanged. They document a
tool that no longer exists in this form; read them as history, not as the
current surface.

## [Unreleased] (viability-gate)

## [0.8.1] - 2026-11-10

A correction release. 0.8.0 shipped a precision sentence that inverted the
numerator: 12 of the 25 hard kills were paid findings (48%), so the kill was
right 13/25 (52%) that the finding was unpaid. The 0.7.0 changelog and the
2026-11-10 review both repeated the inversion. The separator is still weak
either way; the number now says what it measures.

### Fixed

- README, `docs/benchmark.md`, `docs/validation.md` and the 0.8.0 changelog
  entry state the paid share (12/25, 48%) and the precision (13/25, 52%)
  separately. The 0.6.0 comparison (19/38, 50%) is unchanged.
- The release workflow publishes the `CHANGELOG.md` section for the tag as the
  release body. The 0.6.0, 0.7.0 and 0.8.0 bodies were the generic install
  template plus GitHub's generated notes, so a reader could not see what the
  release changed.

### Changed

- Version 0.8.0 -> 0.8.1. No rule, gate, outcome or CLI behaviour changed.

## [0.8.0] - 2026-11-10

Not published to crates.io. Install from git.

A review release. The 2026-11-10 Fable 5.2 portfolio review made the
precision problem a blocker: on the public pair a 0.7.0 hard kill was right
13/25 times (52%); 12 of the 25 kills were paid findings (48%), and the kill
hit 12 of 220 paid Medium/High findings (5.5%). The
review's point is not that the number was hidden - the README and benchmark
said it - but that a weak separator was still the default hard stop. This
release changes the default consequence.

### Changed - outcomes

- **A claim-heuristic hard rule is ROUTE-TO-VERIFY by default.** The
  P1-P26 rules no longer stop a submission unless the operator passes the new
  `--strict` flag. A known-issue match (`KI:`) and a failed citation anchor
  (`CITE`) are structured hard failures and stay KILL in every mode.
  `--funnel` is still accepted as a deprecated alias for the default.
- The default hard-kill count on the public pair is therefore **0** (was 12 of
  220 paid). The same 12 paid findings are routed to verification; a route is
  advice, not a stop. The measured separator is still weak (48% of the routes
  are paid findings; 52% are invalid/won't-fix), and `docs/benchmark.md` says
  so in both modes.
- `--strict` reproduces the 0.7.0 behaviour (12 paid hard kills; 13/25, 52%,
  precision) for callers who explicitly accept the trade.

### Added

- **`docs/validation.md`**, a claim-by-claim ledger: every public claim with a
  test or committed result file, a date, and a status (validated / unvalidated /
  retracted). It records the 2026-11-06 audit, what 0.5.0 fixed, and the
  precision history.
- **The hard-stop policy.** A claim rule may be a default stop only with
  precision >= 0.75, >= 20 fires, and a paid hard-kill rate <= 1%. No rule meets
  it today; the rule that comes closest is P2 at 0.53 over 19 fires. The policy
  is in `docs/validation.md`.
- **A release checklist step**: refresh the benchmark (or state why it did not
  change) and update the validation ledger before a tag. The release workflow
  already refuses a tag that disagrees with `Cargo.toml`.

### Fixed - release provenance

- Annotated tags `v0.6.0` and `v0.7.0` are restored on the commits whose
  `Cargo.toml` carries those versions. The 0.6.0 and 0.7.0 releases were on
  `main` with no tag, so the README's "pin to a tag" link and a reader's
  ability to match a claim to a release were both broken.

### Docs

- README tagline, example, benchmark tables, outcome table and limits
  distinguish the 0.8.0 default from `--strict`; the example now shows P18 as
  ROUTE-TO-VERIFY.
- `docs/benchmark.md` carries both tables and states the route precision
  (13/25, 52%; 12/25, 48%, paid share) instead of implying a kill is a verdict.
- Rule pages say a hard rule is `KILL with --strict; ROUTE-TO-VERIFY by
  default since 0.8.0`.

## [0.7.0] - 2026-11-09

Not published to crates.io. Install from git.

Outcomes change. A bare front-run is a hold. An unprivileged yield theft that
mentions an owner or front-running is not killed for those words. "No one but
the admin can" is a kill. A cited file or line that is not in the pinned tree
is a kill and the score is not 100. `--funnel` turns claim-heuristic hard rules
into ROUTE-TO-VERIFY.

### Added

- Gate A1, citation anchor. `--target <dir> --commit <hex-sha>` reads cited
  source files from that commit (`git show`, not the working tree). A missing
  file, a line past EOF, or a function that is not in the file fails the gate
  and emits hard finding `CITE` (weight 40), so the score cannot stay 100.
  Fenced blocks and test, script, and dependency paths are not anchored.
  Unpinned is not a pass. The check does not compare snippets byte for byte
  and does not prove reachability.
- `--funnel`. Claim-heuristic hard rules (P1–P26) become ROUTE-TO-VERIFY.
  Known-issue matches (`KI:`) and `CITE` stay KILL. `--fail-on-kill` does not
  fail on a route.
- Path values that start with `-` are rejected on findings, `--target`,
  `--commit`, `--rules`, and the other path flags. A commit must be a hex SHA.
  This crate has no MCP server; the check is the CLI equivalent of not
  letting a crafted path select flags.

### Changed

- P2. "No one but / nobody except / no one other than" is an affirmation, not
  a negation. An unprivileged yield theft suppresses the rule for the whole
  document, including when another sentence says the owner can set a parameter.
- P3. Base verdict is soft. Hard requires front-run-only, a sole dependence on
  ordering, "before the operator", or operator delay. The same unprivileged
  yield-theft suppression applies.
- P18. Also matches "already been reported", "previously disclosed", and
  "acknowledged as a known issue". It still does not match a bare "prior
  audit". The 0.6.0 bench fired it zero times because judged writeups do not
  contain the admission. That is investigated in docs/benchmark.md, not fixed
  by widening the rule into the opposite claim.
- P26 is opt-in (`--severity-notes`). It no longer fires when no tier is
  asserted, and a severity-matrix sentence satisfies it. On the 0.6.0 bench it
  fired on 340/340 reports.
- README tagline. This is a writing filter, not a validity oracle, and not a
  promise the report will avoid Informative, Duplicate, or out of scope.

### Measured

- Re-scored the unchanged public manifest with `vg` 0.7.0 (`python eval/run.py score`, `python eval/run.py check`). Hard KILL on **12/220** paid Medium/High (5.5%; was 19/220, 8.6%) and **13/120** invalid or won't-fix (10.8%; was 19/120, 15.8%). Precision of a hard kill on that pair is 12/25 (48%). The unpaid/paid kill-rate ratio is about 2.0 (was 1.8). P3 hard-killed 0 paid findings (7 fires, all soft). P2 was unchanged at 9 paid hard kills. P18 fired 0 times. P26 did not run. Details in `docs/benchmark.md`. The separator is still weak.

## [0.6.0] - 2026-11-09

Not published to crates.io. Install from git.

Outcomes change where a negated phrase used to fire, and where a prior-issue
admission used to be missed.

### Fixed

- Contrastive phrases (`rather than`, `instead of`, `as opposed to`, and the
  rest of the list in `docs/negation-audit.md`) negate the words that follow
  them. An immediately preceding `non` negates a hyphenated form. All 26 rules
  were audited; the residuals are in that doc.
- P7 no longer fires on "rather than a best-practice observation."
- P18 fires on "same root cause as", "accepted as <name> issue", hyphenated
  "previously-reported", and an issue id (H-1, M-01, L-3) next to an admission.
  Firm names stay out of the catalog. Suppression is sentence-scoped. A bare
  issue id still does not fire.
- `tests/fixtures/cmp-04-report.md` is the report that produced the false HOLD.
  No personal email was in it.

### Added

- P23–P26, weight 0: attacker profile, preconditions and victim action,
  quantified impact, quoted program severity rule. A `finding.yaml` (or
  `<name>.finding.yaml`) can supply the same fields. A recorded prior-audit
  check does not suppress P18.
- `vg import audit`, `vg import program`, `vg import prior`. Local files only.
  Schema in `docs/import.md`. PDF via an uncompressed scrape, then `pdftotext`
  if it is installed.
- `vg match` and `--index`. D1 ranks prior findings by code identifiers, not
  word overlap. Threshold documented as uncalibrated in `docs/duplicate-matching.md`.
- `eval/run.py` downloads a public Code4rena and Sherlock set and scores it.
  Bodies are not committed. Headline: hard KILL on 19/220 paid Medium/High
  (8.6%) and 19/120 invalid or won't-fix (15.8%). Nine of the paid kills are
  P2, seven are P3. `docs/benchmark.md`.

### Changed

- Snapshots of [rejection-taxonomy](https://github.com/KhomDev/rejection-taxonomy)
  v0.6.0 and [bounty-economics](https://github.com/KhomDev/bounty-economics) v0.5.0
  live under `data/`, with their licences and a `SOURCE.md` each. `check_readme.py`
  reads the vendored CSV. `vg import` writes ledger entries against that schema
  and only emits a clause class the taxonomy lists. The source repositories
  were not modified.
- A weight-0 note no longer hides an unknown gate. The outcome stays
  INCOMPLETE until the gates can run.
- The pinned rejection-taxonomy tag is `v0.6.0` (was `v0.5.1`). rejection-taxonomy
  reset its history on 2026-11-06, so the `v0.5.1` tag no longer exists. The
  `origin_rows` counts did not change: that tag moved columns this repository
  does not read.
- README leads with the pitch, the install line, the CMP-04 output, and the
  benchmark table. Name collision, the dated in-sample study, and the other
  caveats are under Limits.

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

[Unreleased]: https://github.com/KhomDev/viability-gate/compare/v0.8.1...HEAD
[0.8.1]: https://github.com/KhomDev/viability-gate/compare/v0.8.0...v0.8.1
[0.8.0]: https://github.com/KhomDev/viability-gate/compare/v0.7.0...v0.8.0
[0.7.0]: https://github.com/KhomDev/viability-gate/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/KhomDev/viability-gate/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/KhomDev/viability-gate/compare/v0.4.4...v0.5.0
[0.4.4]: https://github.com/KhomDev/viability-gate/compare/v0.4.3...v0.4.4
[0.4.3]: https://github.com/KhomDev/viability-gate/compare/v0.4.2...v0.4.3
[0.4.2]: https://github.com/KhomDev/viability-gate/compare/v0.4.1...v0.4.2
[0.4.1]: https://github.com/KhomDev/viability-gate/compare/v0.4.0...v0.4.1
[0.4.0]: https://github.com/KhomDev/viability-gate/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/KhomDev/viability-gate/releases/tag/v0.3.0
