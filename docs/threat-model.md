# Threat model

A short, STRIDE-light model of priorart. Every statement names the code or the
test that backs it. Line numbers come from the 0.1.3 tree and move with the
code, so the names are the stable reference.

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
| X4. A prompt-injected agent | Drives the CLI with arguments, or `priorart-mcp` with tool calls, and reads the output | Yes |
| X5. A compromised dependency, toolchain or release | Ships malicious code in a crate, in `pdftotext`, or in a release archive | Partly (see the open items) |
| X6. A user with write access to `PATH` or the working directory | Already controls the user's shell | No |

## Trust boundaries

Each row is a point where bytes from an attacker-influenced file are parsed.

| Input | Parser | Reference | Limits today |
|---|---|---|---|
| Program page, known issues, audits (text) | Lossy UTF-8, then the HTML normalizer for `.html`/`.htm` or a line and heading scan | `src/dossier/html.rs`, `src/dossier/import.rs` | `MAX_INPUT_BYTES` (64 MiB) per file, `MAX_INPUT_FILES` (1000), `MAX_TOTAL_BYTES` (512 MiB); a required input that is a symlink, FIFO or device is exit 2, an optional one is named and skipped |
| HTML program page | In-repo converter: comments, doctype, script and style dropped; headings, bullets, tables and `href`s mapped; entities decoded | `src/dossier/html.rs` | 64 MiB like any input. Not a browser: no CSS, no JavaScript, no images, no attribute text other than `href` |
| PDF, uncompressed | Byte scan for literal strings, in process; UTF-16BE/LE and UTF-8 literals decoded; the PDF must be a regular file | `src/dossier/import.rs` `scrape_pdf_literals`, `read_regular_file` | 64 MiB like any input |
| PDF, compressed | Subprocess `pdftotext -layout -q <absolute path> -` (native poppler, same user, no sandbox) | `src/dossier/import.rs` `find_on_path`, `run_pdftotext` | Killed after `PDFTOTEXT_TIMEOUT` (60 s) or `PDFTOTEXT_MAX_OUTPUT` (16 MiB); found on absolute `PATH` entries only, never the current directory; the PDF is passed as an absolute path and must be a regular file, so it can't be read as an option; disabled entirely by `--no-pdftotext` |
| `program.yaml` | `serde_saphyr::from_str`, unknown keys ignored | `src/dossier/import.rs` `read_program_file` | `MAX_INPUT_BYTES` (64 MiB) per file; must be a regular file |
| Known issues as YAML | `serde_saphyr::from_str` | `src/dossier/import.rs` | 64 MiB per file; must be a regular file |
| Ledger | `serde_saphyr::from_str` on one YAML document | `src/ledger/mod.rs` | None |
| Finding (markdown) | Lossy UTF-8 | `src/check.rs` `read_finding` | `MAX_FINDING_BYTES` (16 MiB) and a regular file; the size is checked from metadata before the read, and the ordering is pinned by tests |
| Dossier JSON | `serde_json`, schema id checked before the struct | `src/dossier/load.rs` `load` | `MAX_DOSSIER_BYTES` (64 MiB) and a regular file; the size is checked from metadata before the read, and the ordering is pinned by tests |
| Directory walk of `audits/` | `read_dir`, recursion | `src/dossier/import.rs` `collect_at_depth`, `check_input_limits` | Depth `MAX_WALK_DEPTH` (16), `MAX_WALK_ENTRIES` (10 000) entries, `MAX_INPUT_FILES` (1000) documents, `MAX_TOTAL_BYTES` (512 MiB); symlinks (file or directory) and non-regular documents are named and skipped; exit 2 naming the limit |
| Vocabularies and rejection data | Compiled in with `include_str!` | `src/vocab/mod.rs`, `src/rejections/mod.rs` | Trusted: part of the build |
| MCP stdio (`priorart-mcp`) | Newline JSON-RPC, or `Content-Length` frames; `serde_json` | `src/mcp.rs` `read_message`, `dispatch_with`, `argv_for` | 8 MiB per line or frame (checked before allocation), 8 KiB per header line, 64 header lines; arguments: strict types, no unknown keys, 4096 bytes, 64 list entries, no leading `-`; optional `PRIORART_MCP_ROOT`; each run killed after 120 s (default) and refused over 4 MiB per stream |
| Terminal output | Control characters stripped | `src/render/text.rs` `sanitize` | JSON output is escaped by `serde_json` |

## Guarantees and what proves them

