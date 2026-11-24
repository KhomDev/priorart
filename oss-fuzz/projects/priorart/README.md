# OSS-Fuzz local project: priorart

This is a local copy of the files an OSS-Fuzz project needs, kept in the
repository so the configuration is reviewable. **No upstream pull request has
been opened and nothing has been sent to OSS-Fuzz.** Submitting is a separate
operator decision.

## Targets

Eight cargo-fuzz targets under `fuzz/fuzz_targets/`:

| Target | Parser |
|---|---|
| `markdown_import` | the dossier markdown importer |
| `pdf_scrape` | the in-process PDF literal scraper |
| `html_normalize` | the saved-HTML normalizer |
| `program_yaml` | `program.yaml` |
| `known_issues_yaml` | `known-issues.yaml` |
| `ledger_yaml` | the ledger schema |
| `dossier_json` | dossier load |
| `finding_profile` | finding parsing |

Each target's property is "the parser never panics on any input". A crash is a
defect to fix, not a valid parse.

## Local build

From a checkout of this repository, with Docker:

```bash
docker build -t priorart-oss-fuzz oss-fuzz/projects/priorart
docker run --rm -v "$PWD/out:/out" priorart-oss-fuzz
```

The image pins `PA_COMMIT` in `Dockerfile`. The pinned 0.2.0 commit's
`fuzz/Cargo.lock` names the root package 0.1.2; `build.sh` runs `cargo metadata`
once to refresh that entry, and the P1 trust-infrastructure commit fixes the
lock. After the P1 tag exists, bump `PA_COMMIT` to the tag and add `--locked` to
`build.sh`.

## Upstream

`project.yaml` has `primary_contact: REPLACE_ME_MAINTAINER_EMAIL`. OSS-Fuzz
requires a deliverable address there, so it must be replaced by the operator
before any submission. No contact, PR or issue has been made from this
repository.
