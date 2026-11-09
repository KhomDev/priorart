# Method

How the rows were classified, what the classification can and cannot support, and why the dataset
is shaped the way it is.

---

## What a row is

One row = one submission to one public bug-bounty program that was **not paid**.

The private source is a working database maintained while hunting. Each entry records the protocol,
the report title, the triage verdict, the triage reasoning, and a post-hoc analysis of the root
cause and the gate that should have caught it.

The public export reduces each row to an id and nine categorical values. Everything else is
dropped, including the protocol.

## Two stages

**Stage 1 - the private classifier** (not published). It reads the private database and assigns
each row one value from each closed vocabulary in `scripts/vocab.py`:

- `triage_class` - bucketed from the triage cell, first matching rule wins (the rules are listed in
  [data/SCHEMA.md](../data/SCHEMA.md)). Before every build the bucketing is compared, row by row,
  with the private workspace's canonical stats script.
- `earliest_gate` - the gate token at the start of the gate cell.
- `root_cause_class` - mapped from the root-cause cell. That cell is free text and routinely names
  other researchers and upstream findings, so it is never passed through.
- `mechanism_class` - matched from the report title.
- `bug_family` and the family grouping - recorded per row in a private ledger.
- a duplicate signal - where the log says a duplicate came from.

Its rules quote private text: a pattern that recognises a mechanism in a report title has to
contain words from that title. That is why the classifier is private. The ledger keys every row by
a hash of its protocol and title, so adding or reordering rows in the private database cannot shift
a classification onto the wrong row, and an edited title stops the build until the ledger is
updated.

**Stage 2 - the public build** (`scripts/build_public_export.py`). It reads only stage 1's
vocabulary values. It validates them, withholds the confirmed-valid rows, derives `family_id`,
`repeat_of_own`, `duplicate_source` and `preventable_by`, checks the invariants between columns,
and writes `data/`. The derivation rules are short, public and tested; SCHEMA.md restates them.

## What this dataset can support

- **Distributions.** How often each outcome occurs, and which gates would have caught what.
- **A taxonomy to reuse.** The gate list and the outcome list are the transferable part.
- **A prior on where effort is wasted.** Scope, duplication and economics dominate; impact
  analysis is a minority.

## What it cannot support

- **Any inference about a specific program.** There is no target axis by design.
- **Statistical claims about the field.** n = 113, one researcher, one selection process.
- **Claims about base rates.** This is a log of *failures*. It says nothing about how many
  submissions succeeded, so it cannot yield a success rate.
- **Comparisons across chains or languages.** Those columns do not exist here.

## Sources of bias, stated plainly

1. **Selection bias.** These are programs I chose to hunt. My target-selection habits - including
   the bad ones - are baked into every proportion.
2. **Self-assessed gates.** The `earliest_gate` value is my own post-hoc judgement of what would
   have caught the finding. It is not a record of what any tool said at the time. A different
   analyst would assign some rows differently.
3. **Survivorship in reverse.** I logged rejections. A researcher who logged only rejections will
   always see a dataset that looks like failure.
4. **Rule drift.** Programs change rules. A verdict reflects the rules in force at submission time,
   not today's.
5. **Small n per cell.** Several `earliest_gate` values have one or two rows. Those cells are
   anecdotes, not rates.

## The privacy model

The dataset carries **no target dimension**: no protocol, chain, ecosystem, date or severity.

An earlier draft published `chain` and `ecosystem`, on the theory that withholding the protocol
name was sufficient. It is not: chain + ecosystem + outcome + gate is a **join key** - a
fingerprint, not an anonymisation. The columns were removed and `--check` fails if they reappear.

Three mechanisms enforce the rest:

1. **Closed vocabularies (primary).** Every published value is a member of a vocabulary in
   `scripts/vocab.py`. The public build never sees private text, so none can reach `data/`.
2. **The classifier is private.** The only code that reads private text is not in this
   repository. Until 0.5.1 it was, and its rules quoted report titles; see the CHANGELOG.
3. **Cross-checks against the private source (defence in depth).** Before each release,
   `scripts/verify_blocklist.py` reports any target-looking token in the private source that the
   redaction blocklist does not cover, and its `--scan-repo` mode fails if any tracked file in this
   repository contains an identifier from the private source or five consecutive words copied from
   it. `scripts/check_hygiene.py`, run with the private inputs, scans every blob and commit message
   in the history against the blocklist and the private tripwire list.

## Reproducing

```bash
# the public build on the synthetic fixture - anyone can run this
python scripts/build_public_export.py --classified tests/fixtures/classified.example.csv \
    --provenance tests/fixtures/provenance.example.json --out /tmp/rt-example

# verify the published data/ without any private input
python scripts/build_public_export.py --check --out data/
python scripts/stats.py --check
```

`data/provenance.json` records the SHA-256 of the private source, the classifier and the ledger a
release was built from. The author keeps snapshots of each, so a release can be rebuilt
byte-for-byte; nobody else can rebuild it, by design.

**Never hand-edit `data/`.** Re-run the build.
