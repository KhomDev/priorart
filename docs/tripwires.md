# Tripwires

`scripts/check_tripwires.py` is a repo-wide check with three parts. It runs in CI
and it is meant to be run before every commit:

```bash
python scripts/check_tripwires.py          # human-readable
python scripts/check_tripwires.py --json   # machine-readable
```

Exit code 0 means clean. It fails closed: anything it cannot read is reported as a
violation, not skipped silently.

## A. Private strings

No file in this repository may contain a private-workspace path prefix. The
prefixes are held in the checker **assembled from fragments**, so that the
checker does not itself contain the strings it forbids — otherwise the checker
would be the first violation it reports.

The scan covers every file git would commit: tracked files plus untracked files
that are not gitignored. Binary files (any file containing a NUL byte) are
skipped; the strings are ASCII and do not occur in the binary assets this repo
does not have anyway.

This is why no page in this repository quotes the private workspace's doctrine
paths, including the pages that explain the tripwire. `rules/anti-patterns.yaml`
records each rule's source as `doc: docs/rules/Pn.md` — a path inside this
repository — instead of a private one.

## B. Pack balance

`rules/packs/` must contain at least **7** ecosystem packs, and the largest pack
must not carry more than **2×** the entries of the smallest. Entries are
`len(tokens) + len(path_globs) + len(test_markers)`.

The reason is not tidiness. A catalog that knows one ecosystem's vocabulary tells
every reader which ecosystem its author hunts. A balanced pack set means the
*presence* of any one ecosystem's tokens says nothing about the author — only
that the catalog covers that ecosystem.

See [`rules/packs/README.md`](../rules/packs/README.md).

## C. Token confinement

Ecosystem tokens declared by the packs must not appear in the **shipped rule
catalog**:

| File | Why it is in the surface |
|---|---|
| `rules/anti-patterns.yaml` | Embedded into the binary (`include_str!`) and read by every user |
| `rules/gates.yaml` | Embedded into the binary and read by every user |

A file extension is also matched in its bare form, because a pattern written as a
dotted alternation of extensions carries exactly the same disclosure as the
dotted form. Bare forms shorter than three characters (`go`, `rs`, `ts`) are not
matched: they collide with ordinary words.

### What the check deliberately excludes, and why

| Excluded | Why |
|---|---|
| `rules/packs/**` | The allowed home for ecosystem tokens. |
| `tests/**` | Fixtures exist to exercise token-bearing patterns. A fixture that may not name the token cannot test it. Test fixtures are not embedded in the binary and are not part of the shipped catalog. |
| `src/**` | Rust is the host language of this tool. `assert!(` appears in ordinary unit tests, so it is not an ecosystem signal — it is not a pack token either, for the same reason. |
| `docs/**`, `README.md`, `examples/**` | Prose. The disclosure risk is the shipped *rule*, not a sentence describing it. |
| `scripts/check_tripwires.py` | The checker reads the token list from the packs; it hard-codes nothing. |

**This is narrower than "ecosystem tokens only under `rules/packs/`".** It is
narrowed to the surface where the disclosure actually happens — the catalog that
ships inside the binary — because the literal rule cannot be satisfied by a repo
whose own test fixtures must name the tokens they test. If the fixtures are ever
relocated into `rules/packs/`, the surface list at the top of
`scripts/check_tripwires.py` can be widened to the whole repository in one line.

## E. Case classes

Every catalog pattern is compiled case-insensitive (`(?i)` is prefixed by the
engine). A character class such as `[A-Z]` therefore also matches lower case,
unless it sits inside a `(?-i:...)` group that switches the flag off. P13's ticker
suppression (`\d ?[A-Z]{2,6}`) shipped that way and matched "2 of", which turned
the rule off on almost every real finding until 0.5.0.

The check strips every `(?-i:...)` group from each pattern in
`rules/anti-patterns.yaml` and fails if an `[A-Z` class remains.
