# Install

One binary, named `priorart` (`priorart.exe` on Windows). There is no second
alias: the old `vg` and `viability-gate` binaries are gone, and the crate that
carried them is frozen at 0.9.0.

## From crates.io

```sh
cargo install priorart
# or, if you have cargo-binstall:
cargo binstall priorart
priorart --version   # priorart 0.1.0
```

Or straight from the release tag on GitHub:

```sh
cargo install --git https://github.com/KhomDev/priorart --tag v0.1.0
```

Rust 1.89 or newer (`rust-version` in `Cargo.toml`). No system dependencies: the
vocabularies and the rejection dataset are compiled into the binary with
`include_str!`, and the crate has no HTTP client. `pdftotext` is optional and
only needed for compressed PDFs ([dossier.md](dossier.md)).

## From a release

Each GitHub release carries archives for Linux, macOS and Windows with SHA256
sums. Unpack, put the binary on your PATH, and check it:

```sh
priorart --version
priorart vocab list      # 5 tier shapes, 14 clause classes
```

## From source

```sh
git clone https://github.com/KhomDev/priorart
cd priorart
cargo build --release
./target/release/priorart --version
```

## Verifying an install

```sh
cargo test --locked          # unit + integration tests, offline
priorart rejections summary  # reads its own compiled data
priorart ledger validate examples/ledger.example.yaml
```

## Upgrading and uninstalling

```sh
cargo install priorart --force   # upgrade in place
cargo uninstall priorart         # uninstall
```

Dossiers built by an older 0.1.x still load: the schema is `priorart.dossier/v1`
and `v1` is additive-only ([schema.md](schema.md)). A dossier does not need
rebuilding for a new binary unless the inputs changed.

## Moving from viability-gate

The `viability-gate` crate still installs at 0.9.0 and still works; it prints a
one-line stderr notice and is not yanked. See [MIGRATION.md](../MIGRATION.md) for
the command-by-command mapping.
