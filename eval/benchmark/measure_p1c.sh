#!/usr/bin/env bash
# P1-C re-measurement of all six priorart corpora with the frozen 0.3.0
# matcher. The two sealed corpora get their dev-split freeze first; the four
# in-sample corpora already have one from P1.
#
# Usage: eval/benchmark/measure_p1c.sh [BINARY]
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"
bin="${1:-target/release/priorart.exe}"
# run.py spawns the binary directly; on Windows a relative path is not found.
case "$bin" in
    /*|[A-Za-z]:*) ;;
    *) bin="$root/$bin" ;;
esac
for dir in eval/benchmark eval/benchmark/pooltogether eval/benchmark/maia-c4 \
           eval/benchmark/panoptic-c4 eval/benchmark/party-c4 eval/benchmark/wildcat-c4; do
    if [ ! -f "$dir/freeze.json" ]; then
        echo "freeze dev: $dir"
        python eval/benchmark/run.py --corpus-dir "$dir" --priorart "$bin" --split dev
    fi
    echo "measure all: $dir"
    python eval/benchmark/run.py --corpus-dir "$dir" --priorart "$bin" --split all
done
python eval/benchmark/pool_p1c.py
