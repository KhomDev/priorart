#!/usr/bin/env bash
# Regenerate the generated blocks in README.md, or check them with --check.
#
# Run from anywhere; paths are resolved from the script location. The same
# blocks are produced by tests/docs.rs from Rust, so a platform that cannot run
# bash still has the check. Windows path separators are normalised to / so the
# blocks are identical on every platform.
#
# On a host whose default cargo targets MSVC, name the toolchain to build with:
#   PRIORART_CARGO_TOOLCHAIN=stable-x86_64-pc-windows-gnu \
#       scripts/regen-readme-examples.sh
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$here"
mode="${1:-write}"
case "$mode" in
  write|--check) ;;
  *) echo "usage: $0 [--check]" >&2; exit 2 ;;
esac

cargo_cmd=(cargo)
if [ -n "${PRIORART_CARGO_TOOLCHAIN:-}" ]; then
  cargo_cmd=(cargo "+${PRIORART_CARGO_TOOLCHAIN}")
fi

"${cargo_cmd[@]}" build --quiet
bin="./target/debug/priorart"
[ -x "$bin" ] || bin="./target/debug/priorart.exe"

block="$(mktemp)"
{
  echo '$ priorart build examples/dossier-demo'
  "$bin" build examples/dossier-demo
  echo
  echo '$ priorart check examples/finding-demo.md --dossier examples/dossier-demo/dossier-demo.dossier'
  "$bin" check examples/finding-demo.md --dossier examples/dossier-demo/dossier-demo.dossier
} | sed 's|\\|/|g' > "$block"

html_block="$(mktemp)"
{
  echo '$ priorart build tests/fixtures/html-program'
  "$bin" build tests/fixtures/html-program
  echo
  echo '$ priorart check tests/fixtures/html-finding.md --dossier tests/fixtures/html-program/html-program.dossier'
  "$bin" check tests/fixtures/html-finding.md --dossier tests/fixtures/html-program/html-program.dossier
} | sed 's|\\|/|g' > "$html_block"

python=""
for candidate in python3 python; do
  # A Windows Store alias can answer `command -v` and then fail to run.
  if command -v "$candidate" >/dev/null 2>&1 &&
     "$candidate" -c 'import sys' >/dev/null 2>&1; then
    python="$candidate"
    break
  fi
done
if [ -z "$python" ]; then
  echo "a Python interpreter (python3 or python) is required" >&2
  exit 2
fi

"$python" - "$block" "$html_block" "$mode" <<'PY'
import pathlib, sys
root = pathlib.Path.cwd()
readme = root / "README.md"
blocks = {
    "<!-- BEGIN GENERATED EXAMPLE (scripts/regen-readme-examples.sh) -->": (
        "<!-- END GENERATED EXAMPLE -->",
        pathlib.Path(sys.argv[1]).read_text(encoding="utf-8").rstrip("\n"),
    ),
    "<!-- BEGIN GENERATED HTML EXAMPLE (scripts/regen-readme-examples.sh) -->": (
        "<!-- END GENERATED HTML EXAMPLE -->",
        pathlib.Path(sys.argv[2]).read_text(encoding="utf-8").rstrip("\n"),
    ),
}
mode = sys.argv[3]
text = readme.read_text(encoding="utf-8")
for begin, (end, block) in blocks.items():
    if begin not in text or end not in text:
        raise SystemExit("README.md is missing a generated-example marker: " + begin)
    head, rest = text.split(begin, 1)
    middle, tail = rest.split(end, 1)
    current = middle.strip("\n")
    if mode == "--check":
        if current != block:
            print("README example is stale. Regenerate with scripts/regen-readme-examples.sh", file=sys.stderr)
            print("--- expected ---", file=sys.stderr)
            print(block, file=sys.stderr)
            raise SystemExit(1)
    else:
        text = head + begin + "\n" + block + "\n" + end + tail
if mode == "--check":
    print("README examples are current")
else:
    # LF endings on every platform, matching the generated text and git.
    readme.write_text(text, encoding="utf-8", newline="\n")
    print("README.md examples regenerated")
PY
rm -f "$block" "$html_block"
