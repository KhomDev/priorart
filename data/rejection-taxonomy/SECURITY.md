# Security and takedown policy

## Scope

This repository publishes a **derived, redacted dataset** and the scripts that validate it. It
contains no credentials, no exploit code, and no live target information. Two kinds of problem
matter here:

1. **A disclosure** - a row, a document or a script that identifies a program, a team or a
   researcher.
2. **A defect in the scripts** - for example one that would let private text or a non-vocabulary
   value reach `data/`.

## Reporting - privately

Use **Report a vulnerability** on this repository's **Security** tab. That is GitHub's private
vulnerability reporting: only the maintainer sees the report, and you can discuss it there in
private.

Please **do not open a public issue** for a disclosure. A public issue would repeat what the row
reveals, and would itself say which program you work on.

## Takedown

**The default is to comply.** You do not have to justify the request, prove you are affiliated with
the program, or explain what the row reveals.

What happens next:

- The row is removed from the next release. Its `R-###` id is not reused, and it is listed under
  `removed_ids` in `data/provenance.json`, so the id sequence does not silently close up.
- Aggregate counts change accordingly, and the CHANGELOG says so without describing the row.

What cannot be done: a release that was already published, and any copy someone else took of it,
cannot be recalled. This is stated plainly rather than promised away.

## What is already withheld

- No protocol, chain, ecosystem, language, date or severity is published.
- No free text is published: every value is a member of a closed vocabulary and is validated by
  allowlist at build time.
- The code that reads private text is not in this repository; the public build sees vocabulary
  values only.
- `below_payout_floor` rows, which are the dataset's confirmed-valid findings, ship only five
  fields.
- `--aggregates-only` drops the per-row tuples entirely.

## Residual risk, stated plainly

The design argument is that category tuples with no target axis disclose nothing about any specific
program. That is an argument, not a proof. Two residual risks remain, and both are documented in the
README rather than hidden:

1. Most rows are the only row with their combination of categorical values (the README's generated
   *Identifiability* line gives the count). A reader who already knows a specific submission could
   recognise its row.
2. The author is not anonymous. The repository is published under a handle.
