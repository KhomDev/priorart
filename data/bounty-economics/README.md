# bounty-economics

**A framework for reading a bug-bounty program's terms before you hunt: payout tiers,
exclusion-clause classes, and prior-audit surfaces.**

The most expensive lesson in bug bounty is not "you missed a bug." It is:

> **You found a real, working, PoC-backed bug — and the program could not pay you for it.**

This repo is the *method* for avoiding that, plus a schema for building your own private ledger
of the programs you hunt.

**It ships no program data.** No names, no chains, no reward tables. Your ledger is yours.

---

## Why the method matters more than a dataset

A published table of "program → payout floor" would be useless within a month: programs change
their reward tables without notice, and a stale floor is worse than no floor because you trust it.

What does not go stale is the **structure** of the problem:

- Every program's reward table resolves to one of a small number of **tier shapes**.
- Every program's scope page contains exclusions that fall into a small number of **clause classes**.
- Every program has a **prior-audit surface** that is already closed.

Learn the three taxonomies, apply them to the live program page, and you get the answer yourself —
current, and cited.

## What is in here

```text
data/tier-shapes.yaml               the 5 tier shapes, with detection and risk notes
data/clause-classes.yaml            the 14 exclusion-clause classes, with escape hatches
data/gate-crossref.yaml             which catalog answers each viability-gate gate token
schemas/ledger.schema.json          JSON Schema (v1) for a private ledger
scripts/validate.py                 validates a ledger; --taxonomy prints the vocabularies
scripts/crossref.py                 gate token -> the catalog that reads the program page for it
scripts/check_generic.py            fails if a taxonomy or the schema names a platform or a foreign repo
scripts/check_hygiene.py            fails on mojibake, an identity token or a private path anywhere
scripts/check_links.py              fails on a relative link in a document that does not resolve
scripts/check_markdown.py           fails on a mangled code fence or a stale pinned tag
scripts/token_scan.py               the chain / platform token vocabulary and its matcher
examples/ledger.example.yaml        one fictional entry, showing the shape
examples/tier-shapes.example.yaml   five fictional entries, one per tier shape
```

Both vocabularies are **data**, not prose, so the documentation, the schema and the validator
cannot drift apart. `validate.py` reads them at runtime.

```bash
python scripts/validate.py --taxonomy                     # print both vocabularies
python scripts/validate.py my-ledger.yaml                 # validate your ledger
python scripts/validate.py my-ledger.yaml --format json   # machine-readable result
python scripts/validate.py examples/tier-shapes.example.yaml
```

`examples/tier-shapes.example.yaml` is worth reading once: five fictional programs, one per tier
shape, each with the decision the shape forces written out. It is the fastest way to learn to
classify a reward table.

## 1. Payout tier shapes

The single most consequential field. Classify every program you consider:

| Tier shape | Pays | Implication |
|---|---|---|
| `critical_only` | Critical | **Everything below Critical is worth $0.** A Medium-class thesis has an expected value of zero here, however correct it is. |
| `high_and_above` | Critical, High | Medium and below are unpaid. |
| `medium_and_above` | Critical, High, Medium | The most common shape. Low and informational unpaid. |
| `low_and_above` | Critical → Low | Low is the lowest tier that pays, and the reward table has to say so. |
| `unknown` | — | **The dangerous one.** Not yet transcribed. Treat as an unhedged bet, never as permissive. |

### "No severity floor" is not a shape — and it is not evidence

Two situations look alike on a scope page. They are not the same ledger entry, and collapsing
them is the defect this field exists to prevent:

| What the page says | `floor_status` | `tier_shape` |
|---|---|---|
| A reward table with a Low row and a non-zero amount | `documented` | `low_and_above` |
| `SEVERITY FLOOR: NONE` or `SEVERITY FLOOR: any`, in those words | `documented` | `low_and_above` |
| Nothing about a floor anywhere on the page | `unstated` | `unknown` |
| Two of the program's own pages disagree | `conflicting` | `unknown` |

An **unstated** floor is not evidence that Low pays. It is evidence that the rewards page has
not been read. Transcribing it as `low_and_above` manufactures a permissive-looking floor out
of an unread page — and a floor you invented is worse than no floor, because you will act on it.
That is the same failure as reading `unknown` as "pays everything", one step earlier.

