# Architecture

This page explains how priorart is put together: why each part exists, what crosses each
boundary, and what the user sees when a part fails. It describes `main` after 0.1.1. Where it
disagrees with the source, the source wins.

## Three questions, one dossier

priorart answers three questions about a draft finding, always with quoted evidence and never
with a verdict: *has this been found? is it excluded? can it be paid?* It works in two steps.
`build` turns a folder of files the user saved into one deterministic JSON dossier. `check`
reads that dossier and a finding and assembles the evidence.

```text
   program folder (untrusted, user-saved)
   scope.md|html|txt  known-issues.*  audits/*.md|txt|pdf  program.yaml
                │
                ▼
   dossier/import.rs ── pdftotext (subprocess, PDFs only, fallback)
                │  sources: path, sha256, page count
                │  items:   page, verbatim quote, identifiers, clause class, mechanism hint
                ▼
   dossier/build.rs ──► <program>.dossier   (priorart.dossier/v1, byte-stable)
                                   │
   finding.md (untrusted) ─────────┤
                                   ▼
                    dossier/load.rs (schema check)
                                   │
                    check.rs ──► match/   rank prior items (identifiers)
                             ├─► gates::exclusions  (G2)
                             ├─► gates::payout      (G1)
                             ├─► gates::scope       (G3)
                             └─► gates::prior_audit_coverage / prior_art (G4)
                                   │
                                   ▼
                   render/text  ·  render/json (priorart.check/v1)
                                   │
                                   ▼
                  cli.rs: exit 0 / 1 (opt-in) / 2
```

## Components

