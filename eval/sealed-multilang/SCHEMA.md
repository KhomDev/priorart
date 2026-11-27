# eval/sealed-multilang schema

`priorart.sealed-multilang/v1`. This file is the scaffold. It names no unit.

A later commit adds `units.json`. Each counted unit is one judged finding or
one real incident, not each duplicate submission.

| Field | Meaning |
|---|---|
| `unit_id` | Stable id. The split input. |
| `source_url` | URL of the original finding or incident reproduction. |
| `commit` | Pinned git commit, when the source is a git blob. |
| `body_sha256` | SHA-256 of the raw upstream bytes (git blob or HTTP body), not a working-tree rewrite. |
| `licence` | `pointer-only` when the text is not copied into git. `copy` only when a licence allows the bytes to be committed and they were. |
| `upstream_licence` | The upstream licence name, or `none` when the repository has no licence file. |
| `tier` | `J` judged contest or bounty outcome published by the platform or sponsor. `X` real-incident reproduction. `N` negative control. `A` auditor finding with a sponsor status. `AJ` AI-judged, negatives only. |
| `language` | `solidity`, `solana`, or `move`. |
| `cluster` | Source cluster, not a contest already sealed in Dup-Bench or the six benchmark corpora. |
| `outcome_quote` | At most 15 words copied from the original source, not from a list or index. |
| `outcome_url` | The URL of the source the quote was copied from. |
| `group_id` | Judge's duplicate group, or the unit id when the source publishes no group. |
| `duplicate_of` | Representative unit id when this row is a duplicate. Counted rows are representatives, so this is null on a counted unit. |
| `split` | `DEV`, `V1`, `V2`, or `FINAL`, from `split.py` and `SALT`. |
| `runnable_test` | The marker in the original source that shows a runnable test (`#[test]`, `forge test`, `it(`, `function test`, `cargo test`). |
| `priorart_label` | `pending`. No reader label. |

No finding body is committed. `fetch.py` writes raw bytes under `.corpus/`, which is gitignored.

Code4rena Solidity judged findings are not added: that cluster is already the Dup-Bench population. A unit without a runnable test is not added.