So an unstated floor keeps `tier_shape: unknown`. `validate.py` **errors** if you write
`floor_status: unstated` beside any other shape, and **warns** even when you get it right,
because the entry is a gap in your ledger rather than a property of the program.

An explicit `NONE` / `any` floor is different. The program has *stated* something, you have
read it, and the statement is that no severity bar applies. Informational findings are still
acknowledged rather than paid, so the lowest tier that actually pays is Low:
`low_and_above`, with `floor_status: documented`.

### Why there is no `any` shape

An earlier version of this taxonomy carried a sixth shape, `any`, for programs whose reward
table assigns a non-zero amount to informational findings. It was removed for a reason worth
keeping: the shape put an informational floor in the ledger, which made a hardening note look
payable and made a genuinely wide-floor program look narrower than it is.

**A shape that reads as a green light is worse than no shape**, whether or not anyone has met a
program that fits it. This repository ships no count of how often such a program was met, so it
makes no claim about that — and no claim should be read into the removal.

### The two failure modes this prevents

**Mode 1 — the floor is above your finding.** You confirm a real Medium with a working PoC. The
team agrees it is real. The reward table has no Medium row. Payout: zero.

**Mode 2 — you misread "unknown" as "pays everything".** A program whose reward table you never
read is not a program that pays everything. It is a program you know nothing about.

### A note from the author's log: the floor, not the bug, decides

The costliest entries in my own rejection log are not misread exclusion clauses and not
duplicates. They are confirmed, PoC-backed findings that the team agreed were real — and that
paid nothing because the program's floor sat one tier above the finding:

- A **valid Medium with a working proof, on a `high_and_above` program.** The accounting defect
  was reproduced and acknowledged; Medium had no reward row. Payout: zero. This shape produced
  the single most expensive entry in the log — correct work, zero return.
- A **valid Low, on a `critical_only` program** (one that labels itself critical-only). Low was
  real and reproduced; the table paid Critical only. Out of scope for payout.

Two lessons I had to learn the expensive way:

1. **Classify the tier shape before you build the PoC, not after.** Every one of these entries
   would have been caught by reading the reward table first — a twenty-minute task — instead of
   after forty hours of proof work. The shape, not the severity of the bug, is what decided the
   payout.
2. **A finding "one tier below the floor" is worth exactly zero, not "almost paid."** A Medium on
   a `high_and_above` program and a Low on a `critical_only` program are the same economic outcome
   as a finding that was wrong: nothing. The floor is a cliff, not a slope.

This is why `tier_shape` is the first field in the ledger and why `validate.py` warns the moment
it sees `critical_only` or `high_and_above`: those are the shapes where correct work most often
earns nothing.

### How to transcribe a tier

1. Open the program's live scope/rewards page.
2. Find the reward table. Record **every** severity row and its amount or range.
3. Note the **lowest severity with a non-zero reward** — that is the floor.
4. If the page states no floor anywhere, set `floor_status: unstated` and leave
   `tier_shape: unknown`. Do **not** write `low_and_above`: that is a claim you have not
   earned. If the program states `NONE` / `any` explicitly, set `floor_status: documented`
   and record the shape those words imply.
5. Record the URL and the date you read it.
6. Re-check before submitting anything. A floor read three months ago is a guess.

> **The trap:** some programs publish two different numbers — a "prize pool" and a "rewards" total,
> or a scope table that labels an asset `Critical` while the reward table pays it at `High`. When
> the two disagree, the **reward table** governs what you are paid. Record the conflict explicitly
> rather than resolving it silently.

## 2. Exclusion clause classes

Almost every program's scope page carries a list of out-of-scope categories. They are worded
differently everywhere and mean one of a dozen things. Classify each clause you find:

