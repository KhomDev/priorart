# The dossier: folder layout and format

`priorart build` turns a folder you saved into one JSON file. The dossier is the
product: build it once per program, run every finding against it. Nothing is
fetched at build time or check time.

## Input layout

```text
program-dir/
  program.yaml         # optional metadata and payout terms
  scope.md             # the saved program page (.html and .txt also work)
  known-issues.md      # the program's own known issues (.yaml/.yml/.txt too)
  audits/              # prior audit reports, .md/.txt/.pdf, recursive
```

What `build` reads, and the order it reads it in:

- exactly one program page: `scope.md`, `scope.html`, `scope.txt` (first found
  in that order; if several exist the build prints which one it used);
- exactly one known-issues file: `known-issues.md`, `.yaml`, `.yml`, `.txt`;
- every document under `audits/`, recursively, with extension `.md`,
  `.markdown`, `.txt` or `.pdf`. `audits/README.md` is skipped: scaffolding is
  not a prior audit. Dot-files and symlinks are skipped.

Program page, then known issues, then audits by path: that order is the source
order, the item order and therefore the id order. If no input is found at all,
the build is a usage error (exit 2), not an empty dossier.

## program.yaml

Unknown keys are ignored on purpose: real engagement notes grow fields, and
dropping the file over a key this build does not read would be worse. The keys
that matter:

| Key | Type | Read by |
|---|---|---|
| `id` | slug | default dossier filename and `program_id` (default: folder name, slugified) |
| `name`, `platform`, `url`, `scope_url` | strings | recorded in the dossier |
| `payout` | mapping | G1 ([gates.md](gates.md)) |
| `prior_audits` | list | G4 ([gates.md](gates.md)) |

```yaml
id: sample-vault
name: Sample Vault
platform: example-platform
url: https://example.invalid/sample-vault
scope_url: https://example.invalid/sample-vault/scope

payout:
  tier_shape: medium_and_above   # one of the five shapes; required if payout is present
  floor_status: documented       # documented | unstated | conflicting
  floor_severity: medium         # critical | high | medium | low
  source_url: https://example.invalid/sample-vault/rewards
  retrieved: "2026-11-01"
  section: Rewards               # optional; filled from the page when absent
  quote: "Medium: 2,000 USD"     # optional; filled from the page when absent

prior_audits:
  - firm: Internal review
    url: https://example.invalid/audit
    covers:                      # the files/contracts the report claims to cover
      - src/VaultEngine.sol
      - src/BalanceSheet.sol
    known_issues:
      - "shortfall via direct ledger writes in the reserve backing check"
```

- `payout.tier_shape` outside the five shapes is a **build error** listing the
  valid values. `floor_status` and `floor_severity` are validated the same way.
- A payout block without `source_url` builds, but G1 reports the terms as
  uncited and returns no payable result. The build prints a note about it.
- `covers` is what G4 matches a cited path or contract name against; an audit
  with no `covers` list reports that coverage could not be read, not "not
  covered".
- `quote` and `section` are optional. When absent, the build fills them from a
  `payout-tier` item whose text names the declared `floor_severity`, so the
  citation is always the page you saved.

## Output: `priorart.dossier/v1`

```json
{
  "schema": "priorart.dossier/v1",
  "tool": {"name": "priorart", "version": "0.1.0"},
  "program": {"id": "dossier-demo", "name": "Rain", "platform": "HardenedProof",
              "url": "https://example.invalid/rain",
              "scope_url": "https://example.invalid/rain/scope",
              "payout": {"tier_shape": "medium_and_above", "floor_status": "documented",
                         "floor_severity": "medium", "source_url": "https://example.invalid/rain/rewards",
                         "retrieved": "2026-09-01", "section": "Rewards",
                         "quote": "High: 10,000 USD"},
              "prior_audits": [{"firm": "Internal review", "url": "https://example.invalid/rain/internal-review",
                                "covers": ["src/VaultEngine.sol", "src/BalanceSheet.sol"],
                                "known_issues": ["shortfall via direct ledger writes in the reserve backing check"]}]},
  "sources": [{"id": "src-0001", "kind": "program-page", "title": "scope.md",
               "path": "scope.md", "sha256": "<64 hex>", "pages": 1}],
  "items": [{"id": "item-0001", "source_id": "src-0001", "kind": "payout-tier",
             "title": "scope.md", "section": "Rewards", "page": 1,
             "quote": "Critical: 50,000 USD", "identifiers": [],
             "mechanism_class": null, "clause_class": null,
             "text": "Critical: 50,000 USD"}]
}
```

Schema: [`schemas/dossier.schema.json`](../schemas/dossier.schema.json). Every
key is always present; a value that does not exist is `null`, so a consumer
never has to tell "absent" from "null".

- `sources[].id` is `src-NNNN` in read order. `kind` is `program-page`,
  `known-issues` or `audit`. `path` is relative to the program folder with
  forward slashes; `title` is the file name.
- `sources[].sha256` is the SHA-256 of the file's **bytes** (implemented in-tree,
  checked against the NIST vectors). `pages` is 1 + the number of form feeds,
  which is what `pdftotext -layout` writes between pages.
- `items[].kind` is one of `audit-finding`, `audit-section`, `known-issue`,
  `scope-clause`, `scope-path`, `payout-tier`, `page-section`.
- `items[].quote` is the sentence that names a strong identifier, whitespace
  collapsed and windowed at a word boundary if it is very long. It is not a
  caption: it is the text the reader will check.
- `items[].identifiers` are the strong identifiers found in the item (symbols and
  paths; [matching.md](matching.md) says which count toward a match).
- `items[].clause_class` is set for `scope-clause` items only, from the 14 clause
  classes; `null` means the clause text matched no class and is never matched.
- `items[].mechanism_class` is a lexical hint ([matching.md](matching.md)); it is
  `null` when the text is negated or unrecognised, never a guess.

## Determinism

The dossier holds no wall-clock field, no absolute path and no machine name.
Sources are ordered, items are ordered, ids are assigned from those orders. Two
builds over the same folder are byte-identical, and
`tests/cli.rs::build_writes_one_deterministic_dossier` asserts it.

## PDFs

An uncompressed PDF is scraped for text literals. If that finds fewer than 40
alphanumeric characters, `pdftotext -layout` is used when it is installed.
Without it the source is **skipped with a warning and the build still exits 0**:

```text
note: <path>: no extractable text (the uncompressed scrape found nothing and
pdftotext is not installed). Convert it with pdftotext -layout and pass the .txt.
- source skipped
```

A scanned (image-only) PDF has no text at all and needs the same conversion; OCR
is not attempted. On Windows without `pdftotext`, that warning is the expected
outcome for a compressed PDF, not a failure.

## What `build` does not do

- **It does not fetch anything.** A page is a file you saved. There is no URL
  argument, no cache and no update check ([no-network.md](no-network.md)).
- It does not check out a repository at a commit, and it does not parse source
  code. It reads documents.
- It does not infer a severity, a floor or a clause class from prose. A payout
  terms block comes from `program.yaml`; a clause class comes from the vocabulary
  ([vocab.md](vocab.md)).
- It does not fail the build over one unreadable input: a PDF it cannot read is a
  note and a skipped source, because a missing tool must not cost you the dossier.

## Cost

The dossier is linear in the size of the inputs you saved. The committed demo
([examples/dossier-demo](../examples/dossier-demo), 3 sources) is 13 items; a
60-file audit set is a few hundred. `check` is then linear in items per query:
there is no separate index or database.
