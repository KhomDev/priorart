# Shipped JSON schemas

Three schemas, all draft 2020-12, all in [`schemas/`](../schemas). A test
validates real tool output against each of them, so they are enforced rather
than decorative.

| File | Schema id | Produced by |
|---|---|---|
| [`schemas/dossier.schema.json`](../schemas/dossier.schema.json) | `priorart.dossier/v1` | `priorart build --json` and the written `.dossier` file |
| [`schemas/check.schema.json`](../schemas/check.schema.json) | `priorart.check/v1` | `priorart check --json` |
| [`schemas/ledger.schema.json`](../schemas/ledger.schema.json) | bounty-economics schema v1 | the input contract for `priorart ledger validate` |

The ledger schema is byte-identical to the bounty-economics v0.6.1 tag
(sha256 `403324A1CA205BE9521FE9F51FCCF45030F3BE9B7AA642A2C727DF5842220A51`);
see [ledger.md](ledger.md). The two output schemas are priorart's own:
[dossier.md](dossier.md) explains the dossier fields and [cli.md](cli.md) the
check report.

## How they are tested

`tests/support/mod.rs` contains a small JSON-Schema subset validator covering
exactly the keywords these schemas use: `type` (including unions such as
`["string","null"]`), `required`, `properties`, `additionalProperties: false`,
`items`, `enum`, `const`, `pattern`, `minLength`, `minimum`, `minItems`.
`format` is documentation, not a check (the ledger validator implements the date
and URL checks itself, with better messages).

The tests are
`tests/cli.rs::build_json_is_valid_against_the_shipped_schema`,
`tests/cli.rs::check_json_is_valid_against_the_shipped_schema` and
`tests/cli.rs::the_ledger_schema_is_the_shipped_v1_contract`. Run them with:

```sh
cargo test --locked --test cli
```

## Stability

The schema ids are versioned. `v1` means additive-only changes: new optional
fields are allowed without a version bump, and a consumer must ignore fields it
does not know. A breaking change is `v2`.

The ledger schema is frozen at bounty-economics v1 for compatibility with
existing private ledgers; `schema_version: 1` is the only value this build
implements, and an unknown version is an error rather than a silent misread.

Output is serialised with every key present. A value that does not exist is
`null`, never a missing key, so a consumer never has to tell "absent" from
"null". That is also why the schemas can use `additionalProperties: false`
without breaking on an unknown optional field: there are none.

## Why dossiers are JSON, not SQLite

The dossier is a file you can read, diff and archive. It is built once per
program, it is small (the committed demo is 13 items), and it holds no
wall-clock field, no absolute path and no machine name, so two builds are
byte-identical. A database would add a dependency and a migration surface to buy
nothing at this size.
