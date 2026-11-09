#!/usr/bin/env bash
# Regenerate every example output in README.md by RUNNING the binary.
#
# Hand-written example output drifts. It already had: the previous README showed
# "0% estimated payout odds" for a finding that trips P2 (weight 40) and P19
# (weight 50), which computes to 10, not 0. The example was stale.
#
# This script is the only writer of the example block. CI runs it with --check
# and fails if the README has drifted from what the tool actually prints.
#
# Usage:
#   scripts/regen-readme-examples.sh            # rewrite README.md
#   scripts/regen-readme-examples.sh --check    # exit 1 if it would change
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

MODE="write"
if [ "${1:-}" = "--check" ]; then MODE="check"; fi

BIN="$ROOT/target/release/vg"
if [ ! -x "$BIN" ] && [ ! -f "$BIN.exe" ]; then cargo build --release >/dev/null; fi
if [ -f "$BIN.exe" ]; then BIN="$BIN.exe"; fi

README="$ROOT/README.md"
BEGIN="<!-- BEGIN:example -->"
END="<!-- END:example -->"

if ! grep -qF "$BEGIN" "$README" || ! grep -qF "$END" "$README"; then
  echo "ERROR: README.md is missing the example markers" >&2; exit 2
fi

TMP="$(mktemp)"
NEW="$(mktemp)"
trap 'rm -f "$TMP" "$NEW"' EXIT

{
  echo "$BEGIN"
  echo
  echo '```text'
  # The subject path is normalised so the block is identical on every machine.
  "$BIN" check tests/fixtures/cmp-04-report.md \
    | sed -e "s#tests/fixtures/cmp-04-report.md#cmp-04.md#"
  echo '```'
  echo
  echo "$END"
} > "$TMP"

# Splice the block in with awk: everything before BEGIN, the new block, then
# everything after END. No python, so this runs in any CI image.
awk -v begin="$BEGIN" -v end="$END" -v blockfile="$TMP" '
  BEGIN { while ((getline line < blockfile) > 0) block = block line "\n" }
  { if (index($0, begin) == 1) { printf "%s", block; skip = 1; next }
    if (skip) { if (index($0, end) == 1) skip = 0; next }
    print }
' "$README" > "$NEW"

if cmp -s "$README" "$NEW"; then
  echo "README examples are current"
  exit 0
fi
if [ "$MODE" = "check" ]; then
  echo "README examples are STALE - run scripts/regen-readme-examples.sh" >&2
  exit 1
fi
cp "$NEW" "$README"
echo "rewrote the README example block"
