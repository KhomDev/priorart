# Test quality: mutation testing and coverage

Measured on 2026-11-19, on `main` at commit `66d0ec8`, on Windows 11 with
Rust 1.98.0. These numbers describe the test suite as it was on that commit.
They are a measurement, not a gate: CI does not run either tool.

## Commands

```text
cargo install cargo-mutants --version 27.1.0 --locked
cargo mutants -f 'src/match/**/*.rs' -f 'src/gates/**/*.rs' -f 'src/dossier/**/*.rs' \
    -j 5 --cargo-arg=--locked --timeout 300

rustup component add llvm-tools-preview
cargo install cargo-llvm-cov --version 0.9.1 --locked
cargo llvm-cov --locked --summary-only
```

The mutation run took 54 minutes of wall-clock time: 571 mutants, each built and
tested in about 28 s, five at a time. The coverage run took about two minutes.

## Mutation results

| Outcome | Count |
|---|---|
| caught | 389 |
| missed (survived) | 158 |
| timeout | 6 |
| unviable (does not compile) | 18 |
| **total** | **571** |

**Survival rate: 28.6%** = missed / (caught + missed + timeout) = 158 / 553.
Unviable mutants are excluded from the rate. A timeout counts as a kill, because
the mutant changed behaviour enough to stop the test run.

By module, the mutants are spread over `src/match/mod.rs` (215), `src/dossier/import.rs`
(214), `src/gates/mod.rs` (68), `src/dossier/sha256.rs` (64), `src/dossier/load.rs` (8)
and `src/dossier/build.rs` (2). `src/dossier/sha256.rs` has no survivor, which
is what its NIST vector test should give.

The aim is not zero survivors. A survivor is a behaviour that no test pins down.
The ten below are the ones that matter most for what priorart reports.

## Scheduled runs

Since 2026-11-22, `.github/workflows/mutants.yml` runs the same `cargo mutants`
command every Monday and uploads `mutants.out/`. It is not a gate: a survivor
is a test to write, not a failed build. The numbers above remain the
2026-11-19 measurement on commit `66d0ec8` until a scheduled run on the release
commit completes and is recorded here.

## The ten most important surviving mutants

Line numbers are for commit `66d0ec8`.

1. **`src/gates/mod.rs:419`**, `>=` replaced by `<` in `floor_of`. The payout gate
   takes the higher of the tier shape's implied floor and the declared floor. No test
   has the two disagree, so the wrong floor would pass unnoticed. This decides whether
   a claimed severity reads as payable (G1).
2. **`src/gates/mod.rs:161`**, match arm `"human_error"` deleted in
   `Preconditions::trigger`. The `human_error` exclusion class is never flagged in a
   test, so an exclusion clause of that class could be dropped from the evidence (G2).
3. **`src/gates/mod.rs:174`**, match arm `"front_run_only"` deleted. The front-running
   exclusion class is not tested, so a clause that the finding trips can disappear from G2.
4. **`src/gates/mod.rs:162`**, `||` replaced by `&&`, and `!` deleted, in the
   `human_error` guard. The guard decides whether a finding that names a role is read as
   admin error or as a code defect. Both mutants survive.
5. **`src/gates/mod.rs:436`**, `||` replaced by `&&`, and `==` by `!=`, in
   `prior_audit_coverage`. This is G4's claim about which cited paths a named prior audit
   covers. A wrong guard would report coverage that is not there, or hide coverage that is.
6. **`src/gates/mod.rs:508`**, `==` replaced by `!=` in `prior_art`. Ranked items are
   resolved to their source by id. Inverting the comparison attributes each quote to the
   wrong source, and no test checks the attribution.
7. **`src/match/mod.rs:214` and `:218`**, the match arms `"critical" | "crit"` and
   `"informational" | "info" | "none"` in `Severity::parse` are deleted. A severity label
   the parser no longer knows becomes `None`, and the payout gate then reports `null`
   rather than a verdict. Nothing tests those spellings.
8. **`src/match/mod.rs:354` and `:355`**, `||` replaced by `&&`, and `<` by `<=` or `==`,
   in `classify_ident`. This decides which tokens count as strong identifiers. A change
   here changes which items can rank at all, and the matcher's tests do not pin the
   boundaries.
9. **`src/match/mod.rs:484` and `:485`**, `>` replaced by `>=` or `==`, and `||` by `&&`,
   in `negated_near`. The negation window decides whether a precondition word such as
   "no role" is negated. Moving the boundary by one changes which exclusion clauses fire.
10. **`src/match/mod.rs:740`**, `-` replaced by `+` or `/` in the `rank` rarity term. The
    rarity score breaks ties in the top-k order. Survival means the order of tied items
    is not pinned by a test.

Each of 1 to 10 is a missing test, not a known bug. Adding a test that fails on the
mutant kills it, and the test should be written first.

## Coverage

`cargo llvm-cov --summary-only`, whole crate, lib and integration tests:

| Measure | Total |
|---|---|
| Lines | **87.34%** (4422 lines, 560 missed) |
| Functions | 85.28% |
| Regions | 88.06% |

The least-covered modules, by line coverage:

| Module | Line coverage |
|---|---|
| `src/dossier/load.rs` | 59.62% |
| `src/ledger/mod.rs` | 82.02% |
| `src/dossier/import.rs` | 82.89% |
| `src/gates/mod.rs` | 83.01% |
| `src/check.rs` | 85.26% |
| `src/render/json.rs` | 85.71% |

`src/dossier/load.rs` is the least covered and has the most to lose: it is the
single gate that refuses a foreign or stale dossier. Its eight mutants are all caught,
so the uncovered lines are the paths the tests never reach, not the ones they test
badly. `src/gates/mod.rs` is the module where coverage and survival both point at
the same gap.

## Not done, and why

- **Not gated in CI.** The numbers move with each change, and a threshold would
  reward tests written to the metric.
- **Only `src/match`, `src/gates` and `src/dossier` were mutated**, as the plan asked.
  `src/ledger`, `src/render`, `src/check.rs` and `src/cli.rs` were not mutated, though
  their coverage is shown above.
- **Measured before the adversarial, fuzz and property suites.** Those suites are in
  open pull requests, and their effect on these numbers is not measured here.
- **No timing budget per mutant.** Each mutant had a 300 s timeout; six hit it.
  They are listed in `mutants.out/timeout.txt` of the run and are not counted as
  survivors.
