# Security policy

## What counts as a security issue here

This is an **offline, deterministic, advisory tool**. It ships no data, makes no
network calls and stores nothing. That rules out most of the usual categories and
leaves three that matter.

### 1. A shipped rule discloses a target

**This is the most serious report you can file.** The catalog is generic by
design: it carries no program names, no chains, no reward tables, and no
repository paths. A rule, pattern, docstring, example or test fixture that names
a real target — or narrows a candidate set to one — is a disclosure, and the
affected text is already published.

Report it by email rather than in a public issue if the disclosure is specific
enough to identify a program.

### 2. A private-workspace string reaches a tracked file

The repository is checked by [`scripts/check_tripwires.py`](../scripts/check_tripwires.py),
which fails on any private-workspace path prefix. If you find one that got
through, report it the same way.

### 3. Something in the tool reaches the network

The no-network property is a hard guarantee, not a preference — target-specific
input is supplied at runtime and must never leave the machine. It is verified in
CI by `cargo tree`, a source grep and a binary string check; see
[`docs/no-network.md`](../docs/no-network.md). A way to make the tool open a
socket is a security issue.

## 4. A path argument is parsed as an option

`vg` rejects a path, commit, or id that starts with `-` (stdin remains `-`).
Pass `--` before positional findings. A `--commit` value must be a 7 to 64
digit hex SHA, and git is invoked with `--end-of-options` and an argument
list, not a shell. This crate has no MCP server. A model-controlled string
spliced into argv must not become `--rules` or another flag.

## What does not count

- **A wrong verdict.** The tool is a heuristic with an unmeasured false-positive
  rate. It will be wrong. File that as a
  [bug](https://github.com/KhomDev/viability-gate/issues/new/choose) or a
  [rule proposal](https://github.com/KhomDev/viability-gate/issues/new/choose),
  not as a security issue.
- **A false positive.** Same answer. Every rule page has a "known
  false-positive modes" section; if yours is not listed, that is a documentation
  bug worth reporting.
- **A dependency advisory.** Dependabot and `cargo audit` cover those.

## Reporting

Report privately through GitHub: open the repository's **Security** tab and choose
**Report a vulnerability**
(<https://github.com/KhomDev/viability-gate/security/advisories/new>). The report
is visible only to the maintainer until an advisory is published.

Do not open a public issue for a disclosure report: a public issue republishes the
text you are reporting.

## What to include

- The file and line, or the rule id.
- Why you believe it is a disclosure, a leak or a network path — not just that it
  looks odd.
- If it is a disclosure: whether the affected text is in the current release, and
  in which version it first appeared.

## Response

There is no service-level commitment. This is a single-maintainer project, and
the policy says so rather than implying otherwise.

For a disclosure report, the maintainer will:

1. Confirm or dispute the disclosure within a reasonable period.
2. If confirmed, remove the text, cut a release, and note the removal in
   `CHANGELOG.md` **without repeating the removed text**.
3. Note that removing it from `main` does not unpublish it: git history and any
   released binary retain it. That is stated plainly because it affects how
   urgent a fix is and because pretending otherwise would be dishonest.
