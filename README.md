# priorart

**Has this finding been found before, is it excluded, and can it be paid?**
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

## Status: 0.1.0 ships retrieval-only

priorart 0.1 ranks prior items to read. It does **not** warn that a finding is
a duplicate: the plan's kill criterion is a 30% false-alarm rate on judged
unique findings, and the release-candidate matcher measured **26/60 = 43.3%**
(Wilson 95% CI 31.6-55.9) - above the criterion, with the interval entirely
above it. The plan pre-committed to this case, so the prior-item section is a
reading list and the duplicate caution is withheld until the matcher is fixed.

Final benchmark, frozen binary sha256 `dabfcc0c` (matcher identical to the deciding `2ec4c3c9` run; 0 tool errors):

| Metric | Result | Wilson 95% CI |
|---|---|---|
| False-alarm rate, 60 judged paid+unique | **26/60 = 43.3%** | 31.6-55.9% |
| Duplicate recall@5, 64 judged duplicates | **39/64 = 60.9%** | 48.7-71.9% |
| Known-prior recall@5 | n=1, 0 hits | not a rate |

Three matcher revisions were measured, and all three runs are in
[docs/benchmark.md](docs/benchmark.md): 18/60 = 30.0% [19.9-42.5] on the first
build, 15/60 = 25.0% [15.8-37.2] after the symbols-only threshold and the
corpus-rarity filter, and 26/60 = 43.3% [31.6-55.9] after identifiers kept their
original case (so short camelCase contract names count) and the specificity
floor and negation window were finalised. Recall rose with each revision
(36/64, 33/64, 39/64); the false-alarm rate did not fall below the threshold, so
the warning ships off. The next release re-measures on a fresh held-out split
before the criterion is re-considered.

## Install

```sh
cargo install priorart        # or: cargo binstall priorart
priorart --version
```

No API key, no network, no data directory: the vocabularies and the rejection
dataset are compiled into the binary. Rust 1.89 or newer.

## Quickstart

```sh
# 1. Scaffold the folder priorart reads, once per program.
priorart init my-program
#    then overwrite scope.md with the saved program page, add known issues and
#    audit PDFs/markdown under audits/, and fill in program.yaml.

# 2. Build the dossier. This is the product: one indexed local file per program.
priorart build my-program

# 3. Run every finding against it.
priorart check finding.md --dossier my-program/my-program.dossier
```

## The generated example

<!-- BEGIN GENERATED EXAMPLE (scripts/regen-readme-examples.sh) -->
$ priorart build examples/dossier-demo
priorart 0.1.1 - dossier dossier-demo (examples/dossier-demo/dossier-demo.dossier)
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
priorart 0.1.1 - examples/finding-demo.md against dossier dossier-demo (examples/dossier-demo/dossier-demo.dossier)

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

## What the three sections mean

- **Has this been found?** Prior items that share at least two *symbol*
  identifiers with your finding: function, error, contract or qualified names,
  or backticked spans. A file path alone never qualifies an item, and a symbol
  that appears in more than 20% of the dossier's items is treated as generic.
  Results are ranked by specificity. This is a reading list, not a duplicate
  warning: see the status section above.
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
  scope.md            # the saved program page (.html and .txt also work)
  known-issues.md     # the program's own known issues (.yaml and .txt too)
  audits/*.md|*.txt|*.pdf
```

Output: `my-program/<program-id>.dossier`, schema `priorart.dossier/v1`
([schemas/dossier.schema.json](schemas/dossier.schema.json)). Two builds over the
same folder are byte-identical; a test asserts it. Format reference:
[docs/dossier.md](docs/dossier.md).

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
known issues, stored as pointers and labels only. Protocol, both runs and the
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
- It is not a fetcher. Program pages are files you saved; priorart never requests
  a URL at runtime ([docs/no-network.md](docs/no-network.md)).

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