| ID | Guarantee | Proof | Status |
|---|---|---|---|
| G1 | No command except `fetch` opens a socket. `fetch` is explicit, one HTTPS URL at a time; `--no-default-features` removes the HTTP client | `tests/cli.rs::the_http_client_is_only_the_fetch_feature` (ureq only as an optional dependency, named only in `src/fetch/ureq_client.rs`, no `std::net` in `src/`); CI job `no-network` builds and tests `--no-default-features` and runs `build`/`check`/`candidates`/`anchors`/`taxonomy` | Tested. The check is by crate name and by which source file may name the client. Nothing inspects what a dependency's build script does. `fetch` does not block a name that later resolves to a private address. |
| G2 | Two builds of one folder are byte-identical | `tests/cli.rs::build_writes_one_deterministic_dossier` | Tested |
| G3 | Output matches the shipped schemas | `tests/cli.rs::build_json_is_valid_against_the_shipped_schema`, `check_json_is_valid_against_the_shipped_schema`, `the_ledger_schema_is_the_shipped_v1_contract` | Tested |
| G4 | Exit codes are 0, 1 and 2; exit 1 only from `--fail-on-excluded` or `--fail-on-unpayable` | `src/error.rs:15-18`; `tests/cli.rs::check_missing_finding_is_a_usage_error`, `fail_on_excluded_exits_one_only_when_a_clause_matches`, `fail_on_unpayable_exits_one_below_the_floor`, `ledger_validate_reports_errors_and_exits_one` | Tested |
| G5 | A foreign, malformed or internally inconsistent dossier is rejected, not read as current | `tests/cli.rs::check_rejects_a_malformed_or_foreign_dossier`; `tests/adversarial.rs::a_page_number_past_the_source_page_count_is_flagged`; schema and page-range checks in `src/dossier/load.rs` | Tested |
| G6 | A PDF with no readable text is a warning, never a panic, and the build still exits 0 | `tests/cli.rs::a_pdf_without_extractable_text_is_a_warning_not_a_panic` | Tested for one compressed-stream PDF. Other malformed PDFs are not tested. |
| G7 | Quotes are text recorded from a saved file, with an in-range page number | `tests/cli.rs::check_prints_three_labelled_sections_with_a_whole_sentence_quote`; quote check in `src/check.rs`; page range in `src/dossier/load.rs`; `tests/adversarial.rs::a_hand_edited_quote_that_is_in_no_source_is_flagged`, `a_page_number_past_the_source_page_count_is_flagged` | Tested for quote-vs-recorded-text and page range. The recorded text is not re-read from the source, so a dossier that edits both is not detected (open item 6). |
| G8 | The dossier records the SHA-256 of each source | `src/dossier/sha256.rs`, checked against NIST vectors (per `docs/dossier.md`); `src/dossier/import.rs:438` | Read. The vector test is the claim in `docs/dossier.md`; not re-checked here. |
| G9 | The build writes only the dossier file, and a program id cannot name a path | `src/dossier/build.rs:45-48` (default path is `<program-dir>/<id>.dossier`); `src/dossier/import.rs:300-317` (`slug` keeps letters, digits, `.`, `-`, `_` only); `slug("../..")` is `None` at `src/dossier/import.rs:986` | Tested for `slug`. The write is tmp-then-rename (`src/dossier/build.rs:58-66`) with no test that a failed write leaves no partial file. |
| G10 | Terminal output cannot carry control characters from input | `src/render/text.rs` `sanitize`; `tests/adversarial.rs::terminal_control_characters_in_a_quote_do_not_reach_stdout` | Tested for the text and build output. JSON is escaped by `serde_json`. |
| G11 | Dependencies are checked for advisories, yanked versions, licences and duplicate versions | CI jobs `cargo-deny` and `cargo-audit`; `deny.toml` (`yanked = "deny"`, `multiple-versions = "deny"`) | Tested in CI |
| G12 | Format, lint and tests gate every change | CI `test` job runs `cargo fmt --check`, `clippy -D warnings`, `cargo test --locked --release` on three OSes | Tested in CI |
| G13 | A release archive can be checked against its digest and its build provenance | `.github/workflows/release.yml` writes `SHA256SUMS` (archives and SBOM) and attests each archive and the SBOM on tag runs; `.github/workflows/verify-release.yml` installs a release three ways and checks `SHA256SUMS` | Tested for the digest. The 0.1.2 and 0.1.3 archives carry attestations and verify with `gh attestation verify`; 0.2.0 adds one when the tag is cut. |
| G14 | A symlink, FIFO, device or directory is never read as an input, finding or dossier | `src/dossier/import.rs` `require_regular_file_from`, the `audits/` directory check and the walk's skip list; `tests/inputs.rs` (`a_symlinked_program_page_is_named_skipped_and_never_read`, `a_folder_whose_only_page_is_a_symlink_is_a_named_usage_error`, `a_symlinked_program_yaml_is_refused_by_name`, `a_symlinked_audits_directory_is_named_and_not_followed`, `a_symlinked_audit_is_named_skipped_and_never_read`, `a_symlinked_audit_subdirectory_is_named_and_not_followed`, `a_hidden_symlink_under_audits_is_named_and_not_followed`, `a_symlinked_finding_is_refused_by_name`, `a_symlinked_dossier_beside_the_finding_is_refused_not_read`, `a_fifo_program_page_is_named_and_never_opened` and `a_fifo_audit_is_named_skipped_and_never_opened` on Unix) | Tested where the OS permits creating the link/FIFO. The Windows symlink tests skip without the privilege (they print `SKIP`), and the Ubuntu CI job runs the Unix cases. The stat check and the read are separate calls: a concurrent writer can swap the path between them, so this refuses an accidental or stale link, not a writer who already controls the folder. |
| G15 | A finding over 16 MiB or a dossier over 64 MiB is refused before it is read | `src/check.rs` `MAX_FINDING_BYTES`, `src/dossier/load.rs` `MAX_DOSSIER_BYTES`; `tests/inputs.rs` (`an_oversized_sparse_finding_is_refused_before_it_is_read`, `an_oversized_sparse_dossier_is_refused_before_it_is_read`, plus the locked-file and chmod-000 tests that pin the metadata check before the read) | Tested with sparse files; the pre-read ordering is pinned by making the read itself fail distinctly |
| G16 | `--no-pdftotext` never starts poppler | `src/dossier/import.rs` `pdf_to_text_with`; `tests/inputs.rs::no_pdftotext_never_runs_poppler_and_the_control_proves_it` (a marker-writing fake pdftotext runs in the control and not with the flag) | Tested |
| G17 | A saved `.html`/`.htm` program page yields headings, bullets, table rows and `href`s, and script/style text never reaches an item | `src/dossier/html.rs` unit tests; `tests/html.rs` (list fixture, header and headerless tables, empty-link cells, a JavaScript-only page); `tests/docs.rs` generated example | Tested against the committed list fixture and synthetic tables. JavaScript-rendered text, images and non-href attributes are not read. Not a measurement of live platform pages; those are checked by hand when the fetch command changes. |
| G18 | A build of this version cannot write a dossier that `check` refuses for size | `src/dossier/build.rs` `check_dossier_body_size` and `write_dossier_with_limit`; tests `a_dossier_over_the_check_limit_is_refused_before_it_is_written` and `the_write_path_checks_the_limit_before_writing_anything` (neither the target nor the temp file is written); `tests/html.rs` and the CLI tests exercise normal builds | Tested at the limit with a one-byte-over input to the guard and a one-byte limit at the real call site; no 70 MB fixture in the suite. A pre-0.2.0 dossier over the cap is still refused by `check`. |
| G19 | `priorart-mcp` only runs read-only commands, and no client string becomes a flag | `src/mcp.rs` `argv_for` (fixed tokens, values validated, positionals after `--`); unit tests `no_tool_reaches_a_writing_or_networked_command`, `a_path_or_id_that_is_a_flag_does_not_run`; `tests/mcp_props.rs::a_tool_call_never_reaches_a_flag_slot_or_a_writing_command` (512 cases) | Tested. The tools read any file the server's user can read unless `PRIORART_MCP_ROOT` is set; the root is checked with `canonicalize` before the run, so a path swapped between the check and priorart's read is not caught. |
| G20 | `priorart-mcp` returns priorart's output unedited | `tests/mcp_stdio.rs` compares every tool's text byte for byte with a direct CLI run; unit test `stdout_is_returned_unedited_after_the_fence` | Tested. The one line the server adds (the "tool output, not instructions" fence) is a fixed prefix. |
| G21 | A hostile MCP client cannot exhaust memory or hang a run | `src/mcp.rs` caps (frame, header, argument, list, output) and `run_bounded`; `tests/mcp_bounded.rs`; `tests/mcp_stdio.rs::an_oversized_line_is_answered_and_the_server_stops`, `an_oversized_frame_is_answered_and_the_server_stops_without_serving_a_ping` | Tested. One request is served at a time, so a 120 s run blocks the session for 120 s. |

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
4. **Symlinks under `audits/` are skipped and named; hidden regular files and
   directories are skipped silently** (`src/dossier/import.rs` `collect_at_depth`).
   A skipped symlink (file or directory, hidden or not) or non-regular document
   is named in the build notes;
   `tests/inputs.rs::a_symlinked_audit_is_named_skipped_and_never_read`,
   `a_symlinked_audit_subdirectory_is_named_and_not_followed` and
   `a_hidden_symlink_under_audits_is_named_and_not_followed` cover symlinks, and
   `a_fifo_audit_is_named_skipped_and_never_opened` (Unix) the non-regular case.
   Skipping hidden regular files/directories has no dedicated test.
   *Partly checked.*
5. **Parser robustness is sampled, not measured.** The eight `fuzz/` targets and
   `tests/fuzz_replay.rs` replay committed seeds and hostile edges, and the weekly
   workflow runs libFuzzer for five minutes per target. Coverage of the YAML, JSON,
   markdown, HTML-normalizer and PDF-scrape paths is not measured, and the weekly job's crash corpus
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
   page numbers and source paths, but no test models an agent. `priorart-mcp` prefixes
   every tool text with a line saying it is tool output, not instructions; that is a
   label, not a defence. *Unverified in practice.*
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
