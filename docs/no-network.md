# Offline by construction

priorart never touches the network. This is a design constraint, not a promise:

- `Cargo.toml` has no HTTP client crate. A test
  (`tests/cli.rs::the_crate_has_no_http_client`) fails if `reqwest`, `ureq`,
  `hyper`, `isahc`, `curl`, `surf`, `attohttpc`, `minreq` or `awc` appears -
  and fails if a source file opens a `TcpStream` or `UdpSocket`.
- `Cargo.lock` is checked for the same crate names, so a transitive addition
  cannot slip in unnoticed.
- Everything a program page says is a file **you** saved. There is no fetch
  command, no URL argument, and no cache to go stale. Fetching a platform page is
  that platform's terms question, and staleness is why this tool refuses to hold
  program data.
- The vocabularies and the rejection dataset are compiled into the binary with
  `include_str!`. There is no data directory to point at, and no update check.

The only place network access is allowed is the benchmark harness under
`priorart/eval/`, which fetches the public corpus by pointer. That directory is
not part of the tool and is not compiled into it.

## Why this matters

- It runs on an air-gapped machine, which is where audit material often lives.
- It cannot leak a private report to a third party.
- A dossier built six months ago still works six months from now, from the same
  file.
