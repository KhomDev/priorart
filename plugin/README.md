# priorart: Claude Code plugin

Checks a smart-contract audit or bug-bounty finding against a bounty program's
saved pages, known issues and prior audits with
[priorart](https://crates.io/crates/priorart), from inside Claude Code. It
answers three questions with quoted evidence and page numbers: has this been
found before, does the program exclude it, and can its claimed severity be
paid. It never gives a verdict.

## Install

```
/plugin marketplace add KhomDev/priorart
/plugin install priorart@priorart
```

Then `/priorart:install` (installs the CLI from crates.io through cargo) and
`/priorart:check finding.md --dossier program.dossier`.

## What it runs, sends and fetches

- `bin/priorart-ensure` installs exactly the priorart version this plugin
  pins (`cargo binstall` when present, else `cargo install --locked`). That
  install is the only network use, and it goes through cargo to crates.io (and
  GitHub release assets when binstall is used). `--latest` also reads the
  newest version number from the crates.io API.
- `priorart check` and `priorart build` read the files you name and write
  nothing but the dossier `build` creates next to your program directory. They
  make no network calls. The plugin does not run `priorart fetch`.
- The SessionStart hook runs `priorart-ensure --status`: it only reports a
  missing or mismatched CLI and never installs anything.

## Linked to the crate

The plugin's version **is** the priorart crates.io version it drives
(`.claude-plugin/plugin.json`). `tests/plugin.rs` in the repository fails if
the plugin, the marketplace entry and `Cargo.toml` disagree, or if the
plugin's skill differs from `skills/priorart/SKILL.md`. Set `PRIORART_BIN` to
use a binary you built yourself.

## MCP

The plugin does not declare an MCP server: Claude Code starts plugin MCP
servers at session start, and `priorart-mcp` exists only after
`/priorart:install`. After installing, add it yourself
(`claude mcp add priorart -- priorart-mcp`), as in
[`docs/mcp.md`](../docs/mcp.md). The skill names its eleven read-only tools.

## Commands

| Command | What it does |
|---|---|
| `/priorart:check <finding.md> --dossier <file>` | runs `priorart check` and reports the quoted evidence |
| `/priorart:install` | installs or repairs the pinned CLI |
| `/priorart:upgrade` | plugin or CLI upgrade |

Ranker and alarm figures priorart prints are DEV-ONLY (measured on data that
was already opened; no held-out figure yet). MIT licensed.
