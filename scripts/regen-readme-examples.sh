#!/usr/bin/env bash
# Regenerate the generated block in README.md, or check it with --check.
#
# Run from anywhere; paths are resolved from the script location. The same block
# is produced by tests/docs.rs from Rust, so a platform that cannot run bash
# still has the check. Windows path separators are normalised to / so the block
# is identical on every platform.
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$here"
mode="${1:-write}"
case "$mode" in
  write|--check) ;;
  *) echo "usage: $0 [--check]" >&2; exit 2 ;;
esac

cargo build --quiet
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

python3 - "$block" "$mode" <<'PY'
import pathlib, sys
root = pathlib.Path.cwd()
readme = root / "README.md"
block = pathlib.Path(sys.argv[1]).read_text(encoding="utf-8").rstrip("\n")
mode = sys.argv[2]
begin = "<!-- BEGIN GENERATED EXAMPLE (scripts/regen-readme-examples.sh) -->"
end = "<!-- END GENERATED EXAMPLE -->"
text = readme.read_text(encoding="utf-8")
if begin not in text or end not in text:
    raise SystemExit("README.md is missing the generated-example markers")
head, rest = text.split(begin, 1)
middle, tail = rest.split(end, 1)
current = middle.strip("\n")
if mode == "--check":
    if current != block:
        print("README example is stale. Regenerate with scripts/regen-readme-examples.sh", file=sys.stderr)
        print("--- expected ---", file=sys.stderr)
        print(block, file=sys.stderr)
        raise SystemExit(1)
    print("README example is current")
else:
    readme.write_text(head + begin + "\n" + block + "\n" + end + tail, encoding="utf-8")
    print("README.md example regenerated")
PY
rm -f "$block"
