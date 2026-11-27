# MCP server and Claude skill

`priorart-mcp` is an MCP server for priorart. It is listed in the MCP Registry
as `io.github.KhomDev/priorart` once a release that ships it is on crates.io
(see [Status](#status)). It ships in the same crate as the CLI, so one install
gives both binaries.

It is a wrapper, not a second implementation. Each tool runs one fixed
`priorart` command line and returns that command's stdout unedited (stderr
when stdout is blank). The server computes no number and rewrites no quote:
the integration tests compare every tool's text byte for byte with a direct
CLI run. Measured figures that priorart itself prints (ranker scores, alarm
calibration, benchmark numbers) keep their status: **DEV-ONLY**
([methodology.md](methodology.md), [benchmark.md](benchmark.md)).

## Install and configure

```sh
cargo install priorart        # installs priorart and priorart-mcp
priorart-mcp --version
```

Claude Code:

```sh
claude mcp add priorart -- priorart-mcp
```

Cursor, Claude Desktop, or any client with a JSON config:

```json
{
  "mcpServers": {
    "priorart": {
      "command": "priorart-mcp",
      "env": { "PRIORART_MCP_ROOT": "/path/to/your/audit/workspace" }
    }
  }
}
```

MCP Inspector:

```sh
npx @modelcontextprotocol/inspector --cli priorart-mcp --method tools/list
```

## Settings

Set by whoever configures the server, never by the client:

| Variable | Default | Effect |
|---|---|---|
| `PRIORART_BIN` | `priorart` next to `priorart-mcp`, then `PATH` | The CLI the tools run |
| `PRIORART_MCP_ROOT` | unset | Every path argument must resolve (symlinks followed) inside this directory, or the call is refused before priorart runs |
| `PRIORART_MCP_TIMEOUT_SECS` | 120 (1 to 3600) | A priorart run past this is killed and the call fails |

A bad value is exit 2 at start, naming the variable.

## Tools

All eleven are annotated read-only, non-destructive, idempotent and
closed-world. Each schema has `additionalProperties: false`, and the server
refuses an argument the schema does not list instead of ignoring it.

| Tool | Command line | Output |
|---|---|---|
| `check` | `check --json [--dossier F] [--top-k N] [--fail-on-excluded] [--fail-on-unpayable] -- FINDING` | `priorart.check/v1` JSON |
| `candidates` | `candidates --json [--corpus P] [--repo D]... [--index F]... [--repo-id ID] [--commit SHA] [--top-k N] [--exclude ID]... [--ranker F] [--alarm F] [--prose-anchors] -- FINDING` | `priorart.candidates/v1` JSON; ranker and alarm figures are DEV-ONLY |
| `anchors_resolve` | `anchors resolve --json --repo D... [--repo-id ID] [--commit SHA] --text-file F` | `priorart.anchors/v1` JSON |
| `anchors_index` | `anchors index --json [--repo-id ID] [--commit SHA] -- REPO` | index JSON |
| `taxonomy_classify` | `taxonomy classify --json -- FILE...` | taxonomy JSON |
| `ledger_validate` | `ledger validate --json [--strict] [--example-mode] -- LEDGER...` | validation JSON |
| `vocab_list` | `vocab list --json` | vocabulary JSON |
| `vocab_show` | `vocab show --json -- tier-shapes\|clause-classes\|gate-crossref` | vocabulary JSON |
| `explain` | `explain -- ID` | text |
| `rules` | `rules` | text |
| `rejections_summary` | `rejections summary --json` | dataset summary JSON |

Not offered: `fetch` (the one networked command), `build` and `init` (they
write files), `candidates --prepare` (writes a corpus) and `anchors bench`
(an evaluation run). Build the dossier yourself with `priorart build`, then
point `check` at it.

`isError` is priorart's exit status: 0 is a result, anything else is an
error. With `fail_on_excluded` or `fail_on_unpayable` that includes exit 1
when the clause or the floor matches ([cli.md](cli.md)). A refused argument
is a JSON-RPC `-32602` error and priorart is not run. A run that times out,
writes more than 4 MiB on a stream, or cannot start is a result with
`isError: true` that says which.

Every tool text starts with one line saying the rest is tool output, not
instructions. Quotes come from files the user saved, and a program page or an
audit can contain text that addresses an agent.

## Argument rules

- Paths: non-empty, at most 4096 bytes, no newline or NUL, and not starting
  with `-`, so no value can be read as a flag. Positional paths follow `--`.
- `repo_id`: `owner/name`, letters, digits, `.`, `_`, `-`, neither part
  starting with `-`.
- `commit`: 7 to 64 hex characters.
- `top_k`: integer 1 to 1000.
- `id` and `exclude` entries: 1 to 256 bytes, not starting with `-`, no
  control characters.
- Lists (`repo`, `index`, `exclude`, `files`, `ledgers`): at most 64 entries.
- Types are strict: `"3"` is not an integer and `"yes"` is not a boolean.

## Transport

Newline-delimited JSON-RPC 2.0 on stdin and stdout, the MCP stdio transport.
A request framed with a `Content-Length` header is also read, and answered in
the same framing. `initialize` echoes the client's protocol revision when it
is `2025-06-18`, `2025-03-26` or `2024-11-05`, and offers `2025-06-18`
otherwise. Methods: `initialize`, `tools/list`, `tools/call`, `ping`;
notifications get no answer.

Limits: 8 MiB per line or frame (answered with `-32600`, then exit 2), 8 KiB
per header line, 64 header lines. A malformed line gets `-32700` and the
session goes on. priorart's stdin is null, so it can never read the MCP
stream.

## The Claude skill

[`skills/priorart/SKILL.md`](../skills/priorart/SKILL.md) tells an agent when
to use priorart and how to read it: three questions, quoted evidence, no
verdict, DEV-ONLY figures. It works with the MCP tools or with the CLI.

## Status

- The server ships from the release after 0.2.2. Until that release is on
  crates.io, `cargo install priorart` gives the CLI only, and the registry
  listing is not live: the registry checks the `mcp-name:` line in the README
  that crates.io shows, and 0.2.2's README has none.
- `server.json` in the repository root is the registry entry. It is published
  by hand with `mcp-publisher` after the crates.io release.

## Tests

`src/mcp.rs` unit tests, `tests/mcp_props.rs` (proptest), `tests/mcp_bounded.rs`
(the timeout and the output cap against a real child) and `tests/mcp_stdio.rs`
(the real binary behind a newline client, every tool compared byte for byte
with the CLI). See [testing.md](testing.md).
