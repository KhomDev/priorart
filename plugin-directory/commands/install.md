---
description: How to install the priorart CLI (you run the command; this plugin does not)
allowed-tools: Read, Grep, Glob, LS
---

This plugin does not install anything. Do not start a shell.

Tell the user to install the latest release themselves:

    cargo binstall priorart

or, if they do not have cargo-binstall:

    cargo install --locked priorart

Then `priorart --version` should report 0.2.3 or later.
