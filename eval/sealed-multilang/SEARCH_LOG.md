# Search log

Every query used while collecting `eval/sealed-multilang`, including dead ends.
Queries before the salt commit `0bf85079b693da95beb915fa9a5a06797d97b385` are
marked pre-seal. They did not choose the salt and did not run priorart.
Outcome quotes in `units.json` were copied from the unit's own file, not from
an index.

## Pre-seal (before any unit id existed)

- `gh search repos` under `sherlock-audit` for orderly, woofi, solana, move, aptos, sui. Hits: `2024-09-orderly-network-solana-contract-judging`, `2024-08-woofi-solana-deployment-judging`. No Sherlock Aptos or Sui repo.
- `gh search repos` under `code-423n4` for pump, lavarage, meteora, garden, aptos, sui, move, solana, jito. Hits: `2025-01-pump-science`, `2024-04-lavarage-findings`, `2025-11-garden`, `2025-08-solana-foundation`, `2025-01-initia-move`. No Meteora, Aptos, or Sui findings repo.
- `GET /orgs/code-423n4/repos` (881) and `GET /orgs/sherlock-audit/repos` (460). Name filter found no Meteora, Aptos, or Sui contest. Solana names: lavarage, pump-science, garden (also has a `sui/` tree), solana-foundation, orderly, woofi. Move name: `2025-01-initia-move`.
- `GET repos/immunefi-team/Past-Audit-Competitions/contents`. Directories include `jito-restaking` and `movement-labs-attackathon`. No Aptos or Sui directory.
- `GET repos/MeteoraAg/audits` and `repos/jito-foundation/restaking/security_audits`. PDFs, no licence file on the audits repo. Jito code repo is Apache-2.0; the PDFs are auditor reports.
- `GET repos/movebit/Sampled-Audit-Reports`, `repos/Zellic/publications`, `repos/initia-labs/audits`, `repos/gardenfi/audits`. MoveBit, Zellic, and Initia audits have no licence file. Garden audits repo has an MIT licence file; its PDFs were not used.
- Code4rena Garden, Pump Science, Solana Foundation, and Initia Move have contest repos and no public `*-findings` repo. Garden's `2025-11-garden-V12-findings.md` is an out-of-scope AI report, not a judged warden set. Not used.

## Post-seal

- Dup-Bench `eval/dupbench/sources.json` on this branch: 111 Code4rena repos, including `2024-04-lavarage-findings`. Lavarage was not added. Anchor `2022-02-anchor-findings` is not in that list.
- `eval/dataset.manifest.yaml`: 460 issue URLs and body sha256s. Units whose URL or normalised body hash collided were dropped. None of the kept units collided.
- `git clone --depth 1 https://github.com/SunWeb3Sec/DeFiHackLabs` at `8a2b37f17377c1c7584cfbe63621dbe01200cfaf`. Repo licence Apache-2.0. Test files declare `SPDX-License-Identifier: UNLICENSED`, so bodies stay pointer-only. 953 Solidity tests, 0 Rust, 0 Move. 929 contain `function test`. 614 also contain an in-file `// @KeyInfo` outcome line and were kept. The other 315 have a runnable test but no in-file outcome sentence, so no quote was taken from the repository README index.
- Sherlock Orderly `90efced2987f20a757be4f1854dba1d1379e026f` and WOOFi `121f82b0b1a58566eb2d401dbaf842b00e7d0beb`. The judged report names 3 and 5 High/Medium findings. None of those representative submissions contains `#[test]`, `function test`, `it(`, or `anchor test`. A few other files in the trees do. They are not the judged representatives. Not added.
- Immunefi `jito-restaking` at `88cb94b4c0525c62d61e755f7d2142226a5a84e0`. Five High reports contain a Rust `#[test]` in the Proof of Concept section. Three Insight reports were not added. No licence file.
- Immunefi `firedancer-v0.1`: 14 Critical/High/Medium reports, none with a runnable test marker. Not added.
- Immunefi `movement-labs-attackathon`: 124 reports. Reports that cite an `aptos-framework` `.move` path and are Critical/High/Medium do not contain a runnable test, except `43333`, whose bug line is `third_party/move/move-vm` Rust and whose test is a Rust test. Not counted as a Move module finding.
- `GET` all issues of `code-423n4/2022-02-anchor-findings` (78). Three bodies match a test marker. None of those three also carries a judge outcome (`sponsor confirmed`, `selected for report`, `duplicate`, or `invalid`) together with High/Medium. Not added.
- `git clone` `MeteoraAg/audits` and `initia-labs/audits`. `pdftotext` over 92 Meteora PDFs and 11 Initia PDFs. Kept the finding sections whose own text contains `it(` or `#[test]` and a finding title: 10 Meteora sections (tier A, Solana). Initia Move Code4rena report has 8 High/Medium findings; the only `#[test]` is M-04, and that section is `keepers.go`, not a `.move` module. Not added. Initia cosmos and rollup PDFs have no in-section runnable test.
- `git clone` `movebit/Sampled-Audit-Reports` (29 PDFs, no licence). Two PDFs match `#[test]`. Cetus Aptos quotes the project's unfinished `clmm_math.move` tests (`......` bodies) under a TODO finding, not a runnable exploit test. Mini-Miners matches no `#[test]` once the header false positive is removed. Not added.
- `gh search repos "solana exploit poc"`, `gh search repos "sui move exploit poc"`, `gh search repos "wormhole exploit foundry"`, and `GET search/repositories?q=solana+hack+poc+in:name` returned no usable repository. Probed and missing: `neodyme-labs/solana-poc`, `ottersec/solana-pocs`, `0xMacro/solana-exploit-pocs`, `blocksecteam/solana-hacks`, `slowmist/solana-exploit-poc`, `SunWeb3Sec/DeFiHackLabs-Solana`, `movebit/move-poc`.
- `coral-xyz/sealevel-attacks` exists (no licence). It is a teaching repo, not a judged contest and not a record of a real incident. Not added.
- Code4rena Solidity judged findings were not collected. Dup-Bench already holds 111 of those contests.

No `priorart` command was run on any unit.
