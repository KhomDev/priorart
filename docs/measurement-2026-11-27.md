# DEV re-measurement on the P2R-2 head, 2026-11-27

DEV only: every number here comes from data that was already opened (Dup-Bench train and validation, the
three DEV pairs contests). Nothing sealed was read: Dup-Bench FINAL and the sealed pairs were not touched, and
`build_ranker_data.py` cannot open FINAL by construction.

| Item | Value |
|---|---|
| Source | `a7f5490a970f57830bc2425acf29c8ccea9047d1` (head of PR #41) |
| Binary | release build, `--no-default-features` (the ranker does not use `fetch`), toolchain `1.95.0-x86_64-pc-windows-gnu`; sha256 `54d23059b3ffc2d7a6f9602f3e03cd1d58b71cc333e487304301dad14df06478`, `priorart 0.2.2` |
| Commands | `build_ranker_data.py --split all`, `train_ranker.py`, `calibrate_alarm.py --check`, `pairs_run.py --mode dev` (twice, `--check`) |

Result: the rebuilt dataset has sha256 `26359c35acda39e21ea6efaebcbfdf6cdd64f64d37db970855142d70cea38d2f`, the same as the
dataset the committed `weights/v1.json` was trained on; the retrained weights equal `weights/v1.json` in every
field except `code_commit`; `calibrate_alarm.py --check` passes; the three pairs contests rerun byte-identically.

| Split | Metric | k / n | Rate [Wilson 95%] |
|---|---|---:|---|
| train (1,163 duplicate queries, 472 primary queries) | recall@5, ranker | 631 / 1,163 | 54.3% [51.4, 57.1] |
| train | recall@5, baseline | 627 / 1,163 | 53.9% [51.0, 56.8] |
| validation (1,392 duplicate queries, 353 primary queries) | recall@5, ranker | 990 / 1,392 | 71.1% [68.7, 73.4] |
| validation | recall@5, baseline | 977 / 1,392 | 70.2% [67.7, 72.5] |
| validation | false alarms, score >= 0.9 | 2 / 353 | 0.6% [0.2, 2.0] |
| train | false alarms, score >= 0.9 | 2 / 472 | 0.4% [0.1, 1.5] |
| DEV pairs (train contests) | pooltogether / ondo / numoen | 0/7, 15/15, 7/9 | 0.0% [0.0, 35.4], 100% [79.6, 100], 77.8% [45.3, 93.7] |

Contest coverage: 94 contests requested (train + validation); 83 used; 11 skipped (5 with no public repo, 5
with no primary text, 1 with an incomplete checkout). Validation is the data the model and alarm were selected
on, so it is not an unbiased estimate; train is in-sample for the ranker. **None of this is held-out.**
Not rerun here: the historical P1 benchmark corpora (Tapioca, PoolTogether, Party, Wildcat, Panoptic, Maia),
whose published numbers are 0.2.2 in-sample history.