# Faultline GitHub Action

A reusable composite GitHub Action that runs [faultline](https://github.com/EffortlessMetrics/faultline-rs) regression localization on any Git repository. It performs a binary search over commit history to pinpoint the exact commit that introduced a regression.

## Quick start

```yaml
- uses: EffortlessMetrics/faultline-rs/.github/actions/faultline@main
  with:
    good: abc1234
    bad: def5678
    cmd: "cargo test"
```

## Inputs

| Input          | Required | Default            | Description                                              |
|----------------|----------|--------------------|----------------------------------------------------------|
| `good`         | yes      |                    | Known-good commit SHA                                    |
| `bad`          | yes      |                    | Known-bad commit SHA                                     |
| `cmd`          | yes      |                    | Predicate command to run at each commit                  |
| `kind`         | no       | `test`             | Probe kind (`build`, `test`, `lint`, `perf-threshold`, `custom`) |
| `timeout`      | no       | `300`              | Timeout per probe in seconds                             |
| `output-dir`   | no       | `faultline-report` | Directory to write report artifacts into                 |
| `markdown`     | no       | `true`             | Generate a Markdown dossier alongside HTML/JSON          |
| `rust-version` | no       | `stable`           | Rust toolchain to install for building faultline         |

## Outputs

| Output       | Description                                                              |
|--------------|--------------------------------------------------------------------------|
| `exit-code`  | Faultline exit code (0=FirstBad, 1=SuspectWindow, 2=Error, 3=Inconclusive, 4=InvalidInput) |
| `report-dir` | Absolute path to the output directory                                    |
| `outcome`    | Outcome keyword: `FirstBad`, `SuspectWindow`, `Inconclusive`, or `unknown` |

## Exit code semantics

| Code | Meaning          | Action step result |
|------|------------------|--------------------|
| 0    | FirstBad found   | Success            |
| 1    | SuspectWindow    | Success            |
| 2    | Execution error  | Failure            |
| 3    | Inconclusive     | Success            |
| 4    | Invalid input    | Failure            |

Codes 0, 1, and 3 represent valid analysis outcomes. The action only fails the workflow step on codes 2 and 4.

## Full example

```yaml
name: Regression hunt
on:
  workflow_dispatch:
    inputs:
      good:
        description: "Known-good commit SHA"
        required: true
      bad:
        description: "Known-bad commit SHA"
        required: true

jobs:
  faultline:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - uses: EffortlessMetrics/faultline-rs/.github/actions/faultline@main
        id: fl
        with:
          good: ${{ github.event.inputs.good }}
          bad: ${{ github.event.inputs.bad }}
          cmd: "cargo test"

      - name: Show result
        run: |
          echo "Exit code: ${{ steps.fl.outputs.exit-code }}"
          echo "Outcome:   ${{ steps.fl.outputs.outcome }}"
          echo "Report:    ${{ steps.fl.outputs.report-dir }}"
```

## Posting results as a PR comment

When the `markdown` input is `true` (the default), faultline writes a
`dossier.md` file that is ideal for posting as a PR comment:

```yaml
      - uses: EffortlessMetrics/faultline-rs/.github/actions/faultline@main
        id: fl
        with:
          good: ${{ github.event.inputs.good }}
          bad: ${{ github.event.inputs.bad }}
          cmd: "cargo test"

      - name: Post dossier as PR comment
        if: github.event_name == 'pull_request'
        uses: marocchino/sticky-pull-request-comment@v2
        with:
          header: faultline
          path: faultline-report/dossier.md
```

## Artifacts

The action automatically uploads the report directory as a GitHub Actions
artifact named `faultline-report`. It contains:

- `analysis.json` -- machine-readable analysis results
- `index.html` -- interactive HTML report
- `dossier.md` -- Markdown summary (when `markdown: true`)
