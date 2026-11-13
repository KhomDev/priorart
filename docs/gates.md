# Gates: G1-G4 as quoted evidence

priorart has no pass and no fail. Each gate reports what it read, what it could
not read, and the quote it came from. An unrun gate is reported as unrun, never
as clear: **unknown is a fourth state, not a lenient version of pass.**

The four gates, as `priorart rules` lists them:

| Gate | Question | Checks |
|---|---|---|
| G1 | Can it be paid? | the claimed severity against the program's payout tier shape and floor |
| G2 | Is it excluded? | the finding's preconditions against the program's exclusion clause classes |
| G3 | Is the root cause in scope? | the paths the finding cites against the in-scope file list |
| G4 | Has this been found? | prior-audit coverage and ranked prior items sharing strong identifiers |

## G1 - Can it be paid?

Compares the severity the finding claims with the program's payout tier shape and
floor, both transcribed into `program.yaml` ([dossier.md](dossier.md)).

- **Floor:** the stricter of `payout.floor_severity` and the floor implied by
  `payout.tier_shape` (the five shapes are in [vocab.md](vocab.md)).
- **Claimed severity:** read from a `Severity: High` line or a
  `| Severity | High |` table row. Never inferred from prose.
- **Payable:** `true` at or above the floor, `false` below it, `null` when it
  could not be checked.

A payable result, quoted from `priorart check examples/finding-demo.md` against
`examples/dossier-demo`:

```text
CAN IT BE PAID?       tier shape: medium_and_above (pays critical/high/medium, floor medium); High is payable (cited: https://example.invalid/rain/rewards, section Rewards)
  "High: 10,000 USD"
  High is at or above the floor (Medium)
```

A below-floor result, from a Low finding against the same dossier:

```text
CAN IT BE PAID?       tier shape: medium_and_above (floor medium); Low is below the floor and pays nothing (cited: https://example.invalid/rain/rewards, section Rewards)
  "High: 10,000 USD"
  Low is below the floor (Medium): it pays nothing under this tier shape
```

**What `null` means here:** the gate could not compare, and the reason is
printed. It returns no result when there is no payout block (`program.yaml has
no payout block, so the payout gate did not run. Unknown is not payable.`), when
the shape is `unknown` (`the reward table has not been transcribed, which is not
the same as permissive`), when the floor has no `source_url` (`the floor is
uncited and the gate reports no result`), or when the finding claims no severity.
None of those is "payable".

**The unstated-floor rule.** A page that says nothing about a floor is
`tier_shape: unknown` with `floor_status: unstated`. It is **not**
`low_and_above`: an unstated floor is not evidence that Low pays, it is evidence
the page has not been read. A page that *explicitly* states "SEVERITY FLOOR:
NONE" or "any" has said something, and is `low_and_above` with
`floor_status: documented` because informational is acknowledged rather than
paid. The ledger validator enforces both directions ([ledger.md](ledger.md)).

`--fail-on-unpayable` turns `false` into exit 1. Nothing else in G1 does.

## G2 - Is it excluded?

Classifies the program's out-of-scope clauses into the 14 clause classes, then
compares them with the preconditions the finding states.

- A `scope-clause` item is classified by the taxonomy's own typical wording,
  then its `suggested_pattern`, then a documented keyword table. A clause that
  matches no class stays `null` and is never matched.
- Preconditions are detected lexically from the finding: attacker role, oracle
  dependence, ordering requirement, off-chain cause, third party, user input,
  known-issue reference, live-network testing, non-standard token, best-practice
  framing, design intent.
- **Negation-aware:** "no role", "no privileged call", "unprivileged", "any
  user", "no oracle ... is needed", "cannot" and "without" do not create a
  precondition. An explicit any-user statement also clears the role
  precondition. The window is the match's own clause, +/- 24 characters,
  stopping at clause punctuation; a unit test pins it.

A match, quoted from `priorart check examples/finding-excluded.md`:

```text
IS IT EXCLUDED?       1 clause class match your preconditions (attacker: role: operator; oracle: none)
  [program-page: scope.md p.1, section Out of scope]  "Attacks that require a privileged role, owner, governance or keeper."
     clause class: privileged_role   your precondition: the finding needs a privileged role (operator), and the clause excludes those
  -> a match is a prompt to read the clause, not a reject.
```

