# CLI reference

Every command, flag and exit code in `priorart 0.2.1`. `priorart --help` and
`priorart <command> --help` print the same surface; the binary is the source of
truth for wording.

```text
priorart fetch <URL>... [--dir DIR] [--force]
priorart build <program-dir> [--out FILE] [--program ID] [--json]
priorart check <finding.md> [--dossier FILE] [--top-k N] [--json]
                            [--fail-on-excluded] [--fail-on-unpayable]
priorart ledger validate <ledger.yaml...> [--json] [--strict] [--example-mode]
                            [--taxonomy]
priorart vocab list [--json]
priorart vocab show <tier-shapes|clause-classes|gate-crossref> [--json]
priorart taxonomy classify <file>... [--json]
priorart candidates [FINDING] --corpus <dossier|dir|prepared.json>
                            [--repo DIR | --index FILE]... [--repo-id ID]
                            [--commit SHA] [--top-k N] [--exclude ID]...
                            [--prepare FILE] [--json]
priorart init [dir] [--force]
priorart explain <clause-class-id|gate-id|tier-shape-id>
priorart rejections summary [--json]
priorart rules
priorart --version
```

The binary name is `priorart` on every platform (`priorart.exe` on Windows).
There is one CLI binary, with no `vg` alias. The MCP server `priorart-mcp`
ships beside it and runs this CLI; its tools are in [mcp.md](mcp.md).

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

## fetch

The only command that opens a socket. One HTTPS GET per URL, no crawling.
Pages are written to `<dir>/pages/001.html`, `002.html`, … in the order given,
plus `<dir>/fetch.json` (url, final URL, HTTP status, UTC time, byte count,
sha256). `build` reads `pages/` after the classic `scope.*` file.
[no-network.md](no-network.md) is the policy.

| Flag | Meaning |
|---|---|
| `--dir DIR` | Program folder. Default: the current directory. |
| `--force` | Overwrite a page this command already saved. `scope.md` is never overwritten. |

Refused before a request: `http:`, `file:`, any scheme other than `https`, a
URL with userinfo, an IP-literal host, `localhost`. At most 5 redirects, each
of which must itself be HTTPS. 30 second timeout. 16 MiB body. No cookies, no
`Authorization` header. `http_proxy`, `https_proxy`, `all_proxy` and the
uppercase forms are ignored. `User-Agent` is `priorart/<version>`.

Exit 2 on a rejected URL, a non-2xx response, too many redirects, or a body
over the cap, and nothing is written. A binary built with
`--no-default-features` has no HTTP client and exits 2 on `fetch`.

```text
$ priorart fetch https://example.invalid/program/information --dir my-program
priorart 0.2.2 - fetched 1 page(s) into my-program
  pages/001.html  200  12345 bytes  sha256 abcdef012345  https://example.invalid/program/information
  wrote fetch.json
Next: priorart build my-program
```

## build

Reads a folder you saved and writes one deterministic dossier
([dossier.md](dossier.md)).

| Flag | Meaning |
|---|---|
| `--out FILE` | Output path. Default: `<program-dir>/<program-id>.dossier`. |
| `--program ID` | Program id override. Default: `program.yaml`'s `id`, else the folder name, slugified (lowercase letters, digits, `.`, `-`, `_`). |
| `--json` | Print the dossier JSON to stdout instead of the summary. Notes go to stderr. |
| `--no-pdftotext` | Never run `pdftotext`. A PDF the built-in scrape cannot read is reported unread and skipped, so poppler never parses an untrusted file. |

