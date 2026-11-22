# Validation ledger

Every public claim priorart makes, the command or file that backs it, and its
status on **2026-11-22**. `validated` means a reader can re-run the evidence in
this repository. `unvalidated` means it is a design statement or an out-of-sample
question, not a measurement. `retracted` means a previously published claim that
this release withdraws.

## Validated

| Claim | Command or file | Date | Status |
|---|---|---|---|
| Two builds of one program folder are byte-identical | `cargo test --locked`, `tests/cli.rs::build_writes_one_deterministic_dossier` | 2026-11-13 | validated |
| `build --json` conforms to `schemas/dossier.schema.json` | `tests/cli.rs::build_json_is_valid_against_the_shipped_schema` | 2026-11-13 | validated |
| `check --json` conforms to `schemas/check.schema.json` | `tests/cli.rs::check_json_is_valid_against_the_shipped_schema` | 2026-11-13 | validated |
| False-alarm rate is 26/60 = 43.3% [31.6-55.9] on 60 judged paid+unique findings | `eval/benchmark/results.json` and `eval/benchmark/BENCHMARK.md`, binary sha256 `dabfcc0c...` (matcher identical to the `2ec4c3c9...` deciding run) | 2026-11-13 | validated (published) |
| Duplicate recall@5 is 39/64 = 60.9% [48.7-71.9] on 64 judge-grouped duplicates | same files | 2026-11-13 | validated (published) |
| Label agreement is 15/20 = 75.0% [53.1-88.8] | `eval/benchmark/labels.yaml` agreement sample | 2026-11-13 | validated (published) |
| The two earlier matcher revisions measured 18/60 = 30.0% and 15/60 = 25.0% FAR | `BENCHMARK.md` three-run table | 2026-11-13 | validated (provenance) |
| The final FAR is above the plan's 30% trigger, so 0.1.0 ships retrieval-only | `CONSOLIDATION-PLAN` §4 kill criteria + `BENCHMARK.md` "Application of the plan's rule" | 2026-11-13 | validated (published) |
| 0.1.1 changes no matcher, gate or schema code, so the 0.1.0 benchmark numbers describe 0.1.1 | `git diff v0.1.0 v0.1.1 -- src/match src/gates schemas` shows only test-string and doc-comment renames | 2026-11-13 | validated |
| The ledger validator preserves schema v1 semantics | `src/ledger/mod.rs` tests ported from bounty-economics `tests/test_validate.py`; golden files `examples/ledger.example.yaml` (1 entry) + `examples/tier-shapes.example.yaml` (5 entries) | 2026-11-13 | validated |
| The ledger schema is byte-identical to bounty-economics v0.6.1 | `schemas/ledger.schema.json`, sha256 `403324A1...` | 2026-11-13 | validated |
| `ledger validate` on the golden files prints 6 entries, 0 errors | `priorart ledger validate examples/ledger.example.yaml examples/tier-shapes.example.yaml` | 2026-11-13 | validated |
| The five tier shapes and fourteen clause classes are the shipped vocabularies | `src/vocab/mod.rs` tests assert the counts, no `any` shape, no informational floor | 2026-11-13 | validated |
| The rejection dataset keeps its row and column contract and its `summary.json` keys | `src/rejections/mod.rs` tests (113 rows, 10 columns, keys present) | 2026-11-13 | validated |
| The crate is offline: no HTTP client, no socket | `tests/cli.rs::the_crate_has_no_http_client` (Cargo.toml, Cargo.lock, every `src/` file) | 2026-11-13 | validated |
| `check` never panics on multi-byte text | `matching::tests::multi_byte_text_never_panics_in_the_negation_window` | 2026-11-13 | validated |
| Quotes are whole sentences, not fixed-length cuts | `matching::tests::quotes_are_whole_sentences_and_never_cut_inside_an_identifier` and the README fixture | 2026-11-13 | validated |
| Negations do not become preconditions or mechanism claims | `gates::tests::an_any_user_atomic_finding_trips_nothing`, `matching::tests::negation_awareness_is_local_to_the_clause` | 2026-11-13 | validated |
| The README examples are generated and cannot drift | `scripts/regen-readme-examples.sh`; `tests/docs.rs` (`readme_example_is_generated_from_the_fixture`, `html_readme_example_is_generated_from_the_fixture`) | 2026-11-22 | validated |
| Every documented command and exit code is exercised | `tests/cli.rs` (build, check, ledger, vocab, init, explain, rejections, rules) | 2026-11-13 | validated |
| 0.1.2 changes no matcher, gate, schema, vocabulary or benchmark code, so the 0.1.0 benchmark numbers describe 0.1.2 | `git diff v0.1.1 v0.1.2 -- src/match src/gates schemas vocab eval` shows only test-fixture strings | 2026-11-19 | validated (historical: 0.1.0-0.1.2 only) |
| 0.1.3 re-measures the public benchmark after changing the specificity denominator: FAR 15/60 = 25.0% [15.8-37.2], duplicate recall@5 36/64 = 56.3% [44.1-67.7] | `eval/benchmark/results.json` and `eval/benchmark/BENCHMARK.md`, same seed and manifest; `docs/matching.md` | 2026-11-21 | validated (published) |
| `pdftotext` is killed after 60 s or 16 MiB of output, found on absolute `PATH` entries only, and given the PDF as an absolute path | `dossier::import::tests` (`a_pdftotext_that_runs_past_the_timeout_is_killed`, `a_pdftotext_that_floods_stdout_is_killed_at_the_cap`, `pdftotext_is_looked_up_on_absolute_path_entries_only`, `the_pdf_path_reaches_pdftotext_absolute_and_never_as_an_option`) | 2026-11-19 | validated |
| A build refuses a folder over its file-size, document-count, total-size, depth or entry limits, with exit 2 | `dossier::import::tests::input_size_is_checked_per_file_and_in_total`, `the_audits_walk_stops_at_its_depth_and_entry_limits`; `tests/cli.rs::build_refuses_*` | 2026-11-19 | validated |
| No parser panics on the seed corpus or its hostile edge cases | `tests/fuzz_replay.rs` (every CI run); `fuzz/` targets run weekly by `.github/workflows/fuzz.yml` | 2026-11-19 | validated for the corpus; libFuzzer runs start with the first weekly job |
| `build` bytes do not depend on file creation order; `check --json` is stable and schema-valid; `--top-k N` returns at most N | `tests/properties.rs` | 2026-11-19 | validated |
| Adding an item that shares no identifier with the finding never changes the top-k | `tests/properties.rs::adding_an_unrelated_item_never_changes_the_top_k` and `unrelated_items_do_not_re_rank_a_nonempty_top_k`; the share is measured over the items that share a symbol with the finding | 2026-11-21 | validated |
| A hand-edited dossier quote, or a page past its source's page count, is reported instead of trusted | `tests/adversarial.rs::a_hand_edited_quote_that_is_in_no_source_is_flagged`, `a_page_number_past_the_source_page_count_is_flagged`; `src/check.rs`, `src/dossier/load.rs` | 2026-11-21 | validated for quote-vs-recorded-text and page-range; the recorded text is not re-read from the source, so a dossier that edits both can still pass |
| ASCII contractions and "not without" are read as the finding states them | `matching::tests::ascii_apostrophe_contractions_are_negation_cues`, `not_without_is_a_requirement_not_a_denial`; `tests/adversarial.rs::a_double_negation_in_the_finding_is_read_as_the_positive_it_states` | 2026-11-21 | validated |
| `src/**/*.sol` matches `src/Vault.sol` as well as nested files | `matching::tests::a_double_star_matches_zero_or_more_directories`; `tests/properties.rs::double_star_crosses_directories_and_star_does_not` | 2026-11-21 | validated |
| Release archives and the SBOM carry build provenance attestations | `.github/workflows/release.yml` (tag runs only); check with `gh attestation verify` (docs/install.md) | 2026-11-19 | validated from 0.1.2 on; earlier releases carry none |
| A saved `scope.html` program page is converted to headings, bullets and blocks; script and style text never reaches an item; the fixture builds and answers G1/G2/G3 | `tests/html.rs` (`html_program_page_yields_payout_scope_and_exclusion_items`, `html_program_page_builds_and_answers_all_three_gates`); `dossier::html::tests` | 2026-11-22 | validated against one synthetic fixture, not against live platform pages |
| A symlink, FIFO, device or directory is never read as a program input, finding or dossier | `tests/inputs.rs` (`a_symlinked_program_page_is_named_skipped_and_never_read`, `a_folder_whose_only_page_is_a_symlink_is_a_named_usage_error`, `a_symlinked_program_yaml_is_refused_by_name`, `a_symlinked_audits_directory_is_named_and_not_followed`, `a_symlinked_finding_is_refused_by_name`, `a_symlinked_dossier_beside_the_finding_is_refused_not_read`, and `a_fifo_program_page_is_named_and_never_opened` on Unix) | 2026-11-22 | validated; the FIFO case is Unix-only and skips where `mkfifo` is unavailable |
| A finding over 16 MiB or a dossier over 64 MiB is refused by name before it is read | `tests/inputs.rs` (`an_oversized_sparse_finding_is_refused_before_it_is_read`, `an_oversized_sparse_dossier_is_refused_before_it_is_read`) | 2026-11-22 | validated with sparse files |
| `pdftotext` is native third-party code (poppler), run as the same user without a sandbox; its lookup, argument, time and output are bounded, and `--no-pdftotext` disables it | `src/dossier/import.rs` (`pdf_to_text_with`, `find_on_path`, `run_pdftotext`); `dossier::import::tests` (`pdftotext_is_looked_up_on_absolute_path_entries_only`, `the_pdf_path_reaches_pdftotext_absolute_and_never_as_an_option`, `a_pdftotext_that_runs_past_the_timeout_is_killed`, `a_pdftotext_that_floods_stdout_is_killed_at_the_cap`); `tests/inputs.rs::no_pdftotext_never_runs_poppler_and_the_control_proves_it` | 2026-11-22 | validated for the bounds and the flag; the "no sandbox" statement describes the design, it is not a containment test |
| `--no-pdftotext` never starts poppler | `tests/inputs.rs::no_pdftotext_never_runs_poppler_and_the_control_proves_it`; the control run proves the fake pdftotext is executed without the flag | 2026-11-22 | validated |
| UTF-16BE and UTF-16LE PDF literals are decoded, and a UTF-8 literal is not read as Latin-1 | `dossier::import::tests` (`a_utf16be_pdf_literal_is_decoded`, `a_utf16le_pdf_literal_is_decoded`, `a_utf8_pdf_literal_is_not_read_as_latin1`, `pdf_octal_escapes_are_decoded`) | 2026-11-22 | validated |
| The build summary prints a dossier path with forward slashes on every platform | `tests/inputs.rs::the_build_summary_prints_the_dossier_path_with_forward_slashes` | 2026-11-22 | validated |
| 0.2.0 changes no matcher, gate, schema or vocabulary code, so the 0.1.3 benchmark numbers describe it | `git diff v0.1.3 -- src/match src/gates schemas vocab eval` is empty | 2026-11-22 | validated |

