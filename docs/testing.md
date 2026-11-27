# Testing: the test map and how to run it

Four layers, plus fuzzing and mutation testing. Commands run from the repository root.

```bash
cargo test --locked                                  # default build, includes the `fetch` feature
cargo test --locked --no-default-features            # the no-network build (no HTTP client compiled)
cargo test --lib fetch::                             # one module
cargo +nightly fuzz run html_normalize -- -max_total_time=30
```

## Layers

| Layer | Where | What it pins |
|---|---|---|
| **Unit** | `#[test]` in `src/` | Matching, gates, dossier load/import/HTML, ledger, candidates (corpus, ranker, alarm), anchors, taxonomy, rejections, renderers, `fetch` (policy loop, render, page numbering, dates), `mcp` (dispatch, argument validation, root confinement, framing, caps) |
| **Property** (proptest) | `tests/properties.rs`; `src/fetch/mod.rs` | `build` is byte-identical under any file-creation order and file-name case. `check --json` is identical across runs and validates. An unrelated item never changes the top-k. The top-k is a prefix of the larger ranking and returns at most n. Glob `**` vs `*`. **mcp** (`tests/mcp_props.rs`): any JSON gets one well-formed answer or none; no tool call puts a client string in a flag slot or reaches `fetch`/`build`/`init`/`--prepare`; `top_k`, `commit` and ids run exactly when valid; arbitrary bytes never panic the reader or the loop. **fetch:** the civil date is monotone and in range, and `check_page_path` accepts only plain files under `pages/` |
| **Adversarial** | `tests/adversarial.rs`, `tests/inputs.rs`, `tests/fetch.rs`, `tests/html.rs`, `tests/fuzz_replay.rs`; `src/fetch/mod.rs` tests; `fuzz/` (10 targets: markdown_import, pdf_scrape, html_normalize, program_yaml, known_issues_yaml, ledger_yaml, dossier_json, finding_profile, taxonomy, candidates_json) | Malformed, huge and unicode inputs; per-file and total size caps; path traversal. **SSRF and redirects:** http, file, IP literals and localhost are refused before any request; a redirect to http, to an IP literal or to a non-URL is refused; more than 5 hops is refused; the body cap; non-2xx and transport errors save nothing. **Poisoned state:** a corrupt or foreign `fetch.json` is never overwritten, and a `fetch.json` page path outside `pages/` (`../`, absolute, drive, backslash) or onto `scope.md` is refused. Escape bytes are stripped from output. The timeout is a pinned constant (30 s) applied in the HTTP client; an end-to-end timeout test would need a live socket and is not run. |
| **Integration** | `tests/cli.rs`, `tests/candidates.rs`, `tests/anchors.rs`, `tests/taxonomy.rs`, `tests/docs.rs`, `tests/mcp_stdio.rs`, `tests/mcp_bounded.rs`; `eval/dupbench/calibrate_alarm.py --self-test` | End-to-end CLI runs over fixture programs, the candidate and alarm output, the doc examples. **MCP:** the real `priorart-mcp` behind a newline client, every tool's text byte for byte against a direct CLI run, the root end to end, oversized line and frame; `run_bounded` kills a sleeping child at the deadline and refuses a 6 MiB flood |

## Gates in CI
- `ci.yml`: fmt, clippy `-D warnings`, and the suite on the default build and on `--no-default-features`.
- `fuzz.yml`: **30 s per target on every PR and integration push**, and 5 min weekly.
- `mutants.yml`: weekly and on dispatch (`files` input). See [`test-quality.md`](test-quality.md).
- Coverage is measured by hand or on dispatch, and CI does not fail on a coverage figure (see
  `test-quality.md`).

## Counts, before and after P2R-2
`#[test]` functions declared in the tree. The proptest functions are a subset of them.

| | `main` (0.2.2) | `p2/integration` + this branch |
|---|---:|---:|
| Unit (`src/`) | 94 | 263 |
| Integration-level files (`tests/`) | 90 | 142 |
| Property functions | 7 | 9 |
| Fuzz targets in the CI matrix | 8 | 10 |
| Executed, `--no-default-features` | n/a | 396 passed, 0 failed, 0 ignored |

Coverage (cargo llvm-cov, regions / functions / lines) was 86.65 / 86.27 / 85.88% at `f486d3a`, and
86.42 / 86.03 / 85.84% after #40 added `fetch` (`7340dc4`). The figure for this branch, with the
`fetch` tests added, is in the PR body.

The unit count includes 26 tests added by the mutation follow-up (`mutants-2026-11-27.md`): the 370 executed
tests at `6cb50d3` became 396 at the follow-up commit.

The MCP server (`p2/pa-mcp`, on top of the follow-up) adds 72 `#[test]` functions, 71 of them run on Windows: 39 unit tests in `src/mcp.rs` (one is Unix-only), 11 proptest
properties at 512 cases (`tests/mcp_props.rs`), 7 in `tests/mcp_bounded.rs` and 15 in `tests/mcp_stdio.rs`. Executed on Windows with
`--no-default-features`: 467 passed, 0 failed, 0 ignored.
