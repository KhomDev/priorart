# Licensing

Two licences, by artefact:

| Artefact | Licence |
|---|---|
| Rust code under `src/`, `tests/`, `scripts/` | MIT (see `LICENSE`) |
| The rejection dataset under `data/rejections/` | CC BY 4.0 (see `data/rejections/LICENSE-DATA`) |
| The vocabularies under `vocab/` | MIT, moved from bounty-economics (MIT) |
| Docs under `docs/` and the README | MIT |
| The dossier files **you** build | yours; they contain excerpts of documents you supplied |

## Attribution

If you redistribute the rejection dataset, cite it as
`data/rejections/CITATION.cff` says: the rejection taxonomy by KhomDev,
version 0.6.1, CC BY 4.0. The dataset carries no third-party program data, so
there is nothing else to clear.

## What priorart does not redistribute

- No program names, reward tables, known-issue text or audit text is bundled.
  Those live only in the dossier you build from files you saved.
- The public benchmark stores pointers and labels (`repo@commit` plus issue
  number), never finding text. See [benchmark.md](benchmark.md).
- The private classifier, ledger and blocklists are not in this repository.

## Dossier contents

A dossier quotes short excerpts of the documents you supplied and records their
SHA-256. It is a private working file unless you choose to share it: an audit
excerpt is still the audit firm's text, and a program's known-issues list is
still the program's. The build prints a note for every source it skips, and it
copies the documents nowhere else.
