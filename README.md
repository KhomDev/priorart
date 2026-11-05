# viability-gate

**Score a draft audit finding against 22 documented rejection anti-patterns and 7 pre-hunt gates,
before you spend a day writing a PoC for something that cannot be paid.**

Offline. Deterministic. No API key. Single binary. No network code at all - see
[docs/no-network.md](docs/no-network.md) for the cargo-tree evidence.

**Ships rules, not data.** The tool carries no program names, no chains, no reward tables. You
point it at *your own* private ledger of the programs you hunt.

```bash
vg check finding.md
```

> **Name collision.** `vg` is also the name of an unrelated bioinformatics tool. This crate ships
> **both** a `vg` and a `viability-gate` binary, so you can install and call whichever does not
> collide on your PATH.

## What it does, in one example

This is real output. `scripts/regen-readme-examples.sh` produces it by running the binary, and CI
fails if the README has drifted from it.

<!-- BEGIN:example -->

```text
viability-gate 0.4.4  —  finding.md

  OUTCOME   KILL
            A hard blocker fired. Do not submit as written.
  SCORE     60 / 100   (uncalibrated heuristic; not a probability; threshold 50)

  HARD  P2   Privileged-role-gated against a program exclusion
             ↳ matched: admin can
             ↳ in sentence: An admin can set the pause flag to zero, which causes every vault operation to
             → Check whether an unprivileged attacker can trigger the bug without role
             → compromise. If not: hold. The one escape is UNKNOWING admin harm - a
             → reasonable admin action with a non-obvious destructive second-order
             → effect.
             (docs/rules/P2.md; evidence: in-sample; origin rows: 3)

  PASS  P1, P6, P12, P13, P3, P5, P11, P20, P7, P16, P18, P21, P22, P15, P4, P9, P8, P10, P14, P17, P19
  SKIP  G1 (no ledger supplied)   G2 (no ledger supplied)   G3 (no scope tree supplied)   G4 (no prior-audit surface supplied)   G5 (no test suite supplied)   G6 (fork status unknown)   G7 (no own-history log supplied)   D1 (D1 is a corpus match)

  Gates:  G1 ?   G2 ?   G3 ?   G4 ?   G5 ?   G6 ?   G7 ?   D1 ?

  ────────────────────────────────────────────────────────────
  Advisory, never authoritative. This tool produces an outcome with reasons;
  it does not decide, and it cannot read your target's live rules.
  Over-filtering is its own failure mode - the catalog is a prioritiser, not
  a kill-switch. A triage rejection is not a final verdict. CLEAR means only
  that no known blocker was found by this tool.
```

<!-- END:example -->

Note three things about that output:

1. **Every gate reports `?`, not a pass.** Nothing but the finding was supplied, so none of them
   could run. Unknown is not pass: a gate you could not run is a gate you have not run. Note that
   `D1` is always `?` under `vg check` — it is driven by `--known-issues`, not by the target
   inputs — which is why a bare `vg check` can never return `CLEAR`.
2. **Every fire shows the matched snippet and the sentence it came from**, so you can argue with it.
3. **Gates do NOT subtract from the score.** The score is 100 minus the fired rules' weights, and
   nothing else. The previous README claimed `0%` for a finding that trips P2 (40) and P19 (50),
   which computes to 10. That example was simply stale; this one is generated.

## Measured (in-sample)

I calibrated this against my own rejection log: **59 of 97 logged rejections** matched to their
original finding text at high confidence, then scored on whether the tool flagged the same gate
the program actually closed it on.