| Component | Files | Why it exists | Must never |
|---|---|---|---|
| Import | `dossier/import.rs` | Reads the program folder in a fixed order (program page, known issues, audits by file name) and splits it into items with page numbers and verbatim quotes. A PDF is scraped for text literals first, then read with `pdftotext` | fetch a URL, or invent text |
| Fingerprints | `dossier/sha256.rs` | SHA-256 of each source, in-tree, so the crate has no hashing dependency | be used as a security boundary (it's an identifier) |
| Build | `dossier/build.rs` | Sorts sources and items, assigns ids from those orders, and writes no wall-clock field. Two builds of one folder are byte-identical (tested in `tests/cli.rs`) | depend on file-system order or time |
| Load | `dossier/load.rs` | Reads and validates a dossier against `priorart.dossier/v1` before anything uses it | accept a dossier with an unknown schema |
| Model | `model.rs` | The two documents the tool emits (`Dossier`, `CheckReport`) and the inputs the gates read. The JSON Schemas in `schemas/` are enforced by tests | — |
| Matcher | `match/mod.rs` | Ranks prior items by shared code identifiers (functions, errors, contracts, qualified names, backticked spans). An item needs at least `MIN_SHARED_IDENTIFIERS` (2) symbols. A symbol in more than `GENERIC_DF_PERCENT` (20%) of items is generic and doesn't count, unless only one or two items exist. Paths break ties but never qualify an item alone. Word overlap isn't scored | say two findings are the same |
| Gates | `gates/mod.rs` | G1 payout, G2 exclusion, G3 scope, G4 prior audits. Each reports what it read and what it didn't | pass or fail a finding |
| Assembly | `check.rs` | The one place that decides what `check` reports. It calls the matcher and each gate and collects their evidence, without collapsing it into a verdict | — |
| Vocabularies | `vocab/` + `vocab/*.yaml` | 5 payout tier shapes, 14 clause classes and the gate cross-reference, compiled in with `include_str!` | change without a release |
| Rejections | `rejections/` + `data/rejections/` | The rejection dataset (from rejection-taxonomy v0.6.1), compiled in. A descriptive vocabulary, not a base rate | be read from disk at runtime |
| Ledger | `ledger/mod.rs` | `priorart ledger validate`: the Rust port of bounty-economics `validate.py`, for the user's transcription of program terms (schema v1, byte-identical) | accept an entry without a citation |
| Init | `init.rs` | `priorart init`: scaffolds the folder `build` reads | overwrite without `--force` |
| Render | `render/text.rs`, `render/json.rs` | Text for people, JSON for scripts. One pretty-printer and one trailing newline, so files are diffable | print a duplicate verdict (retrieval-only since 0.1.0) |
| CLI | `cli.rs`, `main.rs`, `error.rs` | Commands, flags, exit codes. Every flag is in docs/cli.md and every command is exercised by `tests/cli.rs` | — |

## A run, end to end

`priorart build examples/dossier-demo`, then `priorart check examples/finding-demo.md --dossier …`:

1. `import::read_program_dir` reads `scope.md`, `known-issues.md`, `audits/internal-review.md`
   (`audits/README.md` is skipped as scaffolding) and `program.yaml`. Each source gets a
   sha256 and a page count. Each item gets its quote, identifiers, a clause class (for exclusion
   text) and a mechanism hint (a keyword table, `null` when it's negated or unknown).
2. `build` sorts and numbers everything and writes `dossier-demo.dossier`.
3. `check` loads and validates the dossier and reads the finding. `match::Profile::from_text`
   extracts the finding's identifiers, and `match::rank` returns the top-k prior items, each
   with the identifiers it shares.
4. The gates run on the same dossier and finding text: payout against `program.yaml`'s payout
   block, exclusions against the finding's preconditions (`gates::preconditions`,
   negation-aware), scope against cited paths and contract names, and prior-audit coverage.
5. `render::text` prints the three sections and closes with "reading list only. priorart does
   not decide duplicates". The exit code is 0.

## States and contracts

**Exit codes** (`error.rs`, `check::failure_exit`):

| Code | Meaning |
|---|---|
| 0 | analysis produced (whatever it found) |
| 1 | only with an opt-in flag: `--fail-on-excluded` and at least one exclusion matched, or `--fail-on-unpayable` and the payout gate returned `payable: false` |
| 2 | usage, IO or parse error |

**Payout (G1)** has three states: `payable: true`, `payable: false`, and `payable: null`. Null
covers no payout block, no `source_url`, or no claimed severity. Null is never "payable", and
`--fail-on-unpayable` doesn't fire on it.

**Unread is not a pass:** an input the import skips (an unreadable PDF, for example) is always
listed as a warning (`dossier/import.rs`). A gate that couldn't run says why in its `detail`,
for example "program.yaml has no payout block, so the payout gate did not run". It never
reports a pass.

## Trust boundaries

| Boundary | What crosses it | How it is contained |
|---|---|---|
| Program folder → import | Markdown, HTML, text, YAML and PDF the user saved, often from a third party | parsed as text; unknown `program.yaml` keys are ignored; no size limits yet (planned) |
| priorart → pdftotext | an untrusted PDF, read by a third-party parser | only when the built-in scrape finds no text; a missing `pdftotext` is a warning, and the build still succeeds. No timeout or output cap yet (planned) |
| Finding → matcher and gates | the user's draft | regexes and keyword tables only; nothing is evaluated |
| Dossier file → check | JSON that may have been edited | validated against the schema on load |
| Network | — | none: no HTTP client in the manifest or lockfile, checked by a CI job ([docs/no-network.md](docs/no-network.md)) |

## Failure modes

| Failure | What the user sees |
|---|---|
| A compressed PDF and no `pdftotext` | a warning on build; the file is listed as unread; the build succeeds |
| A scanned, image-only PDF | no text; the file must be converted outside priorart (OCR isn't attempted) |
| A finding with fewer than 2 symbol identifiers | a note that prior items can't be ranked; the gates still run |
| A prior item that shares identifiers but a different mechanism | it is listed, with `their mechanism` against `yours`; ranking ignores the mechanism |
| A false alarm (an unrelated item ranked) | measured at 26/60 = 43.3% on the public benchmark; that's why the output is a reading list ([docs/benchmark.md](docs/benchmark.md)) |
| A wrong transcription in `program.yaml` | G1 is confidently wrong; the ledger validator rejects an uncited entry |
| An invalid dossier | exit 2 with the schema error |

## Where to change things

- **A new clause class:** edit `vocab/clause-classes.yaml`, add a test in `vocab/`, and
  regenerate the docs listed in docs/vocab.md.
- **A matcher change:** change `match/mod.rs` only after freezing a fresh benchmark corpus
  (docs/benchmark.md). Report FAR and duplicate recall together, with n and intervals.
- **A new output field:** add it to `model.rs` and `schemas/check.schema.json`. Fields are
  added within v1, never removed or renamed.