The no-match line is equally literal:
`no clause class matches your preconditions (attacker: any user; oracle: none)`.

**What `null` means here:** two different things, both explicit. A clause that
could not be classified is `clause_class: null` and is never used. A
precondition that was not stated prints as `attacker: not stated` (or
`oracle: none`); it is not asserted absent, only not found lexically.

`scope_boundary` is the one class G2 does not trigger: a root cause outside the
in-scope tree is G3's job, so the class is classified and shown but not matched
as an exclusion.

`--fail-on-excluded` turns any match into exit 1. A match is a prompt to read
the clause, not a reject.

## G3 - Is the root cause in scope?

Traces the code the finding cites into the program's in-scope list.

- Paths cited in the finding are checked against `scope-path` entries with glob
  support (`src/*.sol`, `src/`, `**/Vault.sol`).
- A contract or function name is accepted too: if the finding says
  `VaultEngine.open`, the scope entry `src/VaultEngine.sol` covers it, and the
  output says `VaultEngine (in src/VaultEngine.sol)`.

From the demo finding, in `NOTES`:

```text
- G3 scope: the finding cites scoped code: VaultEngine (in src/VaultEngine.sol), BalanceSheet (in src/BalanceSheet.sol), PegStabilityModule (in src/PegStabilityModule.sol)
```

A cited path that is not in scope is shown, not hidden:
`it also cites src/Vault.sol - check that the root cause is not there`.

**What `null` means here:** no scope list, or no cited code, is reported as
such (`no in-scope file list was indexed from the program page; scope was not
traced`, or `the finding names no scoped file or contract, so the root cause was
not traced`). It is never "in scope".

## G4 - Has this been found?

Two pieces of evidence, both quotes:

- **Named prior audits** from `program.prior_audits[].covers`: a cited path or a
  cited contract/function name is matched to each audit's coverage, and the
  output says which identifier matched. A `known_issues` string that shares two
  symbols with the finding is quoted too.
- **Ranked prior items** from the identifier matcher ([matching.md](matching.md)),
  each with its source, page, section, quote, shared identifiers and mechanism.

From the demo finding, in `NOTES`:

```text
- G4 prior audits: Internal review covers src/VaultEngine.sol (cited identifier VaultEngine), src/BalanceSheet.sol (cited identifier BalanceSheet) - read that report's findings for this function
```

And the ranked item itself, in `HAS THIS BEEN FOUND?`:

```text
HAS THIS BEEN FOUND?  1 prior item to read (shares >=2 symbol identifiers)
  [audit: internal-review.md p.1, section H-02 Reserve backing check]  "The backing check in `BalanceSheet.distributeSurplus` reverts with `ReserveBackingShortfall` when the stable reserve is smaller than the summed debt of fee-exempt collateral types."
     shared: balancesheet, balancesheet.distributesurplus, pegstabilitymodule, pegstabilitymodule.sellstable, reservebackingshortfall, vaultengine, vaultengine.open
     their mechanism: accounting_order_of_operations
  -> reading list only. priorart does not decide duplicates; see docs/benchmark.md
```

The closing line is the whole point of retrieval-only mode: 0.1.0 prints no
duplicate warning, because the final benchmark's false-alarm rate is above the
plan's trigger ([benchmark.md](benchmark.md)).

**What `null` means here:** an audit with no `covers` list prints
`no 'covers' list recorded, so coverage could not be read`; a dossier with no
audit surface prints `no prior-audit surface in this dossier`. Neither is
"not previously found". The no-hit line is
`nothing ranked (no prior item shares >=2 symbol identifiers)`; a finding with
fewer than two identifiers says
`nothing ranked (the finding names fewer than 2 symbol identifiers)`.

## Why there is no score

The deleted 0-100 score was a weighted sum of hand-set rule weights. Its own
benchmark showed the rules it was built on did not separate paid from unpaid
findings: 12 of the 25 hard-killed findings were paid, so the kill's precision
was 13/25 (52%) that a finding was unpaid. A number that looks calibrated and is
not is worse than a quote with a page number, which a reader can check.
