# Contributing

The most valuable contribution is **a case where priorart was wrong, with the
file and the re-runnable command**. The second is a documentation fix where the
docs and the binary disagree. Both are more useful than a new feature.

## Ground rules

1. **No target names.** No program, platform, chain, protocol or repository name
   in a rule, a pattern, a docstring, an example, a fixture, a commit message or
   an issue. The public benchmark stores pointers and labels only
   (`repo@commit` plus a path), never finding text. A doc example that names a
   real program is a disclosure.
2. **No invented numbers.** Every figure in a doc must come from a command or a
   committed file, with the command recorded next to it, or be labelled "not
   measured". The benchmark's numbers live in `eval/benchmark/results.json`;
   `docs/benchmark.md` cites it and does not round it.
3. **Never weaken a test to make it pass.** If a matcher change breaks a fixture,
   either the change is wrong or the fixture is wrong. Decide which, and say
   which in the commit message.
4. **No private-workspace strings.** No local absolute paths, no private
   classifier output, no blocklist tokens. The release checklist scans for them.
5. **Do not add data.** The vocabularies and the rejection dataset move only
   through the two-stage export from their upstream repositories. Never vendor a
   report corpus, a program list or a reward table; the benchmark fetches by
   pointer into a gitignored cache.
6. **The crate stays offline.** No HTTP client crate, no socket, no update check.
   `tests/cli.rs::the_crate_has_no_http_client` enforces it. Network is allowed
   only in `eval/` fetch scripts, with pointers committed.

## Setup

```sh
git clone https://github.com/KhomDev/priorart
cd priorart
cargo build            # Rust 1.89 or newer
cargo test --locked    # unit + integration tests, offline
python scripts/check_links.py   # every relative doc link resolves
```

The only Python needed for the repo checks is Python 3 (standard library).
`eval/benchmark/fetch.py` needs its own dependencies; it is not part of the
crate and it is the only component that touches the network.

## Before you open a pull request

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked
python scripts/check_links.py
```

CI runs the same on Windows and Linux, plus an MSRV build and `cargo deny`.
Running them locally is faster than reading a red build.

## Changing the matcher or the gates

A matcher change is a claim about retrieval, so it needs more than a green test:

1. Add or update a fixture in `tests/` for the behaviour you changed.
2. Re-run the public benchmark (`python eval/benchmark/run.py --priorart
   target/debug/priorart --split all`) and update `docs/benchmark.md` **and**
   `docs/validation.md` with the new n, intervals and the binary hash. If the
   numbers did not move, say so.
3. If the rule changed meaning, update `docs/matching.md` or `docs/gates.md`
   in the same pull request.
4. Do not tune on the held-out split. `run.py` refuses a held-out run without a
   committed `freeze.json`, and the reason is in `docs/benchmark.md`.

A change to the 5 tier shapes or the 14 clause classes starts upstream in
bounty-economics and is moved in with its provenance and byte-identity check. Do
not edit `vocab/*.yaml` in place without updating `docs/vocab.md`.

## Documentation rules

- `README.md`'s generated example block is written by
  `scripts/regen-readme-examples.sh` and checked by `tests/docs.rs`. Do not
  hand-edit it; run the script and commit the result.
- Every relative link must resolve. `python scripts/check_links.py` fails on a
  missing target, an absolute host path or a link that escapes the repository.
- Every command in the docs must have been run at least once. If you change a
  flag, change `docs/cli.md` and the test that exercises it.
- A number in a doc needs a source: `results.json`, a committed file, or a
  command in the same section.

## Releasing

A release is a claim, so the checklist is part of the change:

1. **Version.** Bump `Cargo.toml` and `Cargo.lock` to the version being
   released. `priorart --version` must print it.
2. **Changelog.** Write the `CHANGELOG.md` entry under a new `## [X.Y.Z]`
   heading. Every number in it must come from a command a reader can re-run.
3. **Benchmark.** Refresh the public benchmark, or state in the changelog why the
   numbers did not change. If a metric moved, update `docs/benchmark.md` and the
   matching rows in `docs/validation.md` with n and the Wilson interval.
4. **Validation ledger.** Update [docs/validation.md](docs/validation.md): add
   rows for the claims this release makes, and mark anything it retracts.
5. **Offline check.** `cargo tree` must show no HTTP client;
   `tests/cli.rs::the_crate_has_no_http_client` must pass. Read
   [docs/no-network.md](docs/no-network.md) if you are adding a dependency.
6. **Private-data scans, over the full history.** These tools live in the
   private/upstream rejection-taxonomy tooling, not in this repository. Run both
   against priorart before any push and every release:
   ```sh
   python <tools>/check_hygiene.py --private-dir <private-dir> --json
   python <tools>/verify_blocklist.py --source <private-log> \
     --blocklist <private-blocklist.yaml> --scan-repo --repo ../priorart
   ```
   `check_hygiene.py` scans every tracked file **and every commit in priorart's
   full history** unless you pass `--no-history`; a clean working tree is not
   enough. `verify_blocklist.py --scan-repo` cross-checks the tracked files
   against the private source. Both must pass. If either fails, do not tag.
7. **Full local suite.** The four commands in "Before you open a pull request",
   plus `cargo test --locked --release` if the release changes codegen.
8. **Tag and publish.** Commit on `main`, then create an **annotated** tag
   `vX.Y.Z` on that commit and push it. The release workflow refuses a tag that
   disagrees with `Cargo.toml` and runs the test suite before it builds anything.
   The maintainer runs `cargo publish`. **Never move or delete a published
   version tag, and never yank a published crate.** A bad release gets a new
   patch version. The `viability-gate` crate is frozen at 0.9.0 for exactly this
   reason: it still installs, it prints a deprecation notice, and it is not
   yanked.

## Commit messages

Conventional commits, one logical change each:

```text
feat(matcher): rank ties by rarity before shared count
fix(dossier): do not skip a PDF whose scrape found no letters
docs(gates): state what null means for each gate
chore(deps): bump serde-saphyr
```

The subject line says what changed. The body says what was measured, and with
which command, if the change moves a number.

## Review

A change is reviewed on evidence, in this order:

1. Does it come with a case or a fixture, and is the case described without
   naming a target?
2. Does it move a number? If so, is the new number from a re-runnable command?
3. Does it weaken a test, hide an "unknown", or turn a quote into a verdict?
4. Do the docs and the binary still say the same thing?

## Licensing

By opening a pull request you agree to license your contribution under the MIT
licence, matching the repository code. The rejection dataset stays CC BY 4.0.
See [docs/licensing.md](docs/licensing.md).
