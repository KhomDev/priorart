# The dossier: folder layout and format

`priorart build` turns a folder you saved into one JSON file. The dossier is the
product: build it once per program, run every finding against it. Nothing is
fetched at build time or check time.

## Input layout

```text
program-dir/
  program.yaml         # optional metadata and payout terms
  scope.md             # the saved program page (.html/.htm convert; .txt too)
  pages/               # further pages, from `priorart fetch` or saved by hand
  fetch.json           # record of what `priorart fetch` saved; not a source
  known-issues.md      # the program's own known issues (.yaml/.yml/.txt too)
  audits/              # prior audit reports, .md/.txt/.pdf, recursive
```

What `build` reads, and the order it reads it in:

- the classic program page: `scope.md`, `scope.html`, `scope.htm`, `scope.txt`
  (first found in that order; if several of those exist the build prints which
  one it used);
- then every regular file in `pages/` with extension `.html`, `.htm`, `.md`,
  `.markdown` or `.txt`, sorted by file name. `priorart fetch` writes
  `pages/001.html`, `pages/002.html`, … in the order of the URLs. A folder
  that has only `scope.md` is read exactly as before. `fetch.json` is not a
  source; `build` reads it only to learn a fetched page's final URL, which is
  the base for relative links in that file;
- exactly one known-issues file: `known-issues.md`, `.yaml`, `.yml`, `.txt`;
- every document under `audits/`, recursively, with extension `.md`,
  `.markdown`, `.txt` or `.pdf`. `audits/README.md` is skipped: scaffolding is
  not a prior audit. Hidden regular files and directories are skipped; every
  symlink (hidden or not, file or directory) and every non-regular document is
  skipped with a note that names it.

Program page, then known issues, then audits by path: that order is the source
order, the item order and therefore the id order. If no input is found at all,
the build is a usage error (exit 2), not an empty dossier.

Every input must be a **regular file**, and a symlink, FIFO, device or
directory named as an input is refused or skipped by name, never read through.
The treatment depends on whether the input is required:

- **Required:** `program.yaml` (for `build`); a finding and an explicit dossier
  (for `check`). A non-regular one is refused with **exit 2** and an error
  naming it.
- **Optional:** `scope.*`, `known-issues.*`, any symlink (file or directory) or
  document under `audits/`, and the `audits/` directory itself. A non-regular
  one is **skipped with a note that names it**. The build exits 2 only when no
  regular input remains.

A dossier **discovered** beside the finding is optional too, with one
exception: when the only `.dossier` candidate is non-regular, discovery is a
usage error (exit 2) naming it; when a regular dossier is also present, the
regular one is used and the non-regular candidate is ignored without a note.
Pass `--dossier` to name a file explicitly.

A FIFO is refused before it is opened, so a build cannot block on one. The
check is a stat followed by a separate open: a concurrent writer can swap the
path between the two calls. It refuses an accidental or stale link, not a
writer who already controls the folder.

## HTML program pages

A saved `scope.html`, `scope.htm`, or a page under `pages/`, is converted to
text before extraction, by `src/dossier/html.rs`:

- `<h1>`-`<h6>` become `#`-`######` heading lines;
- `<li>` becomes a `- ` bullet;
- a `<table>` becomes one bullet per body row. With a header row (`<th>` or
  `<thead>`), each body row is `- <Header1>: <cell1>; <Header2>: <cell2>`, and
  an empty cell is skipped. Without a header row the bullet is `- cell1 | cell2`;
- an `<a href>` with no text, or whose text is not the URL, appends the URL.
  `utm_*` query keys are removed. A relative href is resolved only against an
  explicit base: the `final_url` in `fetch.json` for that file. With no base,
  the relative href is left relative;
- a GitHub repository, tree or blob URL, and an explorer `/address/0x`… or
  `/token/0x`… URL, is a `scope-path` item;
- a block boundary (`<div>`, `<p>`, a heading) ends the previous line. A
  severity word that sits in its own block labels the reward lines that follow
  it, so it is not glued onto the previous amount;
- an impacts row (a severity and a title, and no amount) is an `impact` item.
  A `payout-tier` item has a severity and an amount, or an "up to" / range, on
  the same line;
- `<p>`, `<br>` and the other block elements become line breaks outside a
  table cell. Inside a cell they stay in that cell, and a nested `<li>` is
  joined with `; `;
- every other tag is dropped and its text kept, so `src/<code>Vault</code>.sol`
  stays `src/Vault.sol`;
- `<script>` and `<style>` contents are dropped and never quoted;
- common named entities and numeric references are decoded; an entity this
  build does not know is left as written;
- whitespace is collapsed the way a browser collapses it, except in `<pre>`.

This is **not a browser**. Still not read:

- text that exists only because JavaScript wrote it (a script-only page yields
  no items, and the build says so);
- images, and text that lives only in attributes other than `href` (`alt`,
  `title`, `aria-label`);
