# Install

Three ways. All three install the same code.

## 1. From crates.io

```bash
cargo install viability-gate
```

Published from 0.3.0 through 0.5.0. **0.6.0 is not on crates.io.** Install that
release from git (method 2) or from a checkout. The crate version of a
published release matches the `v*` tag it was published from.

## 2. From the repository

```bash
cargo install --git https://github.com/KhomDev/viability-gate
```

Every method installs **both** binaries, `vg` and `viability-gate`. They are the same
program; see [the name collision](#the-vg-name-collision) below.

Methods 1 and 2 compile from source and require **Rust 1.89 or newer**. The
MSRV is declared in `Cargo.toml` and checked in CI. The floor is set by the
YAML parser, `serde-saphyr`, which declares 1.89. It was 1.85 up to 0.3.0, when
the parser was `serde_yaml`. A release binary (method 3) needs no Rust at all.

To install from a local checkout instead:

```bash
git clone https://github.com/KhomDev/viability-gate
cd viability-gate
cargo install --path .
```

## 3. From a release binary

Releases are published on tags matching `v*`. Each release carries prebuilt
binaries for Linux, macOS and Windows, a `SHA256SUMS` file, and a build
provenance attestation.

```bash
# pick the asset for your platform from the release page, then:
sha256sum -c SHA256SUMS --ignore-missing
```

On macOS and Windows, `shasum -a 256` and `Get-FileHash -Algorithm SHA256` are
the equivalents. Verify the checksum before running the binary; that is what the
file is for.

The attestation (`gh attestation verify`) ties the binary to the workflow run
that built it. It proves the binary came from this repository's release
workflow — it does not prove the source is good, which is what
[`docs/calibration.md`](calibration.md) is for.

## The `vg` name collision

`vg` is already the name of an unrelated and widely packaged bioinformatics
tool — a variation-graph toolkit, distributed through Homebrew, conda and most
Linux distributions. If you have ever installed anything in that space, your
`PATH` may already have a `vg`, and `cargo install` will put this one somewhere
that shadows it or is shadowed by it.

That is why this crate ships **two binaries with the same code**:

| Binary | When to use it |
|---|---|
| `viability-gate` | Always safe. No known collision. Use this in scripts and CI |
| `vg` | Shorter, and matches every example in the docs. Use it if you have checked `which vg` |

```bash
which vg          # if this is not the tool you expect, use viability-gate
viability-gate check finding.md
```

Every command in the documentation takes either name.

## After installing

The rules are **embedded in the binary**, so there is no data directory to
populate and nothing to download. To point the tool at your own catalog instead:

```bash
vg --rules ./my-rules check finding.md
```

Start with:

```bash
vg init my-target/
```

which scaffolds the four input files a new target needs. See the
[`vg init` section of the README](../README.md#vg-init--start-here).

## Uninstall

```bash
cargo uninstall viability-gate
```

This removes both binaries, since they come from the same crate.
