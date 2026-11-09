# No network code — the verification

**The claim:** `viability-gate` makes no network calls. Target-specific input is
supplied at runtime and never leaves the machine.

**Why it is a hard guarantee, not a preference:** the whole point of the tool is
that you point it at your own private ledger of programs, scopes and prior
audits. A tool that could phone home would be a disclosure vector for exactly the
data the user is trying to keep local. "We don't send it anywhere" is a promise;
"there is no code that could" is a property.

## How it is verified

### 1. The dependency graph carries no network crate

```bash
cargo tree --edges normal
```

Every package in the graph, normal edges only, as of crate version 0.4.0:

```text
aho-corasick        annotate-snippets   anstream            anstyle
anstyle-parse       anstyle-query       anstyle-wincon      arraydeque
base64              cfg-if              clap                clap_builder
clap_derive         clap_lex            colorchoice         core_detect
encoding_rs         encoding_rs_io      granit-parser       heck
is_terminal_polyfill itoa               memchr              multiversion_no_op
nohash-hasher       num-traits          once_cell_polyfill  proc-macro2
quote               regex               regex-automata      regex-syntax
scopeguard          serde               serde-saphyr        serde_core
serde_derive        serde_json          simdutf8            smallvec
strsim              syn                 unicode-ident       unicode-width
utf8parse           viability-gate      windows-link        windows-sys
zmij
```

49 packages. **None of them is a network client, and none of them pulls one
transitively.** The five direct dependencies are:

| Direct dependency | What it does | Network capability |
|---|---|---|
| `clap` | Argument parsing | None |
| `regex` | Pattern matching | None |
| `serde` | Derive macros for (de)serialisation | None |
| `serde_json` | JSON output | None |
| `serde-saphyr` | Rule and ledger parsing (pure-Rust YAML) | None |

The transitive set is entirely made of parser internals (`regex-automata`,
`memchr`, `aho-corasick`, `granit-parser`, `encoding_rs`), error rendering
(`annotate-snippets`, `unicode-width`), formatting (`itoa`, `zmij`), proc-macro
plumbing (`syn`, `quote`, `proc-macro2`) and platform bindings (`windows-sys`,
`windows-link`, used by `anstream` for terminal colour detection).

0.4.0 replaced `serde_yaml` 0.9 and its C-translated `unsafe-libyaml`, both
unmaintained (RUSTSEC-2024-0449), with `serde-saphyr`. That raised the count from
39 to 49 packages; none of the ten additions has network capability.

**The denylist CI enforces** — if any of these ever appears in the graph, the
build fails:

```text
reqwest  hyper  ureq  curl  isahc  surf  attohttpc  tokio  async-std
native-tls  rustls  openssl  openssl-sys  tungstenite
```

`tokio` and `async-std` are on the list because they are the usual route by
which an HTTP client arrives transitively, and because an async runtime in an
offline, deterministic tool is a design regression even when it opens no socket.

### 2. The source contains no socket or URL code

```bash
grep -rniE '(TcpStream|UdpSocket|TcpListener|std::net|reqwest|hyper|ureq|curl|http://|https://api\.)' src/
```

The only matches this pattern has ever produced in `src/` are documentation
comments quoting example URLs (`https://example.invalid/...`) in the
`--known-issues` help text. Those are strings in a template, not requests.

CI runs both checks on every push:
[`.github/workflows/ci.yml`](../.github/workflows/ci.yml), job `no-network`.

### 3. The binary is inspected for the string table

```bash
cargo build --release
```

A linked HTTP client leaves its name in the binary. This is a weak check and it
is here because it is cheap and it catches a dependency that arrived without a
manifest change. It is not the primary evidence; the dependency graph is.

## What this verification does not prove

- **It does not prove the absence of a network call.** It proves the absence of a
  network *dependency*. A hand-written syscall would not show up in
  `cargo tree`. Nothing in `src/` does this, and the source grep is what covers
  it, but the graph alone is not the whole proof.
- **It is a point-in-time statement.** The list above is valid for the recorded
  commit. A future dependency can change it, which is why CI re-runs the check
  rather than citing this document.
- **It says nothing about the `vg bench` and `calibrate` fixtures.** The
  Python harness in `tools/calibrate/` runs a subprocess (`vg`) and reads local
  files; it opens no socket either, and it is not shipped in the binary.
- **`eval/run.py fetch` does use the network.** It downloads public contest
  issues so the benchmark can be re-run. It is not linked into `vg`, and `vg`
  never calls it. `eval/run.py score` only runs the local binary.

## Reproduce it

```bash
cargo tree --edges normal
cargo tree --edges normal --prefix none --no-dedupe | awk '{print $1}' | sort -u | wc -l
grep -rniE '(TcpStream|UdpSocket|TcpListener|std::net|reqwest|hyper|ureq|curl)' src/
```

Expected: no match from the grep, and a package count that matches the list above
for the recorded commit.