| Class | Typical wording | What it kills |
|---|---|---|
| `privileged_role` | "attacks requiring access to privileged addresses" | Anything needing admin/owner/capability action |
| `human_error` | "human-based errors and rogue privileged users" | Admin mistakes, including *unknowing* admin harm |
| `admin_input` | "issues requiring admin input validation" | Unvalidated parameters supplied by a trusted role |
| `dos_no_extraction` | "DoS without enabling direct fund extraction" | Freezes, griefing, liveness attacks |
| `front_run_only` | "vulnerabilities exploitable through front-running only" | Mempool races — unless the attack is atomic |
| `best_practice` | "best-practice critiques", "optimization suggestions" | Style, gas, convention deviations |
| `design_issue` | "design issues that are not necessarily a security risk" | Intentional tradeoffs you disagree with |
| `off_chain` | "off-chain components are out of scope" | Node logic, relayers, signers, backends |
| `third_party` | "issues in external dependencies" | Oracles, bridges, other protocols |
| `user_input` | "issues arising from user input construction" | Self-inflicted preconditions |
| `known_issue` | "previously reported or acknowledged issues" | Anything in the prior-audit surface |
| `testing_limits` | "no mainnet or public-testnet testing" | PoCs that require live-network execution |
| `non_standard_token` | "fee-on-transfer/rebasing tokens" | Token-integration findings, unless listed |
| `scope_boundary` | "only files under `src/` are in scope" | Root causes outside the named tree |

