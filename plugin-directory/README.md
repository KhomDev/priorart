# priorart — Anthropic Directory plugin

A tighter Claude Code plugin for
[priorart](https://crates.io/crates/priorart). It ships the skill and
read-only commands. It does **not** install or run the CLI: you run
`cargo binstall priorart` (or `cargo install --locked priorart`) and
the checker yourself, then paste or save the output.

This is the bundle submitted to the Anthropic plugin directory. The Directory
does not accept hooks, a `bin/` installer, or shell `allowed-tools` from an
unverified publisher. The full plugin, which can install and run the pinned
CLI, stays in [`plugin/`](../plugin/) and is unchanged.

The privacy policy is in [PRIVACY.md](PRIVACY.md): this plugin does not
collect, store or send any data.

## Install

From this repository, in Claude Code, add the marketplace and install the
plugin, or load the `plugin-directory/` path the Directory listing names.

Then `/priorart:install` (shows the cargo command; it does not run it)
and `/priorart:check finding.md --dossier program.dossier` (shows the check
command; you run it).

## What is in this tree

- `skills/priorart/SKILL.md` — the skill, worded so the agent asks you to
  run the CLI instead of running it itself.
- `commands/check.md`, `install.md`, `upgrade.md` — the same three commands
  as the full plugin, with `allowed-tools` limited to `Read`, `Grep`, `Glob`
  and `LS`.
- `.claude-plugin/plugin.json` — name, version and the Directory listing
  URLs (privacy policy, docs and support).

There is no `hooks/` directory, no `bin/` directory, and no script.

`tests/plugin_directory.rs` fails if a hook, a `bin/` path, a disallowed
tool, a `$` in `allowed-tools`, an image or font reference, or a missing
privacy-policy URL is added.