## Retracted

| Claim | Why | Date |
|---|---|---|
| Duplicate recall@5 is 0/64 | Harness locator bug: judged id `acc:001-M` compared with dossier item `acc-001-M.md` without normalising `:` to `-`. Corrected to 36/64 on that binary; later revisions measured 33/64 and 39/64. | 2026-11-13 |
| "0.1.0 ships with the prior-item warning because FAR 25.0% is below the 30% trigger" | Superseded by the final matcher: FAR 26/60 = 43.3%, above the pre-registered trigger. The release consequence is retrieval-only. The intermediate number is kept as provenance in `BENCHMARK.md`, not as the decision. | 2026-11-13 |
| The viability-gate 0-100 score and its P1-P26 prose rules | Deleted, not deprecated: 12 of 25 hard kills were paid findings. Their numbers are history, not priorart results. | 2026-11-13 |
| "`scope.html` works", as stated in README, docs/dossier.md and the `init` scaffold through 0.1.3 | 0.1.3 accepted the file name but had no HTML handling: the raw markup became one `page-section`, with zero `payout-tier`, `scope-path` or `scope-clause` items, so G1/G2/G3 had nothing to answer. 0.2.0 implements the conservative converter in `src/dossier/html.rs`; the claim now carries the limits in [dossier.md](dossier.md#html-program-pages). | 2026-11-22 |

## Not validated (do not claim these)

| Open claim | Status |
|---|---|
| The matcher generalises beyond this corpus | unvalidated: one contest pair, 60 paid+unique and 64 duplicate labels |
| The tool improves a hunter's outcomes | unvalidated: no user study |
| An "excluded", "in scope" or "payable" reading is correct for a platform | unvalidated and not claimed: the gates report evidence, never a verdict |
| Known-prior recall is a rate | unvalidated: n=1, 0 hits; reported as a count only |
| Duplicate recall is comparable to recall@5 on a prior audit | unvalidated: different relation and different denominator |
| G2 finds every precondition, or only real ones | unvalidated: lexical detector, documented as such |
| Behaviour on private reports, other platforms or real-world HTML program pages | unvalidated: the HTML path is measured on one synthetic fixture, not on live platform pages |
| A dossier whose recorded text and quote were both edited is detected | unvalidated: `check` compares the quote with the item text the dossier records; it does not re-read the source bytes (see the validated row above) |
| Scanned PDFs are indexed | unvalidated and false: OCR is not attempted |
| The rejection dataset supports a precision or base-rate claim | unvalidated and false: one researcher's log, no positive class, no target dimension |
