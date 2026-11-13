# Validation ledger

Every public claim priorart makes, the command or file that backs it, and its
status on **2026-11-13**. `validated` means a reader can re-run the evidence in
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
| The ledger validator preserves schema v1 semantics | `src/ledger/mod.rs` tests ported from bounty-economics `tests/test_validate.py`; golden files `examples/ledger.example.yaml` (1 entry) + `examples/tier-shapes.example.yaml` (5 entries) | 2026-11-13 | validated |
| The ledger schema is byte-identical to bounty-economics v0.6.1 | `schemas/ledger.schema.json`, sha256 `403324A1...` | 2026-11-13 | validated |
| `ledger validate` on the golden files prints 6 entries, 0 errors | `priorart ledger validate examples/ledger.example.yaml examples/tier-shapes.example.yaml` | 2026-11-13 | validated |
| The five tier shapes and fourteen clause classes are the shipped vocabularies | `src/vocab/mod.rs` tests assert the counts, no `any` shape, no informational floor | 2026-11-13 | validated |
| The rejection dataset keeps its row and column contract and its `summary.json` keys | `src/rejections/mod.rs` tests (113 rows, 10 columns, keys present) | 2026-11-13 | validated |
| The crate is offline: no HTTP client, no socket | `tests/cli.rs::the_crate_has_no_http_client` (Cargo.toml, Cargo.lock, every `src/` file) | 2026-11-13 | validated |
| `check` never panics on multi-byte text | `matching::tests::multi_byte_text_never_panics_in_the_negation_window` | 2026-11-13 | validated |
| Quotes are whole sentences, not fixed-length cuts | `matching::tests::quotes_are_whole_sentences_and_never_cut_inside_an_identifier` and the README fixture | 2026-11-13 | validated |
| Negations do not become preconditions or mechanism claims | `gates::tests::an_any_user_atomic_finding_trips_nothing`, `matching::tests::negation_awareness_is_local_to_the_clause` | 2026-11-13 | validated |
| The README example is generated and cannot drift | `scripts/regen-readme-examples.sh`, `tests/docs.rs::readme_example_is_generated_from_the_fixture` | 2026-11-13 | validated |
| Every documented command and exit code is exercised | `tests/cli.rs` (build, check, ledger, vocab, init, explain, rejections, rules) | 2026-11-13 | validated |

## Retracted

| Claim | Why | Date |
|---|---|---|
| Duplicate recall@5 is 0/64 | Harness locator bug: judged id `acc:001-M` compared with dossier item `acc-001-M.md` without normalising `:` to `-`. Corrected to 36/64 on that binary; later revisions measured 33/64 and 39/64. | 2026-11-13 |
| "0.1.0 ships with the prior-item warning because FAR 25.0% is below the 30% trigger" | Superseded by the final matcher: FAR 26/60 = 43.3%, above the pre-registered trigger. The release consequence is retrieval-only. The intermediate number is kept as provenance in `BENCHMARK.md`, not as the decision. | 2026-11-13 |
| The viability-gate 0-100 score and its P1-P26 prose rules | Deleted, not deprecated: 12 of 25 hard kills were paid findings. Their numbers are history, not priorart results. | 2026-11-13 |

## Not validated (do not claim these)

| Open claim | Status |
|---|---|
| The matcher generalises beyond this corpus | unvalidated: one contest pair, 60 paid+unique and 64 duplicate labels |
| The tool improves a hunter's outcomes | unvalidated: no user study |
| An "excluded", "in scope" or "payable" reading is correct for a platform | unvalidated and not claimed: the gates report evidence, never a verdict |
| Known-prior recall is a rate | unvalidated: n=1, 0 hits; reported as a count only |
| Duplicate recall is comparable to recall@5 on a prior audit | unvalidated: different relation and different denominator |
| G2 finds every precondition, or only real ones | unvalidated: lexical detector, documented as such |
| Behaviour on private reports, other platforms or non-markdown pages | unvalidated: not measured |
| Scanned PDFs are indexed | unvalidated and false: OCR is not attempted |
| The rejection dataset supports a precision or base-rate claim | unvalidated and false: one researcher's log, no positive class, no target dimension |
