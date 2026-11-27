# priorart

**Has this finding been found before, is it excluded, and can it be paid?**
![priorart demo](docs/assets/priorart-demo.gif)
priorart answers those three questions with quoted evidence and page numbers,
and never with a verdict. It is offline and deterministic: everything it reads
is a file you already saved.

```text
priorart check finding.md --dossier example-lend.dossier

HAS THIS BEEN FOUND?  1 prior item to read (shares >=2 symbol identifiers)
  [audit: internal-review.md p.1, section M-03 Fee index rounding]  "The consistency check in `FeeLedger.distributeFees` ..."
     shared: FeeLedger.distributeFees, FeeAccountingMismatch
  -> reading list only. priorart does not decide duplicates; see docs/benchmark.md
IS IT EXCLUDED?       no clause class matches your preconditions (attacker: any user; oracle: none)
CAN IT BE PAID?       tier shape: medium_and_above (pays critical/high/medium, floor medium); High is payable
```

That block is illustrative. The generated block further down is produced from
the committed fixture by `scripts/regen-readme-examples.sh` and checked by
`tests/docs.rs`, so it cannot drift.

> **Research:** [I measured my own audit tools against real exploit proofs, fixed what broke, and measured again](https://github.com/KhomDev/realism-gate/blob/main/docs/research/2026-11-measure-fix-remeasure.md) (November 2026): both tools on judged contest data, the failed 0.3.0 alarm redesign, and why the warning stays off.

## Status: retrieval-only (0.2.2; matcher unchanged since 0.1.3)

**P1-C (2026-11-25): the 0.3.0 alarm redesign was measured and rejected.** A
shared claim component plus an absolute document frequency does not lower the
Code4rena false-alarm rate (Maia 8/15 = 53.3%, Panoptic 6/11 = 54.5%) and the
two sealed corpora are 3/9 = 33.3% [12.1-64.6] each. It also destroys duplicate
recall (Maia 87/289 -> 7/289, Panoptic 38/80 -> 3/80, Party 0/132). The
0.3.0 matcher was reverted; **0.2.2 ships the harness fixes only**, and the
unchanged matcher re-measures materially as P1 (Tapioca 15/60 = 25.0% FAR,
35/64 = 54.7% duplicate recall; Maia 8/15 and Panoptic 6/11 unchanged; the
formerly sealed pairs 3/9 = 33.3% and 4/9 = 44.4%, 7/18 = 38.9% pooled). The
warning stays off. Tables: [docs/benchmark.md](docs/benchmark.md) and
`eval/benchmark/BENCHMARK.md`.

**On two Code4rena-judged corpora the matcher's false-alarm rate is above the
30% trigger:** 14/26 = 53.8% (Wilson 95% CI 35.5-71.2) pooled over Maia and
Panoptic, the first time an interval lies wholly above it. The duplicate
warning stays off, and the alarm rule (any prior item sharing >= 2 strong
identifiers) is the next thing to fix. Details and the one harness defect these
corpora exposed are in [docs/benchmark.md](docs/benchmark.md).

priorart ranks prior items to read. It does **not** warn that a finding is a
duplicate. The plan's kill criterion is a 30% false-alarm rate on judged unique
findings. 0.1.3 changes the specificity denominator (the share is counted over
items that share a symbol with the finding) and re-measures **15/60 = 25.0%**
(Wilson 95% CI 15.8-37.2) on the same corpus and seed. That is below the
criterion, but the warning still ships off: the interval reaches 37.2%, the
corpus is one contest pair, and its held-out half was opened for 0.1.0, so this
is a re-measurement rather than a fresh out-of-sample result. **0.2.0 changes
no matcher, gate, schema or vocabulary code**, so the 0.1.3 numbers below
describe this release. A second corpus on PoolTogether V5 was frozen and
measured on the 0.2.0 binary: **9/40 = 22.5%** (CI 12.3-37.5) false alarms and
**32/85 = 37.6%** (CI 28.1-48.3) duplicate recall@5, with 0/2 known-prior hits
(not a rate).

Measured benchmark (0 tool errors):

| Corpus | Metric | Result | Wilson 95% CI |
|---|---|---|---|
| Tapioca, 0.1.3 `be373973` | False-alarm rate, 60 judged paid+unique | **15/60 = 25.0%** | 15.8-37.2% |
| Tapioca, 0.1.3 `be373973` | Duplicate recall@5, 64 judged duplicates | **36/64 = 56.3%** | 44.1-67.7% |
| Tapioca, 0.1.3 `be373973` | Known-prior recall@5 | n=1, 0 hits | not a rate |
| PoolTogether, 0.2.0 `06fdf195` | False-alarm rate, 40 judged paid+unique | **9/40 = 22.5%** | 12.3-37.5% |
| PoolTogether, 0.2.0 `06fdf195` | Duplicate recall@5, 85 judged duplicates | **32/85 = 37.6%** | 28.1-48.3% |
| PoolTogether, 0.2.0 `06fdf195` | Known-prior recall@5 | n=2, 0 hits | not a rate |
| PoolTogether, 0.2.2 `b9459878` | Known-prior recall@5 (heading identifiers) | n=2, 1 hit | not a rate |
| Maia (Code4rena-judged), 0.2.0 `06fdf195` | False-alarm rate, 15 judged paid+unique | **8/15 = 53.3%** | 30.1-75.2% |
| Maia (Code4rena-judged), 0.2.0 `06fdf195` | Duplicate recall@5, 289 judged duplicates | **87/289 = 30.1%** | 25.1-35.6% |
| Panoptic (Code4rena-judged), 0.2.0 `06fdf195` | False-alarm rate, 11 judged paid+unique | **6/11 = 54.5%** | 28.0-78.7% |
| Panoptic (Code4rena-judged), 0.2.0 `06fdf195` | Duplicate recall@5, 80 judged duplicates | **38/80 = 47.5%** | 36.9-58.3% |
| Maia + Panoptic pooled (preregistered) | False-alarm rate, 26 | **14/26 = 53.8%** | 35.5-71.2% |
| Maia + Panoptic pooled (preregistered) | Known-prior recall@5 | n=4, 0 hits | not a rate |

Every run is in [docs/benchmark.md](docs/benchmark.md). The Tapioca 0.1.0
number of record was 26/60 = 43.3% [31.6-55.9] false alarms and
39/64 = 60.9% [48.7-71.9] duplicate recall; before it, 30.0% and 25.0%. The
0.1.3 denominator change gives up some duplicate recall (39/64 to 36/64) to
remove false alarms that rested on symbols shared by most of the relevant
items. Both Sherlock-judged corpora put the false-alarm point estimate below
the 30% trigger with intervals that include 30%; the Code4rena-judged corpora
put it above, with Maia's and the pooled interval wholly above. The duplicate
warning stays off.

## Install

```sh
cargo install priorart        # or: cargo binstall priorart
priorart --version
```

No API key and no data directory: the vocabularies and the rejection dataset
are compiled into the binary. `priorart fetch` is the only command that opens
a socket. Rust 1.95 or newer.

## Quickstart

```sh
# 1. Scaffold the folder priorart reads, once per program.
priorart init my-program
#    then save the program page as scope.md (or scope.html/scope.htm), add known issues
#    and audit PDFs/markdown under audits/, and fill in program.yaml.

# 2. Build the dossier. This is the product: one indexed local file per program.
priorart build my-program

# 3. Run every finding against it.
priorart check finding.md --dossier my-program/my-program.dossier
```

## The generated example

<!-- BEGIN GENERATED EXAMPLE (scripts/regen-readme-examples.sh) -->
$ priorart build examples/dossier-demo
priorart 0.2.2 - dossier dossier-demo (examples/dossier-demo/dossier-demo.dossier)
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

$ priorart check examples/finding-demo.md --dossier examples/dossier-demo/dossier-demo.dossier
priorart 0.2.2 - examples/finding-demo.md against dossier dossier-demo (examples/dossier-demo/dossier-demo.dossier)

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

NOTES
  - G4 prior audits: Internal review covers src/LoanBook.sol (cited identifier LoanBook), src/FeeLedger.sol (cited identifier FeeLedger) - read that report's findings for this function
  - G3 scope: the finding cites scoped code: LoanBook (in src/LoanBook.sol), FeeLedger (in src/FeeLedger.sol), SwapDesk (in src/SwapDesk.sol)
  - priorart does not decide duplicates, exclusion, or payment. Read the quoted lines.
<!-- END GENERATED EXAMPLE -->

The fixture is [examples/dossier-demo](examples/dossier-demo): a fictional
program with a rewards table, a scope list, two known issues and one prior
audit. Every URL in it is reserved (`example.invalid`), and no real program,
audit or finding is described.

### HTML program pages, generated

The second fixture is `tests/fixtures/html-program`: a saved page whose
rewards, assets and exclusions are HTML lists, with a `<script>` and a
`<style>` that must never be quoted. This block is generated from it by the
same script and checked by `tests/docs.rs`.

<!-- BEGIN GENERATED HTML EXAMPLE (scripts/regen-readme-examples.sh) -->
$ priorart build tests/fixtures/html-program
priorart 0.2.2 - dossier html-program (tests/fixtures/html-program/html-program.dossier)
  1 source(s), 7 item(s)
    payout-tier      3
    scope-clause     2
    scope-path       2
  [src-0001] program-page  scope.html  p.1  sha256 27540f87d255
  note: 1 source(s) indexed (0 audit(s)); 7 item(s) extracted
Next: priorart check <finding.md> --dossier tests/fixtures/html-program/html-program.dossier

$ priorart check tests/fixtures/html-finding.md --dossier tests/fixtures/html-program/html-program.dossier
priorart 0.2.2 - tests/fixtures/html-finding.md against dossier html-program (tests/fixtures/html-program/html-program.dossier)

HAS THIS BEEN FOUND?  nothing ranked (no prior item shares >=2 symbol identifiers)

IS IT EXCLUDED?       1 clause class match your preconditions (attacker: role: operator; oracle: none)
  [program-page: scope.html p.1, section Out of scope]  "Attacks requiring access to privileged roles"
     clause class: privileged_role   your precondition: the finding needs a privileged role (operator), and the clause excludes those
  -> a match is a prompt to read the clause, not a reject.

CAN IT BE PAID?       tier shape: medium_and_above (pays critical/high/medium, floor medium); High is payable (cited: https://example.invalid/html/rewards, section Rewards)
  "High: 10,000 USD"
  High is at or above the floor (Medium)

NOTES
  - G4 prior audits: no prior-audit surface in this dossier
  - G3 scope: the finding cites scoped code: Vault (in src/Vault.sol), FeeLedger (in src/FeeLedger.sol)
  - no prior item shares two symbol identifiers with the finding. That is not evidence of novelty: the dossier only holds what you saved.
  - priorart does not decide duplicates, exclusion, or payment. Read the quoted lines.
<!-- END GENERATED HTML EXAMPLE -->

## What the three sections mean

- **Has this been found?** Prior items that share at least two *symbol*
  identifiers with your finding: function, error, contract or qualified names,
  or backticked spans. A file path alone never qualifies an item, and a symbol
  that appears in more than 20% of the items sharing a symbol with the finding
  is treated as generic. Results are ranked by specificity. This is a reading
  list, not a duplicate warning: see the status section above.
- **Is it excluded?** The program's own out-of-scope clauses, classified into
  14 clause classes, against the preconditions the finding states. A match is a
  prompt to read the clause, not a reject.
- **Can it be paid?** Your claimed severity against the program's payout tier
  shape and floor, transcribed into `program.yaml`. An unread or uncited
  reward table reports no result; unknown is never "payable".

Nothing here is a pass. Each section can say it could not run, and that is not
the same as clear.

## The dossier

`priorart build` reads a folder you saved and writes one deterministic JSON file:

```text
my-program/
  program.yaml        # optional: name, platform, urls, payout, prior_audits
  scope.md            # the saved program page (.html/.htm convert; .txt too)
  known-issues.md     # the program's own known issues (.yaml and .txt too)
  audits/*.md|*.txt|*.pdf
```

A saved `scope.html` (or `scope.htm`) is converted to text by a small in-repo
normalizer: headings become `#` lines, `<li>` becomes a bullet, block elements
become line breaks, other tags are stripped, `<script>`/`<style>` content is
dropped, and common entities are decoded. It is designed for **list-based
pages** (a heading plus a `<ul>`/`<li>` list). It is not a browser: CSS,
JavaScript, attribute-only text and **table structure** are not read. A plain
rewards or scope table yields no payout/scope items, though a cell whose text is
itself a Markdown bullet (`- ...`) is still read as one. Save a table-based page
as Markdown. Table mapping is a later-release plan, not 0.2.0. The behavior and
its limits are in [docs/dossier.md](docs/dossier.md#html-program-pages).

Inputs must be regular files. A symlink, FIFO, device or directory named as an
input is refused or skipped by name, never read through: `program.yaml`, a
finding and an explicit dossier are required and refused with exit 2, while an
optional `scope.*`/`known-issues.*` name, an audit (including a symlinked
subdirectory of `audits/`), or an `audits/` directory is skipped with a note
that names it (and the build fails if no regular input is left). A dossier
discovered beside the finding follows the same rule: a non-regular only
candidate is exit 2, and a regular candidate beside it is used instead. The
check is a stat followed by a separate open, so a concurrent writer can swap the
path between them; it refuses an accidental or stale link, not a writer who
already controls the folder.

Output: `my-program/<program-id>.dossier`, schema `priorart.dossier/v1`
([schemas/dossier.schema.json](schemas/dossier.schema.json)). Two builds over the
same folder are byte-identical; a test asserts it. Format reference:
[docs/dossier.md](docs/dossier.md).

### PDFs and the native parser

An uncompressed PDF is scraped in-process. For a compressed PDF the fallback is
`pdftotext` from poppler: a **native third-party parser started as the same
user, with the same privileges, and without a sandbox**. It receives the PDF by
absolute path and runs under a 60-second and 16 MiB output limit; it is found
only on absolute `PATH` entries. `priorart build --no-pdftotext` never starts
it, and a PDF the built-in scrape cannot read is then reported unread and
skipped. See [docs/limitations.md](docs/limitations.md) and
[ARCHITECTURE.md](ARCHITECTURE.md#trust-boundaries).

## Ledger, vocabularies, dataset

```sh
priorart ledger validate my-ledger.yaml      # schema v1, the bounty-economics contract
priorart vocab list                          # 5 tier shapes, 14 clause classes
priorart vocab show clause-classes
priorart explain privileged_role             # or a gate id, or a tier shape
priorart rejections summary                  # the shipped 113-row dataset
priorart rules                               # the whole check surface on one page
```

The ledger schema is unchanged from bounty-economics schema v1: an uncited entry
is an error, an unstated floor is not "Low pays", and a removed `any` shape gets
a migration message rather than a bare enum failure. See
[docs/ledger.md](docs/ledger.md) and [docs/schema.md](docs/schema.md).

The rejection dataset is moved from rejection-taxonomy v0.6.1 with rows,
columns, licence and `summary.json` keys unchanged. It is **one researcher's
log**: no positive class, no target dimension, n=113 with 3 withheld rows. It is
the vocabulary behind the mechanism hints and the descriptive layer, never the
headline evidence. See [docs/rejections.md](docs/rejections.md).

## Benchmark status

Public, multi-researcher corpus: judged contests that publish prior audits and
known issues, stored as pointers and labels only. Protocol, every run and the
ship decision: [docs/benchmark.md](docs/benchmark.md). The numbers are stated
once there, with n and Wilson 95% intervals, so there is one place to update and
one place to be wrong.

## What priorart is not

- It is not a duplicate detector. It ranks what to read and never says two
  findings are the same.
- It is not a payout oracle. It compares your claimed severity with a table you
  transcribed.
- It is not a proof checker. Whether the PoC proves the claim is
  [realism-gate](https://github.com/KhomDev/realism-gate)'s question.
- It is not a crawler. `priorart fetch` is the one command that opens a socket:
  the HTTPS URLs you name, one GET each, no cache and no refresh. Every other
  command reads files ([docs/no-network.md](docs/no-network.md)).

## Migrating from viability-gate

The prose anti-pattern engine (P1-P26), the ecosystem packs and the 0-100 score
are deleted: they did not separate paid from unpaid findings (a hard kill was
right 12 times in 25). Gates G1-G4, the importer, the matcher and `init` moved
here as evidence. The row-for-row mapping is in [MIGRATION.md](MIGRATION.md).

## Docs

| Doc | What it covers |
|---|---|
| [docs/cli.md](docs/cli.md) | every command, flag and exit code |
| [docs/dossier.md](docs/dossier.md) | the program folder and `priorart.dossier/v1` |
| [docs/matching.md](docs/matching.md) | the identifier rule, specificity, false alarms |
| [docs/gates.md](docs/gates.md) | G1-G4 as quoted evidence |
| [docs/ledger.md](docs/ledger.md) | ledger schema v1 validation |
| [docs/schema.md](docs/schema.md) | the three shipped JSON schemas |
| [docs/vocab.md](docs/vocab.md) | 5 tier shapes, 14 clause classes, their sources |
| [docs/rejections.md](docs/rejections.md) | dataset provenance, licence, one-person-log caveat |
| [docs/benchmark.md](docs/benchmark.md) | corpus, protocol, measured results |
| [docs/validation.md](docs/validation.md) | every public claim, its evidence, its status |
| [docs/limitations.md](docs/limitations.md) | what this cannot do |
| [docs/install.md](docs/install.md) | install and release artifacts |
| [docs/licensing.md](docs/licensing.md) | MIT code, CC BY 4.0 data |

## Licence

Code MIT. The rejection dataset under `data/rejections/` is CC BY 4.0; see
[`data/rejections/LICENSE-DATA`](data/rejections/LICENSE-DATA) and
[`docs/licensing.md`](docs/licensing.md).