Which classes actually waste submissions is a question about **your own** log. The author's
log is published, classified, in
[rejection-taxonomy](https://github.com/KhomDev/rejection-taxonomy): at v0.5.1 (113 rows),
**38% of the rejections were duplicates (43/113) and 23% were out of scope (26/113)**, by its
`triage_class` column. (Earlier versions of this README quoted 37% and 27%: the same log at 97
rows.) That is one researcher's log, **not an industry census**. Treat it as a prior, not a
number you can cite about anyone else.

### Sometimes argued: unknowing admin harm

`privileged_role` and `human_error` are often argued to be a large source of wasted
submissions. Treat that as an argument, not a measurement:

- **It is not measured here.** This repository ships no industry-wide count of wasted submissions
  by category, and the author's own log contains **no successful instance** of the
  unknowing-admin-harm framing.
- **Many programs exclude it explicitly.** `human_error` ("human-based errors and rogue
  privileged users") is written to close exactly this door.
- **So read the clause, in full, on the program you are hunting** before spending hours on the
  framing. A regex match is a prompt to check, not a verdict — in either direction.

The distinction the argument rests on is still worth checking, because where it survives the
clause it is a real code defect:

- A finding that **requires** an admin to act maliciously is out of scope.
- A finding where a **reasonable admin action** has a non-obvious destructive second-order effect
  can be in scope — the admin is not misbehaving, the code is.

It is not a loophole; it is the difference between "the admin attacked the protocol" and "the
protocol broke when the admin did their job." It is also not a reliable payout:
`human_error` exists to exclude it, and usually does.

### Turning a clause into a test

Transcribe each clause as a **regex over your own finding description**, then run it. A match is a
prompt to check, not an automatic reject — over-trusting a regex manufactures false negatives,
which is its own expensive failure. [viability-gate](https://github.com/KhomDev/viability-gate)
0.5.0 and later run every exclusion `pattern` in your ledger over a draft's claim sections in
gate G2 (`vg check`, or `vg gates --finding`). Earlier versions only checked that the patterns
compiled.

Write patterns both tools can read: viability-gate uses Rust's `regex` crate,
case-insensitively, so **no lookaround and no backreferences**. `validate.py` rejects those, and
it compiles every pattern case-insensitively, as viability-gate does.

## 3. Prior-audit surface

The surface that is already closed. Before hunting:

1. Collect every linked audit report, contest result and disclosure for the target.
2. Note which **files** and **functions** each one covers — not just the titles.
3. Record the **known issues** section verbatim (it is the program's own list of things not to
   report).
4. Note the **post-audit delta**: what changed after the last audit. That is where the unpicked
   fruit is.

Two traps:

- **A prior audit naming a file is not the same as covering your function.** Read the finding, not
  the table of contents.
- **Post-audit rewrites are the richest surface.** A large rewrite after the last audit is where
  new bugs live — and it is also where you are least likely to be duplicating.

## 4. The ledger schema

Build your own. Keep it **private** — it will contain program names, and your judgement about
them is not something to publish.

```yaml
# my-ledger.yaml  — PRIVATE, never commit
- id: <your-slug>
  schema_version: 1              # optional; absent means 1
  tier_shape: medium_and_above
  floor_status: documented       # documented | unstated | conflicting
  floor_severity: medium         # only when floor_status is 'documented'
  source_url: https://...        # the live rewards page
  retrieved: 2026-10-08
  conflicts: []                  # e.g. "scope table says Critical, rewards pay High"
  exclusions:
    - class: privileged_role
      clause: "<verbatim from the scope page>"
      pattern: "owner|admin|capability"
    - class: dos_no_extraction
      clause: "<verbatim>"
      pattern: "dos|freeze|lock"
  prior_audits:
    - firm: <name>
      url: https://...
      covers: ["src/Vault.sol", "src/Oracle.sol"]
      known_issues: ["<verbatim>"]
  post_audit_delta: "<module> +878 lines, <module> +182 lines"   # what changed since the last audit
```

An entry with no floor statement looks like this instead — and note that it is `unknown`, not
`low_and_above`:

```yaml
- id: <your-slug>
  schema_version: 1
  tier_shape: unknown            # the page has not been read
  floor_status: unstated         # no floor statement exists
  floor_severity: null
  source_url: https://...        # still required: cite the page you looked at
  retrieved: 2026-10-08
```

Write `retrieved` **quoted** (`retrieved: "2026-10-08"`). An unquoted YAML date is read as a
date object rather than a string, and the schema declares a string, so the validator warns — and
any JSON-Schema consumer would reject the entry.

`scripts/validate.py` checks a ledger against [`schemas/ledger.schema.json`](schemas/ledger.schema.json)
(with the `jsonschema` package: an unknown or misspelt key, a wrong type and a bad date format
are errors) and against the shipped taxonomies: required fields, closed vocabularies for
`tier_shape`, `floor_status` and exclusion `class`, regexes both tools can compile, and a
non-empty `source_url` + `retrieved` date on every entry. **An uncited entry fails validation** —
including yours and mine.

```bash
python scripts/validate.py my-ledger.yaml          # your private ledger
python scripts/validate.py my-ledger.yaml --strict # warnings become errors
python scripts/validate.py my-ledger.yaml --format json
python scripts/validate.py examples/ledger.example.yaml
python scripts/validate.py --taxonomy              # print both vocabularies
python scripts/validate.py --taxonomy --format json
```

`--format json` writes the same verdict as a JSON object — `ok`, `entries`, `errors`,
`warnings` — so a CI job or another tool can consume it without parsing prose. It is not a
second implementation: the exit code and the error list come from the same pass as the text
output.

It warns on the things that cost the most:

- **`floor_status: unstated`** — the floor is not evidence-backed. Do not read it as "Low pays";
  open the rewards page.
- **`floor_status` missing** — the validator cannot tell a floor you read from one you assumed.
- **`tier_shape: unknown`** — not permissive. An untranscribed program is an unhedged bet.
- **`tier_shape: critical_only` / `high_and_above`** — pays nothing below that bar, so confirm
  your thesis can reach it before hunting.
- **`retrieved` older than 180 days** — program terms change without notice; re-read the page.
- **`prior_audits[].covers` missing** — a report naming a file is not the same as covering your
  function.

It **errors** on the contradictions: an unstated or conflicting floor beside a shape other than
`unknown`, a documented floor with no `floor_severity`, a documented floor beside `unknown`, a
`retrieved` date in the future, a pattern viability-gate cannot compile, an unknown key, a
`schema_version` this validator does not implement, and of course a missing citation.

It also flags a **floor conflict**: if `tier_shape` implies a floor of `medium` but
`floor_severity` says `high`, one of them is wrong and it says so.

Placeholder hosts (`example.com`, `localhost`, any `*.invalid` host, or a host with a `todo` /
`tbd` / `changeme` label) are an **error** in a real ledger and a warning in a file named
`*.example.yaml`, so the shipped examples validate while a copied-and-not-edited ledger does not.
The check reads the URL's host, so a real page whose path happens to contain "todo" is not
flagged.

### Tests

```bash
python -m pytest tests/ -q     # or: python tests/test_validate.py
```

Four suites:

- `tests/test_validate.py` — the validator's behaviour: an uncited entry fails, a placeholder
  host fails outside example mode, the removed `any` shape gets a migration message, an unstated
  floor cannot be written as `low_and_above`, and `--format json` reports the same verdict as
  the text output.
- `tests/test_counts.py` — the README, the taxonomy files, the schema and the worked example
  quote the same counts and the same vocabularies. This is the suite that keeps the counts in
  this README tied to the files they describe.
- `tests/test_generic.py` — the shipped taxonomies name no platform and no foreign repository.
- `tests/test_compat_viability_gate.py` — the cross-repo contract below.

### Schema and cross-repo version matrix

The ledger is a YAML **list** of entries, and the version lives in the schema's `$id` plus an
optional `schema_version` on each entry. The list shape is deliberate: `vg`
([viability-gate](https://github.com/KhomDev/viability-gate)) deserializes the file as a sequence,
so changing the root to a mapping would break a tool that reads the same ledger.

| bounty-economics schema version | viability-gate ledger version | Status |
|---|---|---|
| v1 | ledger v1 (`vg init`) | current |
| v1 | ledger v1, pre-versioning | entries with no `schema_version` are read as schema version 1; the validator warns if `floor_status` is absent, and `vg` 0.5.0 fails G1 on it |

New fields are **additive**: `vg` ignores ledger fields it does not read, so a v1 ledger written
here stays parseable by it. `vg` does read `floor_status`: its G1 gate fails an `unstated`,
`conflicting` or missing floor. The validator is still the stricter of the two - it is the one
that checks the schema, the citation and the shape/status contradictions.

The pin lives in exactly one place: `PINNED_VG_TAG` in
[tests/test_compat_viability_gate.py](tests/test_compat_viability_gate.py). That test checks both
directions at the pinned tag — `vg init` output must satisfy this repo's schema once filled in,
and a ledger this repo accepts must be readable by `vg` and read the same way: an unstated floor
fails `vg`'s G1, and every example exclusion pattern compiles in `vg`'s G2. Locally it **skips**
when the sibling checkout `../viability-gate` is absent, when the pinned tag is missing, or when
`git` or `cargo` is unavailable. **In CI it fails instead** (`BE_COMPAT_REQUIRED=1`), so a broken
pin cannot pass silently. The in-repo half of the contract (schema ↔ taxonomy ↔ example) always
runs. Set `VG_SIBLING` to point the test at a checkout under another folder name.

> **Current state:** pinned to viability-gate `v0.5.0`, the release whose G2 runs ledger
> exclusion patterns over a draft and whose G1 reads `floor_status`.

### Install

```bash
python -m pip install .            # or -e ".[dev]" for a working checkout
bounty-econ-validate --taxonomy
bounty-econ-validate my-ledger.yaml --format json
```

The installed package is `bounty_economics`; the taxonomies and the schema ship inside it, so
the command works from a plain install, not only from a checkout. (Before 0.5.0 only an editable
install worked, and it installed top-level modules called `validate` and `check_generic`.)

CI runs pytest, ruff, the taxonomy print, the genericity check, the placeholder-host negative
test, a clean wheel install and the compatibility test, on Python 3.10 through 3.13.

## What this repo deliberately does not contain

- **No program names.** Not one, not anonymised, not as an example.
- **No chains or ecosystems.** Which chains a researcher works on is itself identifying.
- **No reward tables.** They go stale, and republishing them detached from the live page invites
  reliance on a stale number.
- **No exclusion clauses quoted from any specific program.** Only the generic *classes*, which are
  industry-standard and appear across programs in almost identical wording.
- **No prior-audit lists.** Naming which audits cover which program identifies the program.

The taxonomies are the contribution. The instances stay with whoever read the page.

## Composing with viability-gate: the gate cross-reference

The four repos share one gate vocabulary. [viability-gate](https://github.com/KhomDev/viability-gate)
maps each of its anti-patterns onto a gate token — a fired rule is a statement *about* a gate. This
repo closes the loop for the two gates that are decided by the **program's own terms** rather than by
the finding's prose:

```
anti-pattern (viability-gate)  ->  gate token  ->  program-terms catalog (here)
```

`data/gate-crossref.yaml` is the mapping, and `scripts/crossref.py` resolves it:

```bash
python scripts/crossref.py                      # the whole cross-reference
python scripts/crossref.py G2_exclusion_clause  # -> clause-classes (the 14 ids)
python scripts/crossref.py G1_severity_floor    # -> tier-shapes (the 5 ids)
```

The two load-bearing mappings are `G1_severity_floor → tier-shapes` (the reward table) and
`G2_exclusion_clause → clause-classes` (the scope page); `program_ceiling` and `P16_below_floor`
also resolve to tier-shapes, and `G4_prior_audit` points at the prior-audit surface this repo
documents in prose. The cross-reference is deliberately **honest about its edges**: gates whose
signal is not in the program's terms (`D1_duplicate_check`, `G3_scope_tree`, `P17_no_attacker_win`
and the rest) are listed as *not answered here*, with a note saying where the signal actually
lives. `tests/test_crossref.py` keeps every gate token inside the shared vocabulary and every
catalog reference resolvable.

## How this relates to the other repos

| Repo | Question it answers | Ships |
|---|---|---|
| **bounty-economics** (this) | *Can this program pay for what I found?* | Taxonomies of program terms + ledger schema |
| [rejection-taxonomy](https://github.com/KhomDev/rejection-taxonomy) | *Why do findings fail after submission?* | Outcome taxonomy + aggregate stats |
| [viability-gate](https://github.com/KhomDev/viability-gate) | *Should I submit this specific draft?* | The CLI: scores a draft against its rule catalog and runs the gates over your private ledger (this repo's schema) |
| [realism-gate](https://github.com/KhomDev/realism-gate) | *Does my PoC show what an attacker can do?* | A static checker for Foundry PoC tests |

All four are methodology. None ships target data.

## Known limitations

- **This is a reading aid, not a measurement.** The taxonomies have not been validated against a
  labelled corpus of program pages, because no such corpus is shipped here.
- **The rejection figures describe one researcher.** The 38% duplicates / 23% out-of-scope split
  is reproducible from the published rejection-taxonomy dataset, but it is one person's log, not
  an industry census, and not something you should cite as one.
- **The unknowing-admin-harm framing is unproven.** The author's log contains no successful
  instance; some programs exclude it explicitly via `human_error`.
- **The cross-repo compatibility test covers one viability-gate tag** (`v0.5.0`). A later
  viability-gate release is only covered once the pin is bumped.
- **No example has been checked against a real program page.** Every example uses the reserved
  `.invalid` TLD, and every program in them is invented.
- **The ledger format is a convention, not an enforcement mechanism.** Nothing stops you writing a
  floor you never read; `floor_status` only makes the gap visible.
- **`floor_status: conflicting` is reported, not resolved.** The schema gives you a place to
  record the disagreement; deciding which reading governs is still your call.

## Caveats

- **Program rules change without notice.** Every taxonomy here is a reading aid, not a substitute
  for the live scope page, which always outranks it.
- **This is not legal or financial advice.**
- **The classes are a starting vocabulary, not a closed set.** Widen them as you find clauses that
  do not fit; PRs adding a class with a generic, unattributed example are welcome.

## License

**The repository is MIT, as it stands.** This table records the *current* state; it does not
change any license.

| Path | License | Notes |
|---|---|---|
| `data/**` | MIT (`LICENSE`) | the two taxonomy data files |
| `schemas/**` | MIT (`LICENSE`) | `ledger.schema.json` (schema v1) |
| `scripts/**` | MIT (`LICENSE`) | the validator and the repository checks |
| `tests/**` | MIT (`LICENSE`) | pytest suites |
| `examples/**` | MIT (`LICENSE`) | fictional entries, `.invalid` hosts |
| `.github/**` | MIT (`LICENSE`) | CI workflow and issue templates |
| `README.md`, `CHANGELOG.md`, `CONTRIBUTING.md`, `SECURITY.md`, `CITATION.cff` | MIT (`LICENSE`) | documentation |

No third-party program data is included or redistributed, so no third-party license applies to
anything shipped here.

**No third-party data.** This repository contains methodology, schemas and tooling only. It
ships no program names, no chains, no reward tables and no scope text from any third party.
Program terms are the property of their programs; this repository grants no rights over them and
reproduces none of them. (Until 0.3.0 this paragraph sat after the MIT text in `LICENSE`, which
stopped GitHub from detecting the licence. [`LICENSE`](LICENSE) is now the MIT text alone.)
Security reports go through GitHub's private vulnerability reporting; see
[`SECURITY.md`](SECURITY.md).
