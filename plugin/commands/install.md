---
description: Install or repair the priorart CLI pinned to this plugin's version (from crates.io)
allowed-tools: Bash(${CLAUDE_PLUGIN_ROOT}/bin/priorart-ensure:*), Bash(priorart --version)
---

Run `${CLAUDE_PLUGIN_ROOT}/bin/priorart-ensure`, then `priorart --version`, and report the installed
version and path. This downloads from crates.io through cargo; say so before running it.
