# GitHub Action

`action.yml` at the repository root is a composite action that installs a priorart release and runs
`priorart check` on one finding. It downloads the release archive for the runner, checks it against the
release's `SHA256SUMS`, and runs offline after that: `check` and `build` open no socket.

```yaml
- uses: actions/checkout@v4
- uses: KhomDev/priorart@v0.2.3
  with:
    finding: findings/reentrancy.md
    program: program/            # saved program pages; the action runs `priorart build` first
    # dossier: program/program.dossier   # or point at a dossier you built and committed
    fail-on-excluded: "true"
```

## Inputs

| Input | Default | Meaning |
|---|---|---|
| `finding` | (required) | The finding, as Markdown. |
| `dossier` | empty | A `.dossier` built by `priorart build`. Empty means the single `.dossier` beside the finding, or the one built from `program`. |
| `program` | empty | A program folder. When set, the action runs `priorart build` on it first. |
| `top-k` | `5` | How many ranked prior items to show. |
| `fail-on-excluded` | `false` | Fail the job when an exclusion clause class matches (`--fail-on-excluded`). |
| `fail-on-unpayable` | `false` | Fail the job when the claimed severity is below the program's floor (`--fail-on-unpayable`). |
| `version` | empty | The priorart release to run, e.g. `v0.2.2`. Empty means the release this action belongs to. |
| `binary` | empty | For testing the action: a priorart binary to run instead of downloading one. |

## Outputs

| Output | Meaning |
|---|---|
| `exit-code` | 0 clean, 1 a fail-on condition matched, 2 or more a usage or input error. |
| `version` | The priorart version that ran. |
| `report` | Path of the `priorart.check/v1` JSON report. |
| `text` | Path of the text report. The same text is in the job summary. |

The report is quoted evidence, not a verdict: see [gates.md](gates.md) for what each gate means and
[limitations.md](limitations.md) for what the numbers do not show.

## Runners

Linux x64 and arm64, macOS x64 and arm64, Windows x64: the platforms the release workflow builds. Any other
runner fails with exit code 2 before anything runs.

## Tests

The `action` job in `.github/workflows/ci.yml` runs the action on Ubuntu, macOS and Windows against
`examples/dossier-demo` and `examples/finding-demo.md`, checks the outputs and the JSON schema, and checks that
a missing finding gives exit code 2.
