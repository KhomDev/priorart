# viability-gate

**Run this before you submit, so the report does not come back Informative, Duplicate, or out of scope, and so it does not read like AI spam.**

For bug-bounty hunters, and for anyone using an AI to draft the report. One offline binary. No account, no API key, no network.

```bash
cargo install --git https://github.com/KhomDev/viability-gate
vg check finding.md
```

0.6.0 is not published to crates.io. Install from git, or `cargo install --path .` from a checkout. Rust 1.89 or newer if you build it. Release binaries need no Rust. Both `vg` and `viability-gate` are installed; they are the same program.

## Before and after

A real report, closed Informative as a duplicate of a published contest issue. Personal emails were not in the text. On the previous release, P7 held it: the matcher saw `best-practice` inside `rather than a best-practice observation`, and P18 did not see `accepted as Sherlock issue M-1`. This is what 0.6.0 prints on that report. `scripts/regen-readme-examples.sh` produces the block, and CI fails if it drifts.

<!-- BEGIN:example -->

```text
viability-gate 0.6.0  —  cmp-04.md

  OUTCOME   KILL
            A hard blocker fired. Do not submit as written.
  SCORE     60 / 100   (uncalibrated heuristic; not a probability; threshold 50)

  HARD  P18  Known characteristic documented in a prior audit
             ↳ matched: accepted as Sherlock issue
             ↳ in sentence: defect the protocol's own 2026-03 audit accepted as Sherlock issue M-1 and whose accepted fix never
             ↳ matched: accepted as Sherlock issue M-1
             ↳ in sentence: defect the protocol's own 2026-03 audit accepted as Sherlock issue M-1 and whose accepted fix never
             ↳ matched: previously-reported
             ↳ in sentence: credit): a known, previously-reported defect whose accepted fix never shipped to mainnet
             → Do not submit. Prior audit findings are part of the public record;
             → submitting a known design characteristic signals you did not read the
             → prior audit.
             (docs/rules/P18.md; evidence: in-sample; origin rows: 1)

  NOTE  P26  Program severity rule not quoted
             ↳ matched: weak: a severity is asserted, but the program's own severity rule is not quoted
             ↳ in sentence: weak: a severity is asserted, but the program's own severity rule is not quoted
             → Quote the program's own severity sentence that this claim maps to. Naming
             → a tier is the author's label, not the program's rule. finding.yaml
             → severity_rule_quoted answers this.
             (docs/rules/P26.md; evidence: none; origin rows: 0)

  PASS  P1, P6, P12, P13, P2, P3, P5, P11, P20, P7, P16, P21, P22, P15, P4, P9, P8, P10, P14, P17, P19, P23, P24, P25
  SKIP  G1 (no ledger supplied)   G2 (no ledger supplied)   G3 (no scope tree supplied)   G4 (no prior-audit surface supplied)   G5 (no test suite supplied)   G6 (fork status unknown)   G7 (no own-history log supplied)   D1 (no known-issues corpus or prior-finding index supplied)

  Gates:  G1 ?   G2 ?   G3 ?   G4 ?   G5 ?   G6 ?   G7 ?   D1 ?

  ────────────────────────────────────────────────────────────
  Advisory, never authoritative. This tool produces an outcome with reasons;
  it does not decide, and it cannot read your target's live rules.
  Over-filtering is its own failure mode - the catalog is a prioritiser, not
  a kill-switch. A triage rejection is not a final verdict. CLEAR means only
  that no known blocker was found by this tool.
```

<!-- END:example -->

P18 is the hard stop. P7 is in the pass list. P26 is a weight-0 note: the report names a tier and does not quote the program's severity sentence. The score is 100 minus the weights that fired. Gates were not given a ledger, so they are `?`, and `?` is not a pass.

![cmp-04 through vg](docs/demo.gif)

## Benchmark

220 paid Medium/High findings (110 Code4rena, 110 Sherlock) and 120 invalid or won't-fix findings, labels applied after `vg` ran. Details, the selection rule, and the per-rule table are in [docs/benchmark.md](docs/benchmark.md).

| Set | N | Hard KILL | Rate |
|---|---:|---:|---:|
| Paid Medium/High | 220 | 19 | 8.6% |
| Invalid + won't-fix | 120 | 19 | 15.8% |
| Code4rena QA | 60 | 14 | 23.3% |
| Sherlock Low/Info | 60 | 10 | 16.7% |

A hard kill on this pair of sets is right about half the time: 19 of 38 kills are paid findings. Nine of the 19 paid kills are P2, seven are P3. Paid reports talk about admins and sandwiches. The full table is in the benchmark doc. The in-sample recall further down is a different set and an older catalog.

## What you run

```bash
vg check finding.md
vg check finding.md --ledger ledger.yaml --scope scope.txt --known-issues known.yaml --index index.yaml
vg import audit audit.yaml --out target/
vg import program saved-page.html --out target/
vg import prior audits/ --out target/
vg match finding.md --index target/index.yaml
vg gates --ledger ledger.yaml --scope scope.txt --finding finding.md
vg explain P18
vg bench --rejected rejected/ --accepted accepted/ --labels labels.csv
```

