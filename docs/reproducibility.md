# Reproducing priorart's numbers

Commands run from the repository root. Python 3.12 runs `eval/` with the standard library (`python -I`).

## Build and tests
```bash
cargo build --release --locked                     # default build includes `priorart fetch`
cargo test --locked                                # all four layers; see testing.md
cargo test --locked --no-default-features          # the no-network build (no HTTP client compiled)
```

## Dup-Bench (DEV only until the held-out rounds)
```bash
python -I eval/dupbench/fetch.py --check                          # verify every pin offline
python -I eval/dupbench/clone_codebases.py --split train          # pinned sponsor snapshots (network)
python -I eval/dupbench/build_ranker_data.py --split all          # queries and features
python -I eval/dupbench/train_ranker.py --dataset target/ranker-data/dataset.jsonl   # reproduces weights/v1.json
python -I eval/dupbench/calibrate_alarm.py --dataset target/ranker-data/dataset.jsonl --check   # verifies alarm-v1.json
python -I eval/dupbench/pairs_run.py --mode dev --contest 2023-01-drips --out target/pairs-dev.json
```
`pairs_run.py --mode pairs` needs `--round`, `--version` and a `--binary-sha256` equal to the
binary's sha256, plus `--pair-labels`. It refuses placeholders and FINAL contests. Do not run it before
the pair labels exist and the Round-1 freeze is committed.

## Pinned inputs
The manifest, labels, pairs and model hashes and the source commit are listed in
`eval/P2R2_PREREGISTRATION_POST_MAIN.md` §1-§2.