# Security policy

## What counts as a security issue here

priorart is an **offline, deterministic, evidence-only tool**. It ships no
program data, makes no network calls and stores nothing. That rules out most of
the usual categories and leaves three that matter.

### 1. A tracked file discloses a target

The vocabularies are generic by design, and the benchmark stores pointers and
labels only (`repo@commit` plus a path). A clause, example, fixture or doc that
names a private target — or narrows a candidate set to one — is a disclosure, and
the affected text is already published. Report it by email rather than in a
public issue if the text identifies a program.

### 2. A private-workspace string reaches a tracked file

This is the leak class the release checklist exists to catch: absolute local
paths, private classifier output, blocklist tokens or private log rows in a
tracked file or in git history. The scans and the commands are in
[CONTRIBUTING.md](CONTRIBUTING.md) ("Releasing", step 6). If one got through,
report it the same way as a disclosure.

### 3. Something in the tool reaches the network

The offline property is a hard guarantee, not a preference: everything priorart
reads is a file you saved, and a dossier must not leave the machine. It is
enforced by `tests/cli.rs::the_crate_has_no_http_client` (no HTTP client crate
in `Cargo.toml` or `Cargo.lock`, no `std::net` or socket call in `src/`) and
explained in [docs/no-network.md](docs/no-network.md). A way to make the tool
open a socket, or a dependency that brings one in, is a security issue.

## What does not count

- **A wrong prior item, a missed duplicate, a wrong gate reading.** The matcher
  and the gates are heuristics with published error rates
  ([docs/benchmark.md](docs/benchmark.md), [docs/limitations.md](docs/limitations.md)).
  File that as a [bug](https://github.com/KhomDev/priorart/issues/new/choose),
  not as a security issue.
- **A dependency advisory.** Dependabot and `cargo deny` cover those.
- **A deliberate design choice** that a gate reports evidence instead of a
  verdict. That is the product, not a vulnerability.

## Reporting

Report privately through GitHub: open the repository's **Security** tab and
choose **Report a vulnerability**
(<https://github.com/KhomDev/priorart/security/advisories/new>). The report is
visible only to the maintainer until an advisory is published.

Do not open a public issue for a disclosure report: a public issue republishes
the text you are reporting.

## What to include

- The file and line, or the release tag and the exact string.
- Why you believe it is a disclosure, a leak or a network path — not just that it
  looks odd.
- If it is a disclosure: whether the text is in the current release, which
  version first carried it, and whether it is in git history.

## Response

There is no service-level commitment. This is a single-maintainer project, and
the policy says so rather than implying otherwise.

For a disclosure report, the maintainer will:

1. Confirm or dispute the disclosure within a reasonable period.
2. If confirmed, remove the text, cut a patch release, and note the removal in
   `CHANGELOG.md` **without repeating the removed text**.
3. Note that removing it from `main` does not unpublish it: git history, the
   crates.io tarball, the GitHub release and the binary retain it. That is stated
   plainly because it affects how urgent a fix is, and because pretending
   otherwise would be dishonest. Published versions are never yanked.
