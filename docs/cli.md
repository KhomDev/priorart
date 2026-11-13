# CLI reference

Every command, flag and exit code in `priorart 0.1.0`. `priorart --help` and
`priorart <command> --help` print the same surface; the binary is the source of
truth for wording.

```text
priorart build <program-dir> [--out FILE] [--program ID] [--json]
priorart check <finding.md> [--dossier FILE] [--top-k N] [--json]
                            [--fail-on-excluded] [--fail-on-unpayable]
priorart ledger validate <ledger.yaml...> [--json] [--strict] [--example-mode]
                            [--taxonomy]
priorart vocab list [--json]
priorart vocab show <tier-shapes|clause-classes|gate-crossref> [--json]
priorart init [dir] [--force]
priorart explain <clause-class-id|gate-id|tier-shape-id>
priorart rejections summary [--json]
priorart rules
priorart --version
```

The binary name is `priorart` on every platform (`priorart.exe` on Windows).
There is one binary: no `vg` alias and no second entry point.

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Analysis produced, including "prior items found". A hit is evidence, not a failure. |
| 1 | Only two opt-in flags: `--fail-on-excluded` (a clause class matched) and `--fail-on-unpayable` (the claimed severity is below the floor). |
| 2 | Usage, IO or parse error: bad flag value, missing file, malformed YAML or JSON, unknown vocabulary, unknown command. |

Nothing else exits 1. The default is evidence without a verdict, which is why a
CI job must pass one of the two flags to fail on a finding.

## stdin and JSON

- **No command reads stdin.** Every input is a path. `-` is not special.
- Text output goes to stdout; errors go to stderr as `priorart: <message>`.
- `--json` is available on `build`, `check`, `ledger validate`, `vocab list`,
  `vocab show` and `rejections summary`. Schema ids are fixed:
  `priorart.dossier/v1`, `priorart.check/v1`,
  `priorart.rejections-summary/v1`; `ledger validate --json` keeps the Python
  validator's key set (`schema_version`, `ok`, `strict`, `ledgers`, `entries`,
  `errors`, `warnings`, plus `ledger` for a single file).
- With `build --json` the dossier goes to stdout and build notes go to stderr,
  so a pipe gets clean JSON.
- `ledger validate` accepts several YAML files; with more than one, each message
  is prefixed with its file and the top-level `ledger` key is omitted.

## build

Reads a folder you saved and writes one deterministic dossier
([dossier.md](dossier.md)).

| Flag | Meaning |
|---|---|
| `--out FILE` | Output path. Default: `<program-dir>/<program-id>.dossier`. |
| `--program ID` | Program id override. Default: `program.yaml`'s `id`, else the folder name, slugified (lowercase letters, digits, `.`, `-`, `_`). |
| `--json` | Print the dossier JSON to stdout instead of the summary. Notes go to stderr. |

Exit 0 on success, even when a PDF could not be read (that is a warning, and the
source is skipped). Exit 2 when the folder does not exist or holds no inputs.

```text
$ priorart build examples/dossier-demo
priorart 0.1.0 - dossier dossier-demo (examples/dossier-demo/dossier-demo.dossier)
  3 source(s), 13 item(s)
    audit-finding    1
    known-issue      2
    payout-tier      3
    scope-clause     4
    scope-path       3
  [src-0001] program-page  scope.md  p.1  sha256 ecf837516e94
  [src-0002] known-issues  known-issues.md  p.1  sha256 46534c52f77f
  [src-0003] audit  audits/internal-review.md  p.1  sha256 79e56ab10021
  note: 3 source(s) indexed (1 audit(s)); 13 item(s) extracted
Next: priorart check <finding.md> --dossier examples/dossier-demo/dossier-demo.dossier
```

## check

Reports the three sections for one finding. It never exits non-zero for "prior
items found" or "excluded" unless asked to.

| Flag | Meaning |
|---|---|
| `--dossier FILE` | The dossier to read. Without it: the single `.dossier` file beside the finding; zero or several is exit 2. |
| `--top-k N` | How many ranked prior items to show. Default 5, range 1-50; outside that is exit 2. |
| `--json` | Print `priorart.check/v1` JSON instead of text. |
| `--fail-on-excluded` | Exit 1 when any exclusion clause class matches the finding's preconditions. |
| `--fail-on-unpayable` | Exit 1 when the claimed severity is below the program's floor. |

The claimed severity comes from a `Severity: High` line or a
`| Severity | High |` table row. It is never inferred from prose; without it the
payout gate reports that it could not compare.

```text
$ priorart check examples/finding-demo.md --dossier examples/dossier-demo/dossier-demo.dossier

HAS THIS BEEN FOUND?  1 prior item shares >=2 strong identifiers
  [audit: internal-review.md p.1, section H-02 Reserve backing check]  "The backing check in `BalanceSheet.distributeSurplus` reverts with `ReserveBackingShortfall` when the stable reserve is smaller than the summed debt of fee-exempt collateral types."
     shared: balancesheet.distributesurplus, pegstabilitymodule, pegstabilitymodule.sellstable, reservebackingshortfall, vaultengine.open
     their mechanism: accounting_order_of_operations

IS IT EXCLUDED?       no clause class matches your preconditions (attacker: any user; oracle: none)

CAN IT BE PAID?       tier shape: medium_and_above (pays critical/high/medium, floor medium); High is payable (cited: https://example.invalid/rain/rewards, section Rewards)
  "High: 10,000 USD"
  High is at or above the floor (Medium)
```

