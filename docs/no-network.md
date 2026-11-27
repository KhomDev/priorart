# Offline, except one command

No command except `priorart fetch` opens a socket. `fetch` is explicit: you
name the HTTPS URLs, it performs one GET each, and it does not crawl, cache or
refresh. `build`, `check`, `candidates`, `anchors`, `taxonomy` and every other
command read files only.

`--no-default-features` compiles a binary with no HTTP client at all. The
client is `ureq` 2 with rustls, behind the `fetch` feature (on by default), and
it is referenced only from `src/fetch/`.

- `tests/cli.rs::the_http_client_is_only_the_fetch_feature` fails if another
  HTTP crate appears in `Cargo.toml` or `Cargo.lock`, if `ureq` is named
  outside `src/fetch/ureq_client.rs`, or if any source file mentions
  `TcpStream`, `UdpSocket` or `std::net`.
- `cargo tree --locked --no-default-features` contains no `ureq`, `reqwest`,
  `hyper`, `rustls` or `tokio`. The CI job `no-network` runs that tree, builds
  and tests `--no-default-features`, and runs `build`, `check`, `candidates`,
  `anchors` and `taxonomy` with proxy variables pointed at a closed port.
- `fetch` does not read `http_proxy`, `https_proxy`, `all_proxy` or their
  uppercase forms (`proxy-from-env` is off and the agent sets
  `try_proxy_from_env(false)`). It sends no cookies and no `Authorization`
  header. The trust anchors are the Mozilla list in `webpki-roots`, not the
  operating system's store. The limits are HTTPS only, no IP-literal or
  localhost host, at most 5 redirects, 30 seconds, and a 16 MiB body.
- A page `fetch` saved is a file. `build` does not request it again. priorart
  does not crawl, does not keep a refresh cache, and does not decide whether
  downloading a platform page is allowed by that platform's terms.
- The vocabularies and the rejection dataset are compiled into the binary with
  `include_str!`. There is no rules download and no update check.

Where this sits among the other claims is [threat-model.md](threat-model.md) (G1).

The benchmark harness under `eval/` also fetches, by pointer, and is not
compiled into the tool.

## Why this matters

- It runs on an air-gapped machine, which is where audit material often lives.
- It cannot leak a private report to a third party.
- A dossier built six months ago still works six months from now, from the same
  file.
