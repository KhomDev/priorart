# Threat model

A short, STRIDE-light model of priorart. Every statement names the code or the
test that backs it. Line numbers are for commit `5b58846` on `main`; they move
when the code moves, so the names are the stable reference.

Each guarantee has a status:

- **Tested**: a test or CI job fails if the guarantee breaks.
- **Read**: true in the code as read, with no test that would catch a regression.
- **Unverified**: a claim or hypothesis not checked. It is listed so it can be
  tested, not so it can be relied on.

## Assets

| Asset | Why it matters |
|---|---|
| The saved program files and the finding under review | Unreleased findings and private notes. priorart reads them and writes quotes from them into the dossier. |
| The user's machine | priorart runs on an audit workstation, often offline and often next to other sensitive material. |
| The honesty of the output | The product is quoted evidence with page numbers, not a verdict. A quote that is not in the source, or a verdict that is not supported, is the failure that matters most. |

## Attackers

| Attacker | Capability | In scope |
|---|---|---|
| X1. A malicious document in a program folder | Controls the bytes of `scope.*`, `known-issues.*`, `audits/**` (`.md`, `.markdown`, `.txt`, `.pdf`) | Yes |
| X2. A malicious structured file | Controls `program.yaml`, `known-issues.yaml`/`.yml`, a ledger YAML | Yes |
| X3. A planted dossier | Puts a `.dossier` file beside a finding, or points `--dossier` at one | Yes |
| X4. A prompt-injected agent | Drives the CLI with arguments, and reads its output | Yes |
| X5. A compromised dependency, toolchain or release | Ships malicious code in a crate, in `pdftotext`, or in a release archive | Partly (see the open items) |
| X6. A user with write access to `PATH` or the working directory | Already controls the user's shell | No |

## Trust boundaries

Each row is a point where bytes from an attacker-influenced file are parsed.

| Input | Parser | Reference | Limits today |
|---|---|---|---|
| Program page, known issues, audits (text) | Lossy UTF-8, then line and heading scan | `src/dossier/import.rs:398-444`, `:429` | None: the whole file is read |
| PDF, uncompressed | Byte scan for literal strings, in process | `src/dossier/import.rs:513-568` | None |
| PDF, compressed | Subprocess `pdftotext -layout -q <absolute path> -` | `src/dossier/import.rs` `find_on_path`, `run_pdftotext` | Killed after `PDFTOTEXT_TIMEOUT` (60 s) or `PDFTOTEXT_MAX_OUTPUT` (16 MiB); found on absolute `PATH` entries only, never the current directory; the PDF is passed as an absolute path, so it can't be read as an option |
| `program.yaml` | `serde_saphyr::from_str`, unknown keys ignored | `src/dossier/import.rs` `read_program_file` | `MAX_INPUT_BYTES` (64 MiB) per file |
| Known issues as YAML | `serde_saphyr::from_str` | `src/dossier/import.rs:888-893` | None |
| Ledger | `serde_saphyr::from_str` on one YAML document | `src/ledger/mod.rs:122-133` | None |
| Finding (markdown) | Lossy UTF-8 | `src/check.rs:105-116` | None |
| Dossier JSON | `serde_json`, schema id checked before the struct | `src/dossier/load.rs:9-28` | None |
| Directory walk of `audits/` | `read_dir`, recursion | `src/dossier/import.rs` `collect_at_depth`, `check_input_limits` | Depth `MAX_WALK_DEPTH` (16), `MAX_WALK_ENTRIES` (10 000) entries, `MAX_INPUT_FILES` (1000) documents, `MAX_TOTAL_BYTES` (512 MiB); exit 2 naming the limit |
| Vocabularies and rejection data | Compiled in with `include_str!` | `src/vocab/mod.rs:82`, `src/rejections/mod.rs:19,27` | Trusted: part of the build |
| Terminal output | Control characters stripped | `src/render/text.rs:10-15` | JSON output is escaped by `serde_json` |

