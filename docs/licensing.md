# Licensing

## Current state

| Path | Contents | Licence |
|---|---|---|
| `src/**` | Rust source | **MIT** |
| `tests/**` | Test code and fixtures | **MIT** |
| `scripts/**` | Python tooling | **MIT** |
| `tools/**` | The calibration harness | **MIT** |
| `rules/anti-patterns.yaml` | The anti-pattern catalog (data) | **MIT** |
| `rules/gates.yaml` | The gate definitions (data) | **MIT** |
| `rules/packs/**` | Ecosystem token packs (data) | **MIT** |
| `docs/**`, `README.md` | Documentation | **MIT** |
| `examples/**` | Synthetic fixtures | **MIT** |
| `eval/sources.yaml`, `eval/dataset.manifest.yaml`, `eval/results/**` | URLs, label facts, and aggregate scores. Not the finding texts | **MIT** |
| `eval/cache/**` | Downloaded contest bodies, gitignored, not part of this repository | not ours |

The repository `LICENSE` is the MIT licence and it is the only licence text in
the tree. There is no per-directory licence file and no `SPDX-License-Identifier`
header anywhere.

## `rules/*.yaml` is covered (resolved in 0.3.0)

Up to 0.2.0 this was an open question: the catalog is data rather than code, and
a reader who wanted to reuse it could reasonably ask whether a licence that reads
as a software licence was meant to cover a data file.

It was. The **Scope** paragraph in `README.md`'s License section now says so: the
MIT grant covers the entire repository, including `rules/` and `docs/`, and the
catalog may be extracted, shipped as a ruleset in another tool, or adapted under
the same terms. The catalog is the author's original work, derived from the
author's own rejection log. The classified log is also published separately in
[rejection-taxonomy](https://github.com/KhomDev/rejection-taxonomy). A snapshot
of that repository at tag v0.6.0 is vendored at
[`data/rejection-taxonomy/`](../data/rejection-taxonomy/). Its code is MIT
([LICENSE](../data/rejection-taxonomy/LICENSE)) and its dataset and prose are
CC BY 4.0 ([LICENSE-DATA](../data/rejection-taxonomy/LICENSE-DATA)). Credit
that repository when you share the dataset.

A snapshot of [bounty-economics](https://github.com/KhomDev/bounty-economics)
v0.5.0 is vendored at [`data/bounty-economics/`](../data/bounty-economics/)
under its MIT licence ([LICENSE](../data/bounty-economics/LICENSE)). Credit
that repository when you share the schema or the taxonomies. Neither snapshot
changes the terms of the source repository.

## What is deliberately not bundled

The tool ships **no** third-party data:

- No program names, no chain names, no reward tables.
- The rejection log is vendored as a snapshot, under that repository's own
  licences, at `data/rejection-taxonomy/`. It is not rewritten into the MIT
  grant above.
- No prior-audit PDFs and no known-issues corpora. Those are user inputs
  (`--known-issues`), supplied at runtime and never redistributed.
- No control set of report text. [`docs/benchmark.md`](benchmark.md) records a
  measurement against public contest issues. The bodies are downloaded by
  `eval/run.py` into a gitignored cache and are not part of this repository.
  [`docs/control-set.md`](control-set.md) is still how to measure the author's
  own paid findings, which do not exist in the log.

That is why there is no `NOTICE` file and no third-party licence inventory: the
only third-party code is the crate graph, and `cargo` resolves and reports those
licences directly (`cargo deny check licenses` in CI).

## Contributions

By opening a pull request you agree to license your contribution under the MIT
licence, matching the repository. See [`CONTRIBUTING.md`](../CONTRIBUTING.md).
