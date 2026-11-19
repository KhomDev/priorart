# Threat model

A short, STRIDE-light model of priorart. Every statement names the code or the
test that backs it. Line numbers are for commit `9841b86` on `main`; they move
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
| PDF, compressed | Subprocess `pdftotext -layout -q <path> -` | `src/dossier/import.rs:476-508` | No timeout, no output cap; resolved through `PATH` by `Command::new` (`:483`); no `--` before the path |
| `program.yaml` | `serde_saphyr::from_str`, unknown keys ignored | `src/dossier/import.rs:202-213` | None: `read_text` reads the whole file (`:465-468`) |
| Known issues as YAML | `serde_saphyr::from_str` | `src/dossier/import.rs:888-893` | None |
| Ledger | `serde_saphyr::from_str` on one YAML document | `src/ledger/mod.rs:122-133` | None |
| Finding (markdown) | Lossy UTF-8 | `src/check.rs:105-116` | None |
| Dossier JSON | `serde_json`, schema id checked before the struct | `src/dossier/load.rs:9-28` | None |
| Directory walk of `audits/` | `read_dir`, recursion | `src/dossier/import.rs:353-374` | No file-count or depth limit |
| Vocabularies and rejection data | Compiled in with `include_str!` | `src/vocab/mod.rs:82`, `src/rejections/mod.rs:19,27` | Trusted: part of the build |
| Terminal output | Control characters stripped | `src/render/text.rs:10-15` | JSON output is escaped by `serde_json` |

## Guarantees and what proves them

| ID | Guarantee | Proof | Status |
|---|---|---|---|
| G1 | The crate makes no network calls | `tests/cli.rs::the_crate_has_no_http_client` (no HTTP crate in `Cargo.toml` or `Cargo.lock`, no `std::net` or socket in `src/`); CI job `no-network` in `.github/workflows/ci.yml` | Tested. The check is by crate name. Nothing inspects what a dependency's build script does. |
| G2 | Two builds of one folder are byte-identical | `tests/cli.rs::build_writes_one_deterministic_dossier` | Tested |
| G3 | Output matches the shipped schemas | `tests/cli.rs::build_json_is_valid_against_the_shipped_schema`, `check_json_is_valid_against_the_shipped_schema`, `the_ledger_schema_is_the_shipped_v1_contract` | Tested |
| G4 | Exit codes are 0, 1 and 2; exit 1 only from `--fail-on-excluded` or `--fail-on-unpayable` | `src/error.rs:15-18`; `tests/cli.rs::check_missing_finding_is_a_usage_error`, `fail_on_excluded_exits_one_only_when_a_clause_matches`, `fail_on_unpayable_exits_one_below_the_floor`, `ledger_validate_reports_errors_and_exits_one` | Tested |
| G5 | A foreign or malformed dossier is rejected, not read as current | `tests/cli.rs::check_rejects_a_malformed_or_foreign_dossier`; schema check at `src/dossier/load.rs:16-26` | Tested |
| G6 | A PDF with no readable text is a warning, never a panic, and the build still exits 0 | `tests/cli.rs::a_pdf_without_extractable_text_is_a_warning_not_a_panic` | Tested for one compressed-stream PDF. Other malformed PDFs are not tested. |
| G7 | Quotes are verbatim text from a saved file, with a page number | `tests/cli.rs::check_prints_three_labelled_sections_with_a_whole_sentence_quote`; page numbering at `src/dossier/import.rs:578-590` | Partly tested. No test compares each quote with its source bytes. |
| G8 | The dossier records the SHA-256 of each source | `src/dossier/sha256.rs`, checked against NIST vectors (per `docs/dossier.md`); `src/dossier/import.rs:438` | Read. The vector test is the claim in `docs/dossier.md`; not re-checked here. |
| G9 | The build writes only the dossier file, and a program id cannot name a path | `src/dossier/build.rs:45-48` (default path is `<program-dir>/<id>.dossier`); `src/dossier/import.rs:300-317` (`slug` keeps letters, digits, `.`, `-`, `_` only); `slug("../..")` is `None` at `src/dossier/import.rs:986` | Tested for `slug`. The write is tmp-then-rename (`src/dossier/build.rs:58-66`) with no test that a failed write leaves no partial file. |
| G10 | Terminal output cannot carry control characters from input | `src/render/text.rs:10-15` | Read. No test. |
| G11 | Dependencies are checked for advisories, yanked versions, licences and duplicate versions | CI jobs `cargo-deny` and `cargo-audit`; `deny.toml` (`yanked = "deny"`, `multiple-versions = "deny"`) | Tested in CI |
| G12 | Format, lint and tests gate every change | CI `test` job runs `cargo fmt --check`, `clippy -D warnings`, `cargo test --locked --release` on three OSes | Tested in CI |
| G13 | A release archive can be checked against its digest | `.github/workflows/release.yml` writes `SHA256SUMS` | Read. Release archives are not attested; see the open items. |

## Open items

Each item is a hypothesis or a known gap. None is a confirmed exploit.

1. **`pdftotext` has no timeout and no output cap** (`src/dossier/import.rs:483-502`).
   A hostile PDF can hang the build or write without limit. *Unverified.*
2. **`pdftotext` is found through `PATH`, and the path is not protected from being read as an option.**
   The path is passed without `--` or an absolute prefix (`:484-486`). On Windows,
   `Command::new` may also search the current directory. *Unverified; to be checked
   against the Rust standard library's Windows behaviour.*
3. **No input limits.** File size, file count and directory depth are unbounded
   (`src/dossier/import.rs:353-374`, `:400`, `:466`, `:477`). *Read.*
4. **Symlinks and dot-files under `audits/` are skipped** (`src/dossier/import.rs:358-366`).
   No test covers this. *Read, untested.*
5. **Parser robustness is not measured.** No fuzzing or property tests exist for the
   YAML, JSON, markdown or PDF scrape paths. Behaviour on YAML alias expansion or
   very large documents in `serde_saphyr` is not checked here. *Unverified.*
6. **A planted dossier is trusted.** A `.dossier` beside a finding is read as
   evidence (`src/dossier/load.rs:34-63`). There is no signature or hash binding it to
   the source files. The `sha256` fields record the sources but nothing checks them
   at `check` time. *Read. By design for now.*
7. **Prompt injection through quoted text.** Quotes from `audits/` and `scope.*` are
   printed verbatim. An agent reading the output may treat quoted text as
   instructions. priorart executes nothing from input content, and the output carries
   page numbers and source paths, but no test models an agent. *Unverified in practice.*
8. **Releases are not attested and no install verification is documented.** The
   release workflow writes `SHA256SUMS` only (`.github/workflows/release.yml`).
   *Read.* Planned in the release PRs.
9. **No reproducible-build check.** Nothing builds a tag twice and compares digests. *Read.*

## Cross-references

- [SECURITY.md](../SECURITY.md): what counts as a security issue, and how to report one.
- [no-network.md](no-network.md): the offline guarantee in detail (G1).
- [dossier.md](dossier.md): the input layout and the PDF fallback.
- [limitations.md](limitations.md): what the matcher and the gates do not claim.
