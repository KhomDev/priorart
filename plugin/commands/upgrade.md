---
description: Upgrade priorart: the plugin through the marketplace, or the CLI to the newest crates.io release
allowed-tools: Bash(${CLAUDE_PLUGIN_ROOT}/bin/priorart-ensure:*), Bash(priorart --version)
---

Explain the two upgrade paths, then do the one the user chose:

- **Recommended: upgrade the plugin:** `/plugin marketplace update priorart`, then
  `/plugin update priorart`. The new plugin version pins the matching priorart release, and the next
  `/priorart:check` installs it. Plugin and CLI stay in lockstep.
- **CLI only, newest crates.io release:** run `${CLAUDE_PLUGIN_ROOT}/bin/priorart-ensure --latest`,
  then `priorart --version`. This can run ahead of the plugin's skill text; say so.