The section ends with a reading-list note. 0.1.0 ships retrieval-only: it prints
no duplicate warning, because the final benchmark's false-alarm rate is above the
plan's trigger ([benchmark.md](benchmark.md)).

A match with `--fail-on-excluded` prints the same evidence and exits 1:

```text
$ priorart check examples/finding-excluded.md --dossier examples/dossier-demo/dossier-demo.dossier --fail-on-excluded
IS IT EXCLUDED?       1 clause class match your preconditions (attacker: role: operator; oracle: none)
  [program-page: scope.md p.1, section Out of scope]  "Attacks that require a privileged role, owner, governance or keeper."
     clause class: privileged_role   your precondition: the finding needs a privileged role (operator), and the clause excludes those
$ echo $?
1
```

For `--fail-on-unpayable`, the finding must claim a severity below the floor
(e.g. `Severity: Low` against the demo's Medium floor); the output then reads
`Low is below the floor and pays nothing` and the exit is 1. The committed
`examples/finding-unpayable.md` claims Medium against a Medium floor and is
therefore payable; the exit-1 path is exercised by
`tests/cli.rs::fail_on_unpayable_exits_one_below_the_floor`.

## ledger validate

A Rust port of bounty-economics `scripts/validate.py`, preserving schema v1
semantics and the Python validator's JSON shape ([ledger.md](ledger.md)).

| Flag | Meaning |
|---|---|
| `--json` | Machine-readable report instead of text. |
| `--strict` | Warnings become failures (exit 1). |
| `--example-mode` | Relax the placeholder-host check; automatic for `*.example.yaml`. |
| `--taxonomy` | Print the shipped vocabularies instead of validating. |

Several ledger files are accepted in one run. Exit 1 for validation errors (or
warnings under `--strict`), 2 for a missing file, unparseable YAML, a non-list
document or more than one YAML document in a file.

```text
$ priorart ledger validate examples/ledger.example.yaml examples/tier-shapes.example.yaml
WARN  priorart/examples/ledger.example.yaml: entry[0] (example-lending-v2): source_url uses a placeholder host (example.invalid) - expected in an example file, not in a real ledger.
...
priorart/examples/ledger.example.yaml: 1 entry, 0 error(s), 1 warning(s)
priorart/examples/tier-shapes.example.yaml: 5 entries, 0 error(s), 9 warning(s)

6 entries, 0 error(s), 10 warning(s)
```

`WARN` lines go to stdout, `ERROR` lines to stderr, then the one-line summary.

## vocab

`list` prints the 5 tier shapes, the 14 clause classes and the gate
cross-reference; `show` prints one in full. `--json` prints the parsed YAML
(`list --json` prints ids, counts and sources).

```text
$ priorart vocab list
tier shapes (5) - vocab/tier-shapes.yaml
  critical_only      pays: critical
  high_and_above     pays: critical/high
  medium_and_above   pays: critical/high/medium
  low_and_above      pays: critical/high/medium/low
  unknown            pays: nothing
...
```

## init

Writes `program.yaml`, `scope.md`, `known-issues.md` and `audits/README.md`
into the directory (default: the current directory). Refuses to overwrite an
existing file without `--force`. `build` reads the scaffold without edits.

```text
$ priorart init my-program
priorart 0.1.0 - scaffolded my-program

  program.yaml
  scope.md
  known-issues.md
  audits/README.md

Next: save the program's real page over scope.md, then run
  priorart build my-program
```

## explain

One clause class, gate or tier shape, with the text that matters: what it kills,
typical wording, the escape hatch and the suggested pattern for a clause class;
the question and what it checks for a gate; pays/floor/implication/risk for a
shape. Unknown id is exit 2 with the vocabulary list in the message.

```text
$ priorart explain G2
G2: Is it excluded?
  checks: the finding's preconditions against the program's exclusion clause classes
  this gate reports evidence. It has no pass or fail, and an unread input is reported as unread, never as a pass.
```

## rejections summary

Totals, triage outcomes, earliest catchable gate, bug family, duplicate source
and preventable-by counts, plus provenance for the shipped 113-row dataset.
`--json` wraps the unchanged `summary.json` under `dataset` with
`provenance`. See [rejections.md](rejections.md).

```text
$ priorart rejections summary
rejection dataset - 113 logged submissions, 110 published, 3 withheld
  pre-hunt catchable from the public record: 63 of 113 (55.8%)
  one researcher's log: no positive class, no target dimension. See docs/rejections.md.
...
```

## rules

The whole check surface on one page: G1-G4, the matching threshold, the
vocabulary counts, what the tool does not do, and the exit codes.