> **Dated note (2026-10-28).** This recall study was measured on the **original 97** logged
> rejections. The published dataset has since grown to **113 rows** (rejection-taxonomy v0.5.0),
> and the recall study has **not** been re-run against the 16 added rows. Every `59/97`, `14/59`,
> `45/59`, `48/59` and related figure in this section and in
> [docs/calibration.md](docs/calibration.md) is therefore the original in-sample measurement,
> unchanged. Only the reachable-automation counts under [Related](#related) track the current
> 113-row dataset.

| Scoring | Result |
|---|---:|
| **Strict** - flagged the gate that actually decided it | **14/59 (23.7%)** |
| **Lenient** - flagged *any* gate-mapped reason to stop | **45/59 (76.3%)** |
| Miss - no gate-mapped reason at all | 14/59 |

**The 52-point gap is the honest summary.** This tool is good at telling you to stop and bad at
telling you why. Reporting either number alone would misrepresent it.

**These are in-sample, and that caveat comes first, not last.** The 22 anti-patterns were derived
from the patterns in this same log, so the tool was built knowing these outcomes. 23.7% is closer
to a ceiling than to an unbiased estimate. The matched 59 are also a subset: their gate mix may
differ from all 97 rows. See [docs/calibration.md](docs/calibration.md).

Split by whether the catalog *can* reach the gate at all:

| Class | Pairs | Hit rate |
|---|---:|---:|
| Gates the catalog can reach | 26 | **53.8%** |
| Duplicate / prior-audit (needs an external corpus) | 28 | 0% |
| Mixed or unmapped | 5 | 0% |

**"0%" does not mean silence**: of those 28 pairs, the tool still fired a gate-mapped rule on 20.
It found *a* reason to stop almost every time; it never found the duplicate.

### The fix for that was tested, and it failed

`--known-issues` supplies the missing corpus. I tested it against the 28 pairs it was meant to fix:

| Pattern | Recall | Cross-fire |
|---|---:|---:|
| 1 term | 13/28 (46%) | **77% of fires are the wrong finding** |
| 2 terms, proximity | 3-4/28 (11-14%) | 40-50% |

No useful operating point. Mean vocabulary overlap between a rejection description and the bug
report it describes is Jaccard 0.024 and containment 0.61.

The test is unfair to the approach in three specific ways (it used rejection summaries rather
than the program's own wording, auto-generated patterns, and a rarest-term heuristic), so the
verdict is **not validated** rather than disproven. **Treat `--known-issues` as a hypothesis
with a stated test, not a demonstrated fix.**

## What it checks

### The pre-hunt gates

Run these against a *target and program*, before hunting.

| Gate | Question | What it needs | Evidence |
|---|---|---|---|
| **G1 - Severity floor** | Is there a payout tier at the severity I can reach? | program reward table | 5 rows |
| **G2 - Exclusions** | Does a written exclusion clause cover my bug class? | program scope text | 20 rows |
| **G3 - Scope tree** | Is the root cause inside the scoped files? | scope file list | 4 rows |
| **G4 - Prior audits** | Is this surface already closed? | prior-audit links/PDFs | 7 rows |
| **G5 - Test intent** | Does the project's own test suite assert this is intended? | target test suite | 1 row |
| **G6 - Fork novelty** | Is this a fork whose behaviour is documented upstream? | target provenance | **none** |
| **G7 - Own history** | Have I already submitted this bug family? | your own log | 12 rows |

Plus **D1 - known-issues / duplicate**, implemented by `--known-issues` because its input is a
corpus rather than a check over the target.

**Which gates have supporting data.** G1-G5 and G7 are derived from rows in my own rejection log;
the counts above are those rows. **G6 has no supporting rows at all** - my log contains no rejection
attributed to fork provenance. It is kept because the failure mode is real and the check is cheap,
but it is a judgement, not a measurement, and `vg gates` prints the evidence class next to every
gate. Every count here is **in-sample**.

### The anti-pattern catalog

Run these against a *draft finding*, before submitting. The full list is generated into
[docs/rules/](docs/rules/) from the rules file, one page per rule.

| ID | Anti-pattern | Verdict |
|---|---|---|
| P1 | Admitted non-exploit | hard (note on a bare hedge) |
| P2 | Privileged-role-gated against a program exclusion | hard |
| P3 | Front-run-only or operator-delay dependency | hard |
| P4 | Defective PoC | **warn** (hard only when the report says the PoC is not the real thing) |
| P5 | Out-of-scope file primitive | hard (ignores PoC-section paths) |
| P6 | Theoretical future-upgrade framing filed at Critical | hard |
| P7 | Best-practice critique | soft |
| P8 | Self-contradiction | soft |
| P9 | Wrong line numbers or unverified paths | hard |
| P10 | Hypothetical TVL-scaling tables | warn |
| P11 | Testnet-only finding at Critical tier | hard |
| P12 | Panic or revert without persisted impact | soft |
| P13 | "Inconsistent state" without a quantified delta | soft |
| P14 | Triager-simulation table in the report | hard |
| P15 | Intended behaviour asserted in the project's own tests | hard (note on bare "documented behaviour") |
| P16 | Severity mismatch with the program payout floor | hard |
| P17 | No attacker-win scenario | soft |
| P18 | Known characteristic documented in a prior audit | hard |
| P19 | Headline impact contradicted by PoC evidence | hard, **structural** |
| P20 | Requires a cooperating party | soft |
| P21 | Realistic-configuration mismatch | soft |
| P22 | Primitive-vs-product confusion | soft |

There are **22 entries, P1-P22**. The rule loader is the authority; this table is a summary and
[docs/rules/](docs/rules/) is generated from it.

### How a rule fires

A document is split into **sections** at markdown headings and each section into **sentences**.
A pattern is evaluated sentence by sentence, and a match is discarded when a negation cue appears
within the **6 tokens immediately before it** (`no`, `not`, `never`, `without`, `n't`, ...).

That window is why *"the prior audit missed this path"* no longer trips P18, and *"the
implementation contradicts the documented behaviour"* no longer trips P15 - those are the strongest
forms of the argument, and the old rules killed them.

Two escape hatches exist:

- `none_of` - a pattern that suppresses the rule. Scoped to the **whole document** by default, or
  to the **matching sentence** when the rule sets `none_of_scope: sentence`. P2 uses the sentence
  scope, because a `permissionless` in an unrelated sentence should not switch off a
  privileged-role match.
- `downgrade_on` - weak forms that are dropped entirely. This is how P4 stops firing on ordinary
  mock scaffolding.

Every fire prints the matched snippet **and its sentence**.

## Verdict and score semantics

One decision rule, applied in this order:

| # | Condition | Outcome |
|---|---|---|
| 1 | any fired rule is `hard` | **KILL** |
| 2 | any fired rule is `soft`, or the score is under the threshold | **HOLD** |
| 3 | any fired rule is `warn` | **NOTE** |
| 4 | any gate is `?` | **INCOMPLETE** |
| 5 | otherwise | **CLEAR** |

**There is no `SUBMIT`.** The tool cannot tell you to submit: it has no positive evidence, only the
absence of a blocker it knows how to look for. An earlier README said *"The tool never says 'submit'.
It says SUBMIT / HOLD / KILL."* - which contradicted itself, and `SUBMIT` is gone.

**`CLEAR` means only "no known blocker found by this tool".** It is not a green light. In
particular, if any gate is `?` and nothing fired, the outcome is `INCOMPLETE`, never `CLEAR`.

### The score

`viability_score` is **100 minus the hand-set weights of the fired rules**, clamped to 0-100.

> **It is an uncalibrated heuristic. It is not a probability, and it is not "payout odds".**

It was previously called "estimated payout odds", which claimed far more than it does. Gates do not
subtract from it - a gate that did not run produces `INCOMPLETE` instead, which is more honest than
inventing a number for it. `--min-score` remains available as an optional CI threshold.

### Removed JSON fields

0.2.0 carried the pre-0.2.0 field names for one minor version, as announced. 0.3.0 removes them,
along with the `deprecated_fields` list that named them:

| Removed in 0.3.0 | Use instead |
|---|---|
| `odds` | `viability_score` (same value) |
| `verdict` | `outcome` (the outcome word, a superset of the old vocabulary) |

## Usage

```bash
# scaffold the inputs a new target needs (start here)
vg init my-target/

# score a draft finding
vg check finding.md

# run the pre-hunt gates against a target + your private program ledger
vg gates --ledger my-ledger.yaml --scope scope.txt --log my-target/history.yaml

# machine-readable, for CI or an agent pipeline
vg check finding.md --json

# check against the program's known issues and prior-audit findings
vg check finding.md --known-issues my-known-issues.yaml

# check a draft against your OWN submission history (gate G7)
vg log check finding.md

# explain a single anti-pattern, with its detection signal and provenance
vg explain P17

# list the catalog
vg rules

# measure the catalog against a labelled set
vg bench --rejected rejected/ --accepted accepted/ --labels labels.csv
```

### `--known-issues` - the known-issue matcher (unvalidated)

Duplicates are the largest single rejection category and the one no pattern over your finding's
prose can find, because the signal lives in an **external corpus**. This flag is that corpus:

```yaml
known_issues:
  - id: KI-1
    summary: "Rounding in share calculation favours the protocol; accepted as intended"
    source: "https://example.invalid/audit-2025.pdf, section 3.2"
    patterns:
      - "round(ing|s)? .{0,40}(favou?rs?|benefits?) the protocol"
      - "dust.{0,30}(stuck|stranded|unclaimable)"
```

A match is a **hard fail** with rule id `KI:<id>`, because a known issue is Not Applicable rather
than Informational - a materially worse outcome than an unpaid finding.

**This is a known-issue matcher, not a duplicate catcher.** It can only see what you transcribed,
so **it cannot catch a concurrent duplicate** - a concurrent duplicate is, by definition, not in any
corpus yet. That limitation is structural, not something better patterns can fix.

> **Honest status: I tested this against the 28 rejections it was meant to fix, and it did not
> work.** Recall was 11-14% at 40-50% cross-fire; loosening to single-term patterns reached 46%
> recall but **77% of everything it fired on was the wrong finding**. See
> [docs/calibration.md](docs/calibration.md).

### `vg log` - your own submission history (gate G7)

A text pattern cannot see your own earlier submissions, and that is a real failure mode: **12 of the
113 rows in the source log repeat a family that was already present**, so a local history beats any
regex over a draft.

```bash
vg log add --family "stale cached exchange rate" --outcome rejected \
           --note "closed as a known design tradeoff"
vg log check draft.md
```

The log is a local YAML file (`.vg-history.yaml` by default) that is **gitignored and never
uploaded** - the tool has no network code. Matching is deterministic token overlap (Jaccard over
lowercased tokens of 4+ characters, threshold **0.18**); there is no model and no randomness. With
no log supplied, **G7 reports `?`** rather than passing.

### `vg init` - start here

The largest barrier to using this tool is not installing it, it is the inputs. `vg init` writes
every file, commented, so the tool teaches its own format:

```bash
vg init my-target/
```

It writes `ledger.yaml`, `known-issues.yaml`, `scope.txt`, `history.yaml` and `NOTES.md`.
Existing files are never overwritten without `--force`.

> The scaffold is **intended to fail validation until you fill it in** - an empty `source_url` is an
> uncited floor, and an uncited floor is a guess.

### `vg bench` - measure it yourself

```bash
vg bench --rejected rejected/ --accepted accepted/ --labels labels.csv
```

Reports, per rule, the fire rate on the rejected set, the fire rate on the accepted set, and the
**false-KILL rate on the accepted set**. That last number is the one that matters and the one this
tool has never been able to measure: **the author's log contains no paid findings.**

A tiny synthetic fixture set ships in [examples/bench/](examples/bench/) so the harness runs end to
end in CI. It proves the harness works; it measures nothing about the world. The columns are
documented in [docs/bench.md](docs/bench.md).

### The ledger is yours, and stays yours

`vg gates` needs inputs, all supplied by you at runtime and never bundled:

| Input | What it is | Where it lives |
|---|---|---|
| `--ledger` | Your transcription of a program's payout tier and exclusion clauses | Your machine. Schema in [bounty-economics](https://github.com/KhomDev/bounty-economics). |
| `--scope` | The in-scope file list from the program's scope page | Your machine |
| `--prior-audits` | Links or local copies of the prior-audit surface | Your machine |
| `--log` | Your own submission history | Your machine |

Nothing is uploaded, cached, or reported anywhere.

### In CI

```yaml
- name: viability gate
  run: |
    vg check findings/*.md --json --min-score 50 --fail-on-kill
```

Use `--fail-on-kill` rather than failing on any finding - HOLD is a normal, healthy state and
failing on it trains you to ignore the tool.

## Design principles

- **Offline and deterministic.** No network, no model, no API key. Same input, same outcome.
- **Advisory, never authoritative.** The tool produces an outcome with reasons. It does not decide.
- **Fails closed on missing input.** A gate whose input you did not supply reports `?`, not a tick.
- **Cite or it does not count.** Every rule that fires names its public documentation page.
- **Over-filtering is a failure mode too.** The catalog is a prioritiser, not a kill-switch.

## What it is not

- Not a vulnerability scanner. It does not read your target's code to find bugs.
- Not a severity calculator. Severity is a judgement about a specific program's rules.
- Not a substitute for the program's scope page, which always outranks this tool.

## Known limitations

Read these before trusting a `CLEAR`.

1. **Heuristic text matching.** Every rule is a regex over prose. It has no understanding of your
   finding and cannot verify any claim in it.
2. **It can be gamed by wording.** Because it is a text matcher, rewording a finding changes the
   outcome. That cuts both ways: it can be talked out of a true positive, and it cannot detect a
   finding that is simply well written.
3. **In-sample.** The rules were derived from the same 97 rejections they were measured against.
   See [docs/calibration.md](docs/calibration.md).
4. **The false-positive rate is unmeasured.** The author's log has no paid findings, so there is no
   control set. The tool fired on 48 of 59 rejected findings (81%); how often it fires on *accepted*
   findings is unknown. `fp_rate` is `null` for every rule and stays null until a control set
   exists. See [docs/control-set.md](docs/control-set.md) and
   [docs/prospective-protocol.md](docs/prospective-protocol.md).
5. **G6 has no supporting data.** No row in the author's log was decided by fork provenance.
6. **`origin_rows` is provenance, not validation.** It counts the rows of the *public* rejection
   dataset a rule's own token appears in, and it is not a detection rate. Ten of the twenty-two
   rules have a public token and therefore a count; the other twelve carry `0` and `evidence: none`,
   because the published columns contain no bucket for them. That is not an estimate of zero
   support - it is the refusal to publish a number that cannot be reproduced. The derivation is
   documented at the top of [`rules/anti-patterns.yaml`](rules/anti-patterns.yaml), and the
   published bucket totals are listed under [Related](#related).
7. **The gate mapping is a judgement**, and the log's gate column is post-hoc and self-assessed.
8. **A HOLD is advice.** The researcher still has to act on it.

## Install

```bash
# from crates.io
cargo install viability-gate

# or from the repository
cargo install --git https://github.com/KhomDev/viability-gate

# or from a local checkout
cargo install --path .
```

Building from source needs Rust 1.89 or newer. Prebuilt binaries for linux, macos and windows,
which need no Rust, are attached to each `v*` release tag, with SHA-256 checksums and build
attestations. See [docs/install.md](docs/install.md).

The rules are **embedded in the binary**, so the installed tool is self-contained and needs no
data directory. Override them with `--rules <DIR>`.

## Testing

```bash
cargo test --release
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
python scripts/check_tripwires.py
python scripts/gen_rule_docs.py --check
bash scripts/regen-readme-examples.sh --check
```

The suite is the mechanism that keeps the catalog honest:

- **Every rule has a positive fixture that must trigger it** - a rule that never fires is dead weight.
- **Every rule has a negative fixture that must not fire** - a rule that over-fires buries real findings.
  These negatives are **not uniform in strength**, and this suite does not claim otherwise: a negative
  that contains nothing the rule matches shows the rule is not trigger-happy on unrelated prose, but it
  does not exercise the rule's escape hatches. The regression fixtures below are what cover the guards.
- **Every reproduced defect has a regression fixture** holding the exact counterexample text, so a
  fix cannot silently revert. The integration test drives the **real engine**, not a
  re-implementation of it.
- The catalog must contain exactly 22 rules and the gate set exactly 7.
- **No shipped rule may name a target, a platform or a private path.** Ecosystem tokens live in
  [rules/packs/](rules/packs/) instead, balanced across EVM/Solidity, Move, Rust/Solana, Cairo,
  Cosmos/Go, TON, Vyper and Polkadot/Substrate, so that a rule's presence reveals nothing about which
  ecosystems the author works on. `scripts/check_tripwires.py` enforces this.

Which suspected defects were reproduced and which were not is recorded in
[docs/rule-defect-report.md](docs/rule-defect-report.md).

## Rules

Rules live in [rules/anti-patterns.yaml](rules/anti-patterns.yaml) and
[rules/gates.yaml](rules/gates.yaml). They are data, not code. A rule is:

```yaml
- id: P17
  title: No attacker-win scenario
  group: E
  verdict: soft
  weight: 30
  any_of: ["no (fund loss|attacker profit|extraction)"]
  none_of: ["attacker (profits?|gains?) \\d"]
  fix: "Identify the beneficiary and quantify the loss, or downgrade."
  doc: docs/rules/P17.md
  origin_rows: 6
  evidence: in_sample
  fp_rate: null
```

`any_of` needs one match; `all_of` needs every match; `none_of` suppresses the rule entirely.
Patterns are case-insensitive regexes unless the pattern itself says otherwise.

## Related

- [rejection-taxonomy](https://github.com/KhomDev/rejection-taxonomy) - the 113 rejections this
  catalog is derived from. **Which log buckets does this tool automate?** The catalog can reach
  72 of the 113 rows, measured against the `earliest_gate` column:

  | Reachable bucket | Rows | Rules that reach it |
  |---|---:|---|
  | `G2_exclusion_clause` | 20 | P2, P3 |
  | `design_review` | 13 | P1, P13 |
  | `G4_prior_audit` | 7 | P18 |
  | `P17_no_attacker_win` | 14 | P17 |
  | `G1_severity_floor` | 5 | P16 |
  | `G3_scope_tree` | 4 | P5 |
  | `REALISM` | 3 | P6, P11, P21, P22 |
  | `P16_below_floor` | 2 | P16 |
  | `G5_test_intent` | 1 | P15 |
  | `program_ceiling` | 1 | (P16's severity-floor class) |
  | `P19_headline_mismatch` | 1 | P19 |
  | `P20_cooperating_party` | 1 | P20 |
  | **reachable total** | **72** | |

  `D1_duplicate_check` is **41 rows and is not reachable** — the signal lives in the program's
  known-issues list, not in the finding's prose. That is the structural ceiling, and it is the
  largest single bucket.

  Three tokens in `docs/gate-mapping.yaml` have **0 rows** in the published data even though the
  catalog maps onto them: `P7_best_practice`, `P12_no_persisted_impact` and
  `P18_design_tradeoff`. The private log recorded those findings under the root-cause column
  rather than the gate column, so a rule firing on one of them cannot be scored against a gate
  token. Said plainly: **the catalog automates 72 of 113 rejections (63.7%), not 69.0%.** The 69.0%
  figure counts D1, which no pattern over a draft finding can find.

  ```bash
  # reproduce the reachable total
  python scripts/check_readme.py
  ```

- [bounty-economics](https://github.com/KhomDev/bounty-economics) - the payout floors and
  exclusion clauses that make Gates 1 and 2 mechanical
- [realism-gate](https://github.com/KhomDev/realism-gate) - the next gate after the PoC is
  written: a static checker for Foundry tests that shows only what the harness made true

Pin to a tag, not to `main`:
[viability-gate v0.4.4](https://github.com/KhomDev/viability-gate/tree/v0.4.4).

## License

| Path | License |
|---|---|
| `src/`, `tests/`, `scripts/`, `tools/` (code) | MIT |
| `rules/anti-patterns.yaml`, `rules/gates.yaml` | MIT - see the note below |
| `rules/packs/` | MIT |
| `docs/`, `README.md` (prose) | MIT |
| `.vg-history.yaml`, any `my-ledger*.yaml` | **yours**; gitignored and never distributed |

**Scope.** The MIT grant in `LICENSE` covers the entire repository, including the data files in
`rules/` and the prose in `docs/` and this README. You may extract the catalog, ship it as a
ruleset in another tool, or adapt it, under the same MIT terms as the code. The catalog is my
original work, derived from my own rejection log; the classified log is published separately in
[rejection-taxonomy](https://github.com/KhomDev/rejection-taxonomy), and this licence does not
change the terms of that repository.

**No bundled data.** This tool ships rules and schemas only. It contains no program names, chains,
reward tables, scope text or target data of any kind. All target-specific input is supplied by the
user at runtime and is never uploaded, cached or reported.