- CSS layout.

The source is recorded with one page, its `sha256` is of the HTML bytes, and
every item quotes the converted text.

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
      - src/LoanBook.sol
      - src/FeeLedger.sol
    known_issues:
      - "fee index rounding in the fee accounting check"
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
  "tool": {"name": "priorart", "version": "0.2.0"},
  "program": {"id": "dossier-demo", "name": "Example Lend", "platform": "ExamplePlatform",
              "url": "https://example.invalid/lend",
              "scope_url": "https://example.invalid/lend/scope",
              "payout": {"tier_shape": "medium_and_above", "floor_status": "documented",
                         "floor_severity": "medium", "source_url": "https://example.invalid/lend/rewards",
                         "retrieved": "2026-09-01", "section": "Rewards",
                         "quote": "High: 10,000 USD"},
              "prior_audits": [{"firm": "Internal review", "url": "https://example.invalid/lend/internal-review",
                                "covers": ["src/LoanBook.sol", "src/FeeLedger.sol"],
                                "known_issues": ["fee index rounding in the fee accounting check"]}]},
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
  `scope-clause`, `scope-path`, `payout-tier`, `impact`, `page-section`.
  `impact` is a severity-tagged in-scope impact that is not a payout tier.
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

A literal is decoded as UTF-16BE or UTF-16LE when it starts with a byte-order
mark, as UTF-8 when it is valid UTF-8, and byte-per-character only as a last
resort. Before 0.2.0 every byte was read as a Latin-1 character, so a UTF-16
literal came out interleaved with NUL characters; the code now decodes it, and a
test uses a UTF-16BE literal (`a_utf16be_pdf_literal_is_decoded`).

### The native parser is a trust boundary

`pdftotext` is poppler: a **native third-party parser**, started by priorart as
the same user, with the same privileges, and **without a sandbox**. The PDF it
reads is an untrusted saved file. priorart contains it with the absolute-path
lookup and the time/output limits above, not with isolation.

`priorart build --no-pdftotext` never starts it. With that flag, a PDF the
built-in scrape cannot read is reported unread and skipped, exactly like the
missing-`pdftotext` case:

```text
note: <path>: no extractable text (the uncompressed scrape found nothing and
pdftotext is disabled with --no-pdftotext). Convert it with a text tool and pass
the .txt. - source skipped
```

`pdftotext` reads a file that may be hostile, so it runs under three limits:

- **Found on `PATH`, absolute entries only.** priorart looks the executable up
  itself. A relative `PATH` entry is skipped, and on Windows the current
  directory is never searched.
- **The PDF is passed by absolute path**, after `-layout -q`, so a file name that
  starts with `-` cannot be read as an option.
- **Time and output are bounded.** The child is killed after 60 seconds, or once
  it has written 16 MiB of text. The source is then skipped with a warning and
  the build still exits 0:

```text
note: <path>: pdftotext did not finish within 60 s; killed - source skipped
note: <path>: pdftotext wrote more than 16 MiB of text; killed - source skipped
```

## Input limits

A build reads at most these, and refuses a folder over one with **exit 2** before
it reads the files. The error names the limit and the file or folder that broke
it:

| Limit | Value | Error names |
|---|---|---|
| Size of one input file (`program.yaml` and every document) | 64 MiB | the file and its size |
| Documents read from one folder (scope, known issues and every audit) | 1000 | the count |
| Total size of the documents | 512 MiB | the total |
| Directory depth under `audits/` | 16 folders | the folder |
| Directory entries visited under `audits/`, files and folders | 10 000 | the limit |

The values are constants in `src/dossier/import.rs` (`MAX_INPUT_BYTES` and the
others). They are not command-line flags: a larger audit should be converted to
text or split into smaller program folders.

`check` has its own limits, checked before the file is read:

| Limit | Value | Error names |
|---|---|---|
| Finding file | 16 MiB (`check::MAX_FINDING_BYTES`) | the file and its size |
| Dossier file | 64 MiB (`dossier::load::MAX_DOSSIER_BYTES`) | the file and its size |

A sparse or accidental huge file is refused by size, not read and then judged.
A **build of this version whose serialized dossier would exceed 64 MiB is exit 2
before the file is written**, with an error that says to split the program
folder; a dossier this build writes can therefore be read by `check`. A
pre-0.2.0 dossier over the cap (0.1.3 had no limit) is still refused.

## What `build` does not do

- **`build` does not fetch anything.** A page is a file, saved by you or by
  `priorart fetch`. There is no cache and no refresh
  ([no-network.md](no-network.md)).
- It does not check out a repository at a commit, and it does not parse source
  code. It reads documents.
- It does not render HTML in a browser. `scope.html`/`scope.htm` and `pages/*`
  are converted ([above](#html-program-pages)). JavaScript-rendered text,
  images and attributes other than `href` are not read.
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