## Guarantees and what proves them

| ID | Guarantee | Proof | Status |
|---|---|---|---|
| G1 | The crate makes no network calls | `tests/cli.rs::the_crate_has_no_http_client` (no HTTP crate in `Cargo.toml` or `Cargo.lock`, no `std::net` or socket in `src/`); CI job `no-network` in `.github/workflows/ci.yml` | Tested. The check is by crate name. Nothing inspects what a dependency's build script does. |
| G2 | Two builds of one folder are byte-identical | `tests/cli.rs::build_writes_one_deterministic_dossier` | Tested |
| G3 | Output matches the shipped schemas | `tests/cli.rs::build_json_is_valid_against_the_shipped_schema`, `check_json_is_valid_against_the_shipped_schema`, `the_ledger_schema_is_the_shipped_v1_contract` | Tested |
| G4 | Exit codes are 0, 1 and 2; exit 1 only from `--fail-on-excluded` or `--fail-on-unpayable` | `src/error.rs:15-18`; `tests/cli.rs::check_missing_finding_is_a_usage_error`, `fail_on_excluded_exits_one_only_when_a_clause_matches`, `fail_on_unpayable_exits_one_below_the_floor`, `ledger_validate_reports_errors_and_exits_one` | Tested |
| G5 | A foreign, malformed or internally inconsistent dossier is rejected, not read as current | `tests/cli.rs::check_rejects_a_malformed_or_foreign_dossier`; `tests/adversarial.rs::a_page_number_past_the_source_page_count_is_flagged`; schema and page-range checks in `src/dossier/load.rs` | Tested |
| G6 | A PDF with no readable text is a warning, never a panic, and the build still exits 0 | `tests/cli.rs::a_pdf_without_extractable_text_is_a_warning_not_a_panic` | Tested for one compressed-stream PDF. Other malformed PDFs are not tested. |
| G7 | Quotes are text recorded from a saved file, with an in-range page number | `tests/cli.rs::check_prints_three_labelled_sections_with_a_whole_sentence_quote`; quote check in `src/check.rs`; page range in `src/dossier/load.rs`; `tests/adversarial.rs::a_hand_edited_quote_that_is_in_no_source_is_flagged`, `a_page_number_past_the_source_page_count_is_flagged` | Tested for quote-vs-recorded-text and page range. The recorded text is not re-read from the source, so a dossier that edits both is not detected (open item 6). |
| G8 | The dossier records the SHA-256 of each source | `src/dossier/sha256.rs`, checked against NIST vectors (per `docs/dossier.md`); `src/dossier/import.rs:438` | Read. The vector test is the claim in `docs/dossier.md`; not re-checked here. |
| G9 | The build writes only the dossier file, and a program id cannot name a path | `src/dossier/build.rs:45-48` (default path is `<program-dir>/<id>.dossier`); `src/dossier/import.rs:300-317` (`slug` keeps letters, digits, `.`, `-`, `_` only); `slug("../..")` is `None` at `src/dossier/import.rs:986` | Tested for `slug`. The write is tmp-then-rename (`src/dossier/build.rs:58-66`) with no test that a failed write leaves no partial file. |
| G10 | Terminal output cannot carry control characters from input | `src/render/text.rs:10-15` | Read. No test. |
| G11 | Dependencies are checked for advisories, yanked versions, licences and duplicate versions | CI jobs `cargo-deny` and `cargo-audit`; `deny.toml` (`yanked = "deny"`, `multiple-versions = "deny"`) | Tested in CI |
| G12 | Format, lint and tests gate every change | CI `test` job runs `cargo fmt --check`, `clippy -D warnings`, `cargo test --locked --release` on three OSes | Tested in CI |
| G13 | A release archive can be checked against its digest and its build provenance | `.github/workflows/release.yml` writes `SHA256SUMS` (archives and SBOM) and attests each archive and the SBOM on tag runs; `.github/workflows/verify-release.yml` installs a release three ways and checks `SHA256SUMS` | Tested for the digest. The attestation step has not run on a real tag yet: the first one is the next release. |

