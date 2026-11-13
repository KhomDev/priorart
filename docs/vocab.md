# The compiled vocabularies

Three YAML files in `vocab/`, moved from bounty-economics v0.6.1 (MIT) and
compiled into the binary with `include_str!`. There is no data directory at
runtime and no way for the vocabulary to drift from the code: a test asserts the
counts and that every suggested pattern compiles.

- `vocab/tier-shapes.yaml` - byte-identical to the v0.6.1 tag (sha256
  `B6041C07825EA0234042E56896D526C7B2A9D2DD5E75C343D5F16448F4DA78A9`).
- `vocab/clause-classes.yaml` - byte-identical to the v0.6.1 tag (sha256
  `FCA9C0EE6BE43DE258773AEB001A5F68FC6A0C67F3A0829B9C58A00604BE1616`).
- `vocab/gate-crossref.yaml` - edited on the move: its rows named
  viability-gate's P1-P22 engine, which priorart deletes, so those rows are gone.
  The structure and ids are priorart's G1-G4.

## 5 payout tier shapes - `vocab/tier-shapes.yaml`

| id | pays | floor |
|---|---|---|
| `critical_only` | critical | critical |
| `high_and_above` | critical, high | high |
| `medium_and_above` | critical, high, medium | medium |
| `low_and_above` | critical, high, medium, low | low |
| `unknown` | nothing | null |

Two rules the file exists to enforce:

1. **There is no `any` shape.** A page that *explicitly* states "SEVERITY
   FLOOR: NONE" or "any" is `low_and_above` with `floor_status: documented`:
   informational findings are acknowledged rather than paid, so Low is the
   lowest tier that pays.
2. **A page that says nothing about a floor is `unknown` with
   `floor_status: unstated`.** That is not evidence that Low pays; it is
   evidence the page has not been read.

## 14 exclusion clause classes - `vocab/clause-classes.yaml`

Each class carries what it kills, typical wording, an escape hatch and a
`suggested_pattern` regex. One line each:

| id | what it kills |
|---|---|
| `privileged_role` | anything requiring an admin, owner, operator or capability to act |
| `human_error` | admin mistakes, including unknowing admin harm |
| `admin_input` | findings whose root cause is an unvalidated parameter supplied by a trusted role |
| `dos_no_extraction` | freezes, griefing and liveness attacks that do not transfer funds |
| `front_run_only` | attacks that need to win a mempool ordering race |
| `best_practice` | style, gas and convention deviations |
| `design_issue` | intentional tradeoffs you disagree with |
| `off_chain` | root causes in node logic, relayers, signers, backends or infrastructure |
| `third_party` | issues in oracles, bridges and other external dependencies |
| `user_input` | findings whose precondition is the victim's own input construction |
| `known_issue` | anything in the prior-audit surface or the program's own known-issues list |
| `testing_limits` | proofs that require live-network execution |
| `non_standard_token` | token-integration findings involving fee-on-transfer, rebasing or unusual tokens |
| `scope_boundary` | root causes outside the named in-scope file tree |

The suggested pattern is a starting point for testing a finding, not a verdict; a
match in G2 is a prompt to read the clause ([gates.md](gates.md)). G2 does not
trigger `scope_boundary`: that is G3's job.

## Gate cross-reference - `vocab/gate-crossref.yaml`

Which catalog answers which gate:

| Row | Catalog | Meaning |
|---|---|---|
| `G1_severity_floor` | tier-shapes | the relevant tier shape and floor |
| `G2_exclusion_clause` | clause-classes | the clause class and its escape hatch |
| `G4_prior_audit` | prior-audit-surface | the audits in the dossier, not a generic list |
| `program_ceiling` | tier-shapes | a program-wide ceiling is the same shape judgement |

And what the program's terms do **not** answer: `G3_scope_tree` (the scoped file
tree in the repository), `duplicate_check` (the prior-art surface, not the terms
page), and `realism` (attacker model and configuration, read from the finding and
the PoC).

## Using them

```sh
priorart vocab list                      # all three, as text
priorart vocab list --json               # ids, counts and sources
priorart vocab show clause-classes       # one in full
priorart vocab show tier-shapes --json   # parsed YAML as JSON
priorart explain privileged_role         # one clause class
priorart explain G2                      # one gate
priorart explain medium_and_above        # one tier shape
```

The tests that guard them:
`vocab::tests::five_tier_shapes_and_fourteen_clause_classes`,
`::there_is_no_any_shape_and_no_informational_floor`,
`::unknown_is_the_only_empty_paying_shape`,
`::every_suggested_pattern_compiles` and
`tests/cli.rs::vocab_list_has_five_shapes_and_fourteen_classes`.

## Provenance and licence

- `tier-shapes.yaml` and `clause-classes.yaml` are byte-identical to
  bounty-economics v0.6.1 `data/`; the sha256 values above were recomputed on
  2026-11-13.
- `gate-crossref.yaml` is edited as described; its structure is unchanged.
- Licence: MIT (code and vocabularies), see [licensing.md](licensing.md).