Exit 0 on success, even when a PDF could not be read (that is a warning, and the
source is skipped). Exit 2 when the folder does not exist, holds no inputs, is
over an input limit ([dossier.md](dossier.md#input-limits)), or a **required**
input is not a regular file (`program.yaml`). An optional page, known-issues or
audit name (including a symlinked subdirectory of `audits/`) that is a symlink,
FIFO or device is skipped with a note naming it; the build exits 2 only when no
regular input remains.

```text
$ priorart build examples/dossier-demo
priorart 0.2.1 - dossier dossier-demo (examples/dossier-demo/dossier-demo.dossier)
  3 source(s), 13 item(s)
    audit-finding    1
    known-issue      2
    payout-tier      3
    scope-clause     4
    scope-path       3
  [src-0001] program-page  scope.md  p.1  sha256 b63b6be9771d
  [src-0002] known-issues  known-issues.md  p.1  sha256 a145ccf17597
  [src-0003] audit  audits/internal-review.md  p.1  sha256 64bff58ef788
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

Both the finding and the explicit dossier are required inputs, so a symlink,
FIFO or device at either path is exit 2, refused by name. A dossier
**discovered** without `--dossier` is optional: a non-regular only candidate is
exit 2 naming it, and a regular candidate beside it is used instead. A finding
over 16 MiB or a dossier over 64 MiB is exit 2 as well, refused by size before
it is read. See [dossier.md](dossier.md#input-limits).

```text
$ priorart check examples/finding-demo.md --dossier examples/dossier-demo/dossier-demo.dossier

HAS THIS BEEN FOUND?  2 prior items to read (share >=2 symbol identifiers)
  [audit: internal-review.md p.1, section M-03 Fee index rounding]  "The consistency check in `FeeLedger.distributeFees` reverts with `FeeAccountingMismatch` when the fee index rounds up past the collected balance."
     shared: feeaccountingmismatch, feeledger, feeledger.distributefees
     their mechanism: rounding_direction   yours: accounting_order_of_operations
  [known-issues: known-issues.md p.1, section Known issues]  "`SwapDesk.swapIn` can reach its cap during high volume; acknowledged, by design."
     shared: swapdesk, swapdesk.swapin
  -> reading list only. priorart does not decide duplicates; see docs/benchmark.md

IS IT EXCLUDED?       no clause class matches your preconditions (attacker: any user; oracle: none)

CAN IT BE PAID?       tier shape: medium_and_above (pays critical/high/medium, floor medium); High is payable (cited: https://example.invalid/lend/rewards, section Rewards)
  "High: 10,000 USD"
  High is at or above the floor (Medium)
```

That closing note is retrieval-only: 0.1.0 prints no duplicate warning, because
the final benchmark's false-alarm rate is above the plan's trigger
([benchmark.md](benchmark.md)).

A match with `--fail-on-excluded` prints the same evidence and exits 1:

```text
$ priorart check examples/finding-excluded.md --dossier examples/dossier-demo/dossier-demo.dossier --fail-on-excluded
IS IT EXCLUDED?       1 clause class match your preconditions (attacker: role: operator; oracle: none)
  [program-page: scope.md p.1, section Out of scope]  "Attacks that require a privileged role, owner, governance or keeper."
     clause class: privileged_role   your precondition: the finding needs a privileged role (operator), and the clause excludes those
$ echo $?
1
```

For `--fail-on-unpayable`, the finding must claim a severity below the floor.
`examples/finding-unpayable.md` claims `Severity: Low` against the demo's Medium
floor, prints `Low is below the floor and pays nothing`, and exits 1 with the
flag. The same test is
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
WARN  examples/ledger.example.yaml: entry[0] (example-lending-v2): source_url uses a placeholder host (example.invalid) - expected in an example file, not in a real ledger.
...
examples/ledger.example.yaml: 1 entry, 0 error(s), 1 warning(s)
examples/tier-shapes.example.yaml: 5 entries, 0 error(s), 9 warning(s)

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

## taxonomy classify

Prints the root-cause classes, impacts and assets a finding (or any text)
asserts, using the deterministic lexicon in `vocab/taxonomy.yaml` and the
clause-level negation rules in `src/taxonomy/`. It is a reading aid, not a
verdict: the classes are cue matches, and per-class precision is measured only
by the benchmark. `--json` prints `priorart.taxonomy-classify/v1`. A file that
is missing, not regular, not UTF-8 or over 16 MiB exits 2 before any output;
an abstention (no class, impact or asset) exits 0 with `abstain_reason:
"no-mechanism"`.

## candidates

Generates structural candidates for a finding (or, with `--prepare`, for a
corpus) and prints them with the five pair features: function and state anchor
overlap, call-graph distance (≤ 2), mechanism match (a shared root cause in a
shared contract), impact match, and today's identifier score as one feature
only. Shared identifiers alone never admit a candidate. An abstention carries a
reason (`no-anchor`, `no-mechanism`, `no-candidates`, `corpus-missing`,
`repo-missing`). The corpus is a dossier, a directory of `.md`/`.txt` items, or
a prepared `priorart.candidates-corpus/v1` file; `--index` accepts a repository
index saved by `priorart anchors index`, and a prepared corpus whose
`repo@commit` pins do not match the supplied index exits 2. `--json` prints
`priorart.candidates/v1`, ordered by anchor overlap, distance, shared root
causes, identifier specificity and prior id.

`--ranker <FILE>` scores every candidate from a committed
`priorart.ranker-weights/v1` table (PA-B4; trained by
`eval/dupbench/train_ranker.py` on the Dup-Bench train split) and orders by
score descending, with the five-key order above as the tie-break; each
candidate then carries `ranker_score`. The score is one dot product over the
table, so inference is deterministic and offline; there is no default table,
no model runtime, no LLM and no network. Without the flag the output is
unchanged. A table that is missing, malformed, omits a feature, names an
unknown one, or states no `missing_value` for a feature that can be null exits
2 naming the rule; a null `call_graph_distance` is scored as
`max_candidate_distance + 1` and sets the `call_graph_distance_missing`
feature, never a silent zero.

`--alarm <FILE>` (requires `--ranker`) adds an `alarm` block from a committed
`priorart.alarm-calibration/v1` table (PA-B5; written by
`eval/dupbench/calibrate_alarm.py` from the Dup-Bench validation split only).
The top candidate's ranker score is mapped through the table's isotonic
calibration, and the "shares a root cause" warning fires only when the
calibrated value is at or above the table's threshold: the lowest block whose
validation precision has a Wilson lower bound ≥ 0.80. A table with no such
block has `threshold: null` and the warning stays off. A table that is not
monotone, or whose threshold is not one of its block values, exits 2. Without
the flag the output is unchanged.

`--prose-anchors` runs a second extraction pass before a text would abstain on
`no-anchor`: bare calls outside backticks (`rageQuit()`), a code-like name
before `function` or `modifier` (`the batchMigrateFollows function`), and line
references (`line 117`, `lines 12-34`, `L117-L120`) bound to the one `.sol`
file the same text cites. The new citations go through the same resolver, so
an unknown or ambiguous name still abstains. It applies to the finding and to
every prior item with no anchor, including at `--prepare`. A text that already
resolves an anchor is unchanged, and without the flag the output is unchanged.

## init

Writes `program.yaml`, `scope.md`, `known-issues.md` and `audits/README.md`
into the directory (default: the current directory). Refuses to overwrite an
existing file without `--force`. `build` reads the scaffold without edits.

```text
$ priorart init my-program
priorart 0.2.1 - scaffolded my-program

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