## Open items

Each item is a hypothesis or a known gap. None is a confirmed exploit.

1. ~~**`pdftotext` has no timeout and no output cap.**~~ *Closed* (Unreleased, PR #23):
   the child is killed after 60 s or 16 MiB, and the source is skipped with a warning.
   Tests: `a_pdftotext_that_runs_past_the_timeout_is_killed`,
   `a_pdftotext_that_floods_stdout_is_killed_at_the_cap` (unit) and
   `a_pdftotext_that_floods_its_output_is_killed_and_the_build_goes_on` (CLI).
2. ~~**`pdftotext` is found through `PATH`, and the path is not protected from being read as an option.**~~
   *Closed* (Unreleased, PR #23): `find_on_path` searches absolute `PATH` entries only,
   and the child is started by its full path, so the current directory is never searched
   on Windows. The PDF is passed as an absolute path, which can't start with `-`.
   Tests: `pdftotext_is_looked_up_on_absolute_path_entries_only`,
   `the_pdf_path_reaches_pdftotext_absolute_and_never_as_an_option`. On Windows a
   `pdftotext.bat` or `.cmd` on `PATH` is accepted too; Rust's standard library escapes
   batch-file arguments and refuses ones it can't escape.
3. ~~**No input limits.**~~ *Closed* (Unreleased, PR #23): per-file size, document
   count, total size, walk depth and walk entries are capped before any file is read.
   Tests: `the_audits_walk_stops_at_its_depth_and_entry_limits` (unit),
   `build_refuses_more_documents_than_the_limit_with_exit_two`,
   `build_refuses_audits_nested_past_the_depth_limit_with_exit_two` (CLI), and
   `input_size_is_checked_per_file_and_in_total` (unit) for the per-file, total and count limits.
4. **Symlinks and dot-files under `audits/` are skipped** (`src/dossier/import.rs:358-366`).
   No test covers this. *Read, untested.*
5. **Parser robustness is sampled, not measured.** The seven `fuzz/` targets and
   `tests/fuzz_replay.rs` replay committed seeds and hostile edges, and the weekly
   workflow runs libFuzzer for five minutes per target. Coverage of the YAML, JSON,
   markdown and PDF-scrape paths is not measured, and the weekly job's crash corpus
   is not part of the release gate. *Partly checked.*
6. **A planted dossier is trusted.** A `.dossier` beside a finding is read as
   evidence (`src/dossier/load.rs`). There is no signature or hash binding it to
   the source files, and the `sha256` fields are not re-checked at `check` time.
   Since 0.1.3 a quote that is not in the item text the dossier records is
   reported, and a page past the source's page count is refused, but a dossier
   that edits both the text and the quote consistently still passes. *Read. By
   design for now.*
7. **Prompt injection through quoted text.** Quotes from `audits/` and `scope.*` are
   printed verbatim. An agent reading the output may treat quoted text as
   instructions. priorart executes nothing from input content, and the output carries
   page numbers and source paths, but no test models an agent. *Unverified in practice.*
8. **Release attestation is in place but not yet exercised.** Since PR #24, tag runs
   attest each archive and the CycloneDX SBOM, and docs/install.md documents
   `gh attestation verify`. Since PR #25, `verify-release.yml` installs a release from
   crates.io, from the archives and through `cargo binstall`. *Read.* The attestation
   step runs on the next tag; until then no priorart release carries one.
9. **No reproducible-build check.** Nothing builds a tag twice and compares digests. *Read.*

## Cross-references

- [SECURITY.md](../SECURITY.md): what counts as a security issue, and how to report one.
- [no-network.md](no-network.md): the offline guarantee in detail (G1).
- [dossier.md](dossier.md): the input layout and the PDF fallback.
- [limitations.md](limitations.md): what the matcher and the gates do not claim.
