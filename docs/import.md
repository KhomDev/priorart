# `vg import`

`vg import` turns files you already have into the inputs the gates read.
It does not open a socket. A program page is a file you saved. A PDF is a file
on disk.

```bash
vg import audit  audit.yaml     --out target/
vg import program page.html     --out target/     # HTML or markdown
vg import prior   audits/       --out target/     # a file or a directory
```

Each command writes `IMPORT-NOTES.md` listing what it copied and what it did
not find. A missing field stays missing. The tool does not invent a floor, a
date, or a regex.

Existing files are not overwritten without `--force`.

## `audit.yaml`

This is an engagement file, the sort of record a hunt keeps beside a target.
It is not the ledger itself. Import transcribes it into one ledger entry in the
shape of [`data/bounty-economics/schemas/ledger.schema.json`](../data/bounty-economics/schemas/ledger.schema.json),
a snapshot of [bounty-economics](https://github.com/KhomDev/bounty-economics)
v0.5.0 (MIT). Tier shapes and clause classes are the closed lists in
`data/bounty-economics/data/`. A class that is not in that list is named in
`IMPORT-NOTES.md` and left out of the ledger. Import does not pick a class
for you. `schema_version: 1` is written on the entry.

```yaml
version: 1                 # optional; if present, must be 1
id: my-engagement          # required; becomes the ledger id
program: ""                # optional, recorded, not matched on
platform: ""               # optional, recorded in the notes, not a rule signal
severity_floor: medium     # critical | high | medium | low | none
floor_status: documented   # documented | unstated | conflicting
tier_shape: ""             # optional; if set, it is copied and not inferred
source_url: ""             # empty fails G1, on purpose
retrieved: ""              # YYYY-MM-DD; empty fails G1, on purpose
exclusion_clauses:
  - id: excl-1
    class: privileged_role
    text: "Attacks requiring access to privileged addresses"  # verbatim
    # clause: is accepted as an alias of text
    pattern: ""            # optional. Import does not invent one.
known_issues:
  - id: M-1
    summary: "refunds credit only the current segment"
    # text: is accepted as an alias of summary
    source: "contest report, issue M-1"
    patterns: []           # copied if you wrote them
    identifiers: ["reduce_outflow"]  # copied as literal patterns, not paraphrased
scope:
  - src/Vault.sol
severity_rules:            # copied into the notes only
  - tier: high
    text: "direct theft of user funds"
prior_audits:              # the ledger's own shape: firm, url, covers, known_issues
  - firm: ""
    url: ""
    covers: []
    known_issues: []
```

Unknown keys are ignored. Real engagement files grow fields this version does
not read, and rejecting the file for one of them would make import useless.

`severity_floor` maps to a ledger `tier_shape` when you did not set one:

| floor | tier_shape |
|---|---|
| critical | critical_only |
| high | high_and_above |
| medium | medium_and_above |
| low | low_and_above |
| none, empty, anything else | unknown |

`unknown` is what G1 treats as unread, not as permissive.

Outputs, in `--out`:

| File | What it is |
|---|---|
| `ledger.yaml` | one bounty-economics entry |
| `scope.txt` | the scope list, one entry per line |
| `known-issues.yaml` | only the rows that have a pattern. An empty pattern list fails D1, so a row with no pattern is left out and named in the notes |
| `index.yaml` | the same known issues, for identifier matching |
| `IMPORT-NOTES.md` | what was not found |

A known issue with no pattern and fewer than two strong identifiers will not
match anything. The notes say so. Word overlap is not used to fill the gap.

## Saved program pages

`vg import program` reads HTML or markdown. Script and style blocks are
dropped. Headings become markdown headings. Bullets under a heading are copied
when the heading says one of:

| Heading contains | Becomes |
|---|---|
| out of scope, exclusion, prohibited, not eligible | an exclusion clause, verbatim, no pattern |
| known issue, prior audit, previous audit | a known-issue row |
| reward, severity, impacts in scope, payout, bounty | the text the floor is read from |
| scope, and not one of the above | a scope line |

The floor is the lowest of critical / high / medium / low that has a `$` or
`usd` on the same bullet and is not marked n/a, ineligible, or `$0`. "critical
only" is critical. If no such bullet exists, the floor stays unstated.

A saved page has no retrieval date and no source URL unless you add them to
the ledger afterwards. G1 fails until you do. That is the point of the note.

The words immunefi, cantina, and hackenproof, if they occur in the page, are
recorded in the notes as the page family. They do not change the parse. The
parse is the headings above.

## Prior-audit reports

`vg import prior` reads `.md`, `.txt`, and `.pdf`, or a directory of them.
Symlinks are not followed.

PDF text is scraped from uncompressed literal strings. If that scrape finds
almost nothing, the command runs `pdftotext -layout` when it is on `PATH`.
Both are local. A compressed PDF with no `pdftotext` installed is an error
that tells you to convert it yourself. The binary does not link a PDF library
and does not grow a network crate to get one.

A file with two or more issue-style headings (`M-1`, `H-02`, a heading that
starts with "Finding" or "Issue ") is split into one index row per heading.
Anything else is one row.

The index is `index.yaml`. See [duplicate-matching.md](duplicate-matching.md)
for how a draft is scored against it. Quotes in the index are short excerpts.
Do not commit an index of reports you do not have the rights to redistribute.
