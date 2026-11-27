# Sealed multilingual corpus

New held-out corpus. It does not modify Dup-Bench, the six benchmark corpora,
their labels, salts, or splits.

The salt and the split rule were committed in
`0bf85079b693da95beb915fa9a5a06797d97b385`, before any unit id existed:

```text
bucket = int(sha256(salt + "|" + unit_id).hexdigest()[:8], 16) % 100
0-39 DEV, 40-59 V1, 60-74 V2, 75-99 FINAL
```

`priorart` was not run on these units. `priorart_label` is `pending`.

Bodies are not committed. `licence` is `pointer-only`. `fetch.py --check`
re-downloads each pinned file into gitignored `.corpus/sealed-multilang/` and
compares sha256 to `body_sha256`.

One counted unit per finding. `duplicate_of` is null on every counted row.
Code4rena Solidity judged findings were not added: Dup-Bench already holds
111 of those contests.

## Counts

| | units |
|---|---:|
| All | 629 |
| tier X (real-incident reproduction) | 614 |
| tier J (platform-judged) | 5 |
| tier A (auditor finding) | 10 |
| tier N | 0 |
| tier AJ | 0 |
| language solidity | 614 |
| language solana | 15 |
| language move | 0 |
| cluster defihacklabs | 614 |
| cluster immunefi-jito-restaking | 5 |
| cluster meteora-audits | 10 |

Split, from the sealed salt, not rebalanced: DEV 233, V1 146, V2 99, FINAL 151.

## What was kept

- **defihacklabs** (X, Solidity). `SunWeb3Sec/DeFiHackLabs` at
  `8a2b37f17377c1c7584cfbe63621dbe01200cfaf`. Each unit is one `src/test`
  file that contains `function test` and an in-file `// @KeyInfo` line.
  The quote is the first 15 words of that line. The repo is Apache-2.0;
  each test file says `UNLICENSED`, so the text is not copied.
- **immunefi-jito-restaking** (J, Solana). Five High reports at
  `88cb94b4c0525c62d61e755f7d2142226a5a84e0` whose Proof of Concept contains
  a Rust `#[test]`. The quote is the first 15 words of the report's Impact
  section. No licence file.
- **meteora-audits** (A, Solana). Ten finding sections in
  `MeteoraAg/audits` whose own section contains `it(` or `#[test]`. The
  quote is the finding heading in that section. No licence file.

## Shortfalls

- **Move: 0.** Initia Move's Code4rena PDF has 8 High/Medium findings; the
  only `#[test]` is M-04, and that section is `keepers.go`. Movement
  Attackathon reports that cite `.move` sources do not ship a runnable test
  of a Move module. MoveBit's Cetus Aptos PDF quotes unfinished project
  tests, not an exploit test. Public Solana/Move exploit-PoC repository
  names that were probed do not exist. See `SEARCH_LOG.md`.
- **Sherlock Orderly and WOOFi Solana: 0.** The judged High/Medium
  representatives do not contain a runnable test.
- **Code4rena Anchor: 0** judged High/Medium issues with a runnable test.
  Lavarage is already in Dup-Bench and was not added.
- **N and AJ: 0.** No published negative-control set or AI-judged negative
  set with runnable tests was found. None were invented.

## Files

| File | Role |
|---|---|
| `SALT` | Split salt. Do not edit. |
| `split.py` | Split function. Do not edit. |
| `units.json` | Counted units. Pointers, quotes, hashes. |
| `build.py` | Rebuilds `units.json` from the pinned checkouts. |
| `fetch.py` | Re-downloads pins and checks sha256. |
| `SEARCH_LOG.md` | Queries and dead ends. |
