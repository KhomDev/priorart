# Contributing

The most valuable contribution to this repository is **a rule with a real cited
example and both test cases**. The second most valuable is a report that one of
the published numbers is wrong.

## Ground rules

1. **No target names.** No program, platform, chain, repository or protocol name
   anywhere — not in a rule, a pattern, a docstring, an example, a test fixture,
   a commit message or an issue. A rule that names a target is a disclosure, and
   the test suite enforces this. Describe a mechanism by the role its symbols
   play ("an effective-leverage calculation"), never by its identifier.
2. **No invented numbers.** Every figure in the docs must come from a command
   someone ran, with the command recorded next to it, or be labelled "not yet
   measured". If you cannot measure it, say so; the repository has several
   `PENDING` and `UNMEASURED` markers and they are deliberate.
3. **Never weaken a test to make it pass.** If a rule change breaks a fixture,
   either the change is wrong or the fixture is wrong. Decide which, and say
   which in the commit message.
4. **No private-workspace strings.** `python scripts/check_tripwires.py` must
   pass. It runs in CI.
5. **Do not add data.** The tool ships rules, not data. Do not vendor a report
   corpus, a program list, a reward table or a control set.

## Setup

```bash
git clone https://github.com/KhomDev/viability-gate
cd viability-gate
cargo build --release
python -m pip install pyyaml
```

## Before you open a pull request

```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test --release

python scripts/check_tripwires.py
python scripts/gen_rule_docs.py --check     # or run it without --check to regenerate
python scripts/check_readme.py
python tools/calibrate/calibrate.py --log examples/calibration/labels.csv \
  --findings-dir examples/calibration/findings --vg target/release/vg \
  --expect examples/calibration/expected.json
```

CI runs all of these, plus `cargo deny check`, `cargo audit`, an MSRV build and
an OS matrix. Running them locally first is faster than reading a red build.

## Releasing

A release is a claim, so the checklist is part of the change, not an afterthought:

1. Bump `Cargo.toml` (and `Cargo.lock`) to the version being released.
2. Write the `CHANGELOG.md` entry. Every number in it must come from a command
   that can be re-run; link the file that holds the result.
3. Refresh the benchmark, or state in the changelog why the numbers did not
   change. If a rule's measured precision moved, update `docs/benchmark.md`.
4. Update `docs/validation.md`: add or change the rows for the claims this
   release makes, and mark anything the release retracts.
5. Run the full local suite above, including `scripts/check_readme.py` and the
   generated-docs check.
6. Commit on `main`, then create an **annotated** tag `vX.Y.Z` on that commit
   and push it. The release workflow refuses a tag that disagrees with
   `Cargo.toml` and runs the test suite before it builds anything.
7. Never move or delete a published version tag. A bad release gets a new patch
   version.

## Adding or changing a rule

The catalog is [`rules/anti-patterns.yaml`](rules/anti-patterns.yaml). A rule is:

```yaml
- id: P17
  title: No attacker-win scenario
  group: E            # A..E, see the file header
  verdict: soft       # hard = KILL with --strict (ROUTE by default), soft = HOLD, warn = note only
  weight: 30          # subtracted from the 100-point estimate
  any_of: ["no (fund loss|attacker profit|extraction)"]
  none_of: ["attacker (profits?|gains?) \\d"]   # escape hatch suppresses the rule
  fix: "Identify the beneficiary and quantify the loss, or downgrade."
  doc: "docs/rules/P17.md"
```

A rule change needs, in the same pull request:

1. **A positive fixture** in [`tests/rules.fixture.yaml`](tests/rules.fixture.yaml)
   — a synthetic finding the rule must fire on.
2. **A negative fixture** — the closest synthetic finding it must *not* fire on.
   This is the half that decides whether a rule survives contact with real
   findings.
3. **A page annotation** in [`docs/rules/_annotations.yaml`](docs/rules/_annotations.yaml)
   — intent, escape hatches, known false-positive modes, both examples, and a
   changelog entry.
4. **Regenerated pages**: `python scripts/gen_rule_docs.py`.

If the change moves the calibration fixture's numbers, regenerate
`examples/calibration/expected.json` **and say in the pull request what moved and
why**. A silent regeneration is the failure mode that file exists to prevent.

If the change moves a number quoted in `README.md` or `docs/calibration.md`,
update it in the same commit. `python scripts/check_readme.py` checks the ones
that can be checked mechanically.

## Adding an ecosystem pack

Ecosystem tokens live in [`rules/packs/`](rules/packs/), never in the catalog.
The pack set must stay balanced — at least seven packs, and no pack with more
than twice the entries of the smallest — because an unbalanced set is a statement
about which ecosystem the author hunts. See
[`rules/packs/README.md`](rules/packs/README.md).

## Commit messages

Conventional commits, one logical change each:

```text
feat(rules): add P23 for ...
fix(engine): stop P18 firing on negated statements
docs(calibration): state both denominators for the corpus-only class
chore(deps): bump serde-saphyr
```

The subject line is a complete sentence about what changed. The body says what
was measured, and with which command, if the change moves a number.

## Pull request review

A rule change is reviewed on evidence, in this order:

1. Does the example exist, and is it described without naming a target?
2. Does the negative fixture make sense — is it the *closest* passing case, not an
   easy one?
3. Are the known false-positive modes honest?
4. Does anything else in the docs need to move?

Expect a rule with no cited example to be closed. That is not unfriendliness; it
is the only bar that keeps the catalog from filling with plausible patterns
nobody has seen fire.

## Licensing

By opening a pull request you agree to license your contribution under the MIT
licence, matching the repository. See [`docs/licensing.md`](docs/licensing.md).
