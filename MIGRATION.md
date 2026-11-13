# Migrating from viability-gate to priorart

The viability-gate repository was renamed to **priorart**. GitHub redirects the
old URLs, but a crate cannot be renamed on crates.io, so the `viability-gate`
crate is frozen instead of deleted. The final release is **0.9.0**: the README is
a deprecation notice pointing at priorart and realism-gate, and the binary prints
a one-line stderr notice. **It still installs and it still works. Nothing was
yanked, moved or unpublished.**

The short version: the prose anti-pattern engine, the ecosystem packs and the
0-100 score are gone. What survives is the part that quoted evidence: the
importer, the identifier matcher, G1-G4, `init`, and the payout and exclusion
vocabularies.

## The migration table, row for row

| If you used | Now use | What changed |
|---|---|---|
| `vg check` anti-patterns / score | — | Removed: it didn't separate paid from unpaid (13 in 25) |
| `vg` known-issue match (D1, P18) | `priorart check` | Ranked prior items with quotes; never says "duplicate" |
| `vg` gates G1-G4, `vg import`, `vg init` | `priorart build / check / init` | Same inputs, evidence instead of pass/fail |
| `vg` citation anchor | `realism-gate check --target --target-commit` (RG20) | One implementation |
| `vg` claim notes P23-P26, P19 | `realism-gate check --report` | Claims bound to PoC evidence |
| bounty-economics taxonomies, ledger schema v1 | `priorart` `vocab/`, `priorart ledger validate` | Schema unchanged |
| rejection-taxonomy CSV/JSON | `priorart/data/rejections/` | Same rows, same columns, same licence |
| realism-gate 0.x | realism-gate 1.0 | Same rule ids; adds `--report` |

## What each row means in practice

**Anti-patterns and the score.** P1-P26, `rules/anti-patterns.yaml`,
`rules/packs/`, the 0-100 estimate and the KILL/ROUTE-TO-VERIFY/CLEAR outcome
enum are deleted in priorart. The 0.8.1 changelog already recorded why: 12 of the
25 hard kills were paid findings, so the kill's precision was 13/25 (52%) that a
finding was unpaid. priorart keeps no score and returns no verdict.

**Known-issue matching.** `vg check --known-issues` and `--index` become
`priorart check --dossier`. The output is ranked prior items, each with its
source, page, section and verbatim quote, plus the identifiers it shares, and it
ends with a reading-list note. 0.1.0 ships retrieval-only: it prints no duplicate
warning (the final false-alarm rate is above the plan's trigger; see
[docs/benchmark.md](docs/benchmark.md)). The old D1 regex corpus and the 0.50
duplicate score are gone.

**Gates, import and init.** `vg import audit|program|prior` becomes the single
`priorart build <program-dir>`: save the page, known issues and audit files in the
folder, and build one deterministic dossier. `vg init` becomes `priorart init`.
G1-G4 keep their questions but report quoted evidence, never pass/fail.

**Citation anchor.** The check that a cited file and line exist at a pinned
commit lives in realism-gate now (`--target` and `--target-commit`, rule RG20).
priorart reads the files you saved into the dossier and does not check a commit.

**Claim notes.** P23-P26 and P19 become the claim table in
`realism-gate check --report` (and `--claims` for structured claims).

**Taxonomies and ledger.** The 5 tier shapes, the 14 clause classes and the
gate cross-reference are compiled into priorart (`vocab/`, MIT, moved from
bounty-economics v0.6.1; tier shapes and clause classes are byte-identical).
The private ledger schema is unchanged at v1 (byte-identical to bounty-economics
v0.6.1) and `priorart ledger validate` is its validator. Existing ledgers do not
need editing.

**Rejection dataset.** `priorart/data/rejections/` carries the same 113 rows, 10
columns, `summary.json` keys and CC BY 4.0 licence as rejection-taxonomy v0.6.1.
It is one researcher's log with no positive class; it is a vocabulary and a
descriptive layer, not evidence.

**realism-gate.** Rule ids do not change; 1.0 adds report reading and claim
binding. See the realism-gate documentation in its own repository.

## What still works, so you can migrate on your own schedule

- `cargo install viability-gate --version 0.9.0` still resolves. The crate is
  not yanked. Run it and you get the old behaviour plus a one-line stderr notice
  pointing here.
- Old repository URLs redirect to priorart; issues and history are kept.
- Old tags and release artifacts remain downloadable.
- The moved data files keep their licences and their provenance files.

## What to change first

1. Replace `vg init` with `priorart init` and `vg import ...` with one
   `priorart build`.
2. Replace `vg check` with `priorart check`; if you scripted `--fail-on-kill`,
   there is no equivalent, because priorart has no kill. Use
   `--fail-on-excluded` or `--fail-on-unpayable` only if you want an exit code.
3. Move citation checking to `realism-gate check --target --target-commit`.
4. Delete the score from any dashboard. A quote with a page number replaced it.
5. Keep your ledger files. Re-validate them with `priorart ledger validate`.

## Not claimed

This migration does not make old results comparable to new ones. The score, the
rules and the outcomes are gone, so a `CLEAR` from vg 0.8 is not a priorart
result. The priorart benchmark measures the new matcher
([docs/benchmark.md](docs/benchmark.md)); it does not re-score vg history.
