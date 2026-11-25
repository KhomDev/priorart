#!/bin/bash -eu
# Build priorart's eight parser fuzz targets for OSS-Fuzz.
#
# cargo-fuzz builds in $SRC/priorart/fuzz (its own workspace), with nightly
# and AddressSanitizer. Every target is a libFuzzer binary; each one's property
# is "this parser never panics on any input". The target names match
# fuzz/fuzz_targets/*.rs and fuzz/corpus/<target>/.

cd "$SRC/priorart/fuzz"

# The pinned 0.2.0 commit's fuzz/Cargo.lock still names the root package 0.1.2,
# so `--locked` aborts before compiling. Cargo refreshes exactly that one entry
# from Cargo.toml and reuses every other locked dependency version; the
# committed lockfile is fixed in the P1 trust-infrastructure commit. Do not add
# --locked until PA_COMMIT points at a commit that contains the fix.
cargo metadata --format-version 1 > /dev/null

# -O: optimizations without debug assertions, the libFuzzer default profile in
# OSS-Fuzz. -s address: the one sanitizer project.yaml lists, stated explicitly
# rather than inferred from the base image.
cargo fuzz build -O -s address

out="$SRC/priorart/fuzz/target/x86_64-unknown-linux-gnu/release"
targets="markdown_import pdf_scrape html_normalize program_yaml known_issues_yaml ledger_yaml dossier_json finding_profile"
for target in $targets; do
  test -x "$out/$target"
  cp "$out/$target" "$OUT/"
  if [ -d "corpus/$target" ] && [ -n "$(ls -A "corpus/$target")" ]; then
    # -j: files at the zip root, which is the layout libFuzzer reads.
    zip -qjr "$OUT/${target}_seed_corpus.zip" "corpus/$target"
  fi
done