`vg import` reads local files only: an engagement `audit.yaml`, a program page you saved, or prior-audit markdown and PDF. It writes the ledger, the scope list, and the known-issues file, and it says in `IMPORT-NOTES.md` what it did not find. It does not invent a regex. Schema: [docs/import.md](docs/import.md).

`vg match` and `--index` rank a draft against prior findings by code identifiers (function names, paths), not by word overlap. The author measured vocabulary overlap at about 0.024 and it did not separate duplicates. The fail line is a stated threshold, not a fitted one. [docs/duplicate-matching.md](docs/duplicate-matching.md).

A `finding.yaml` next to the report, or `<name>.finding.yaml`, can record `attacker`, `victim`, `preconditions`, `impact_usd`, `severity_rule_quoted`, `prior_audit_check`, `duplicate_check`, and `status`. Filled claim fields suppress the matching note. A sentence that says the prior-audit check was done does not suppress P18.

## What it checks

**26 entries, P1–P26**, plus gates G1–G7 and D1. The catalog is [rules/anti-patterns.yaml](rules/anti-patterns.yaml). One page per rule is generated into [docs/rules/](docs/rules/).

P1–P22 are the rejection shapes (privileged role, best-practice, prior audit, no attacker-win, headline contradicted by the proof, and the rest). P23–P26 are weight-0 notes from the author's post-mortems: attacker profile, preconditions and victim action, a quantified impact, and a quoted program severity rule. A weight-0 note does not hide a gate that could not run.

| # | Condition | Outcome |
|---|---|---|
| 1 | any hard rule | **KILL** |
| 2 | any soft rule, any failed gate, or score under the threshold | **HOLD** |
| 3 | any warn rule with weight | **NOTE** |
| 4 | any gate `?` | **INCOMPLETE** |
| 5 | only weight-0 notes, and every gate ran | **NOTE** |
| 6 | otherwise | **CLEAR** |

There is no `SUBMIT`. `CLEAR` means no known blocker was found by this tool. The score is 100 minus the weights of the fired rules. It is not a probability. Gates do not subtract from it.

## Limits

Read these before trusting a `CLEAR`.

**Name collision.** `vg` is also an unrelated bioinformatics tool. This crate ships `vg` and `viability-gate`. Use whichever is not already on your `PATH`. [docs/install.md](docs/install.md).

**In-sample, and of an older catalog.** The original measurement was 59 of 97 logged rejections, scored on the catalog as it stood for 0.2.0. The log has since grown, and 0.3.0, 0.5.0, and 0.6.0 changed rules. That study has not been re-run. The source texts are private.

| Scoring | Result |
|---|---:|
| **Strict** — flagged the gate that actually decided it | **14/59 (23.7%)** |
| **Lenient** — flagged any gate-mapped reason to stop | **45/59 (76.3%)** |
| Miss — no gate-mapped reason at all | 14/59 |

The 52-point gap is the summary: good at saying stop, bad at saying why. The rules were derived from the same log, so 23.7% is closer to a ceiling than to an unbiased estimate. [docs/calibration.md](docs/calibration.md).

**The author's log has no paid findings.** The false-positive rate on that log is unmeasured. The public contest numbers above are the measurement the public data can support. They are not a substitute. [docs/control-set.md](docs/control-set.md).

**Heuristic text.** Every rule is a regex or a small structural check over prose. It can be talked out of a true hit by rewording, and it cannot see a duplicate you did not put in the index. Negation handling, and where it still fails, is [docs/negation-audit.md](docs/negation-audit.md).

**G6 has no supporting rows.** No rejection in the author's log was decided by fork provenance.

**`origin_rows` is provenance, not a detection rate.** It counts rows of the public rejection dataset. Rules with no public token carry 0 and `evidence: none`. That is a refusal to publish a number that cannot be reproduced, not a claim of zero support.

**A HOLD is advice.** The researcher still decides.

## Related

- [rejection-taxonomy](https://github.com/KhomDev/rejection-taxonomy) — the public log this catalog is derived from. Which buckets the catalog can reach, against the `earliest_gate` column of the pinned tag:

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

  `D1_duplicate_check` is 41 rows and is not reachable from the finding's prose alone. "Can reach" means a rule maps to that bucket. It is not a recall figure.

  ```bash
  git clone --branch "$(python scripts/check_readme.py --print-pinned-tag)" \
      https://github.com/KhomDev/rejection-taxonomy ../rejection-taxonomy
  python scripts/check_readme.py --require-dataset
  ```

- [bounty-economics](https://github.com/KhomDev/bounty-economics) — the ledger schema behind G1 and G2
- [realism-gate](https://github.com/KhomDev/realism-gate) — a static checker for the proof, after this one
- [examples/bench/](examples/bench/) — the synthetic bench fixture. It proves the harness runs. It measures nothing about the world.
- [docs/rules/](docs/rules/) — generated page per rule

Pin to a tag, not to `main`:
[viability-gate v0.6.0](https://github.com/KhomDev/viability-gate/tree/v0.6.0).

## License

MIT, the whole repository, including `rules/` and `docs/`. The catalog is original work derived from the author's rejection log. The classified log is published separately and this licence does not change its terms.

No bundled program data. Target ledgers, scopes, indexes, and history files are yours. `eval/` commits URLs and scores, not report text. See [docs/licensing.md](docs/licensing.md).
