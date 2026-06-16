# faultline

[![CI Fast](https://github.com/EffortlessMetrics/faultline-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/EffortlessMetrics/faultline-rs/actions/workflows/ci.yml)
[![Codecov](https://codecov.io/gh/EffortlessMetrics/faultline-rs/branch/main/graph/badge.svg)](https://codecov.io/gh/EffortlessMetrics/faultline-rs)
[![RIPR](https://img.shields.io/endpoint?url=https://raw.githubusercontent.com/EffortlessMetrics/faultline-rs/main/badges/ripr.json)](docs/VERIFICATION.md)
[![MSRV](https://img.shields.io/badge/MSRV-1.93-blue.svg)](Cargo.toml)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)

**faultline** is a local-first regression archaeologist for Git repositories.

Given a known-good boundary, a known-bad boundary, and a predicate you already trust,
`faultline` narrows the honest failure window and emits a portable artifact showing where to start reading.

## Mission

Compress regression archaeology from hours of senior attention into one deterministic artifact.

## Vision

Every red build should come with a black box recorder.

## Product promise

`faultline` does three things:

1. Walks history safely between a known-good and known-bad boundary.
2. Runs the same predicate you already trust at candidate revisions.
3. Leaves behind JSON + HTML artifacts that explain the narrowest credible regression window.

It does **not** pretend to know the root cause.
When the history is messy, it returns a suspect window instead of fake precision.

## Workspace layout

- `faultline-codes` — shared diagnostic / ambiguity vocabulary
- `faultline-types` — pure shared value objects and report model
- `faultline-localization` — regression-window engine
- `faultline-surface` — coarse changed-surface bucketing
- `faultline-ports` — outbound hexagonal ports
- `faultline-app` — use-case orchestration
- `faultline-git` — Git history and checkout adapters
- `faultline-probe-exec` — process execution adapter
- `faultline-store` — filesystem-backed run store
- `faultline-render` — JSON + HTML artifact writers
- `faultline-cli` — operator-facing CLI
- `faultline-fixtures` — fixture builders for BDD-style scenarios

## Current scope

This v0.1 implementation is deliberately narrow.

### Included

- known-good / known-bad explicit boundaries
- ancestry-path and first-parent history linearization
- disposable Git worktrees per probe
- structured probe execution with timeout handling
- persistent observation store for reruns
- exact boundary or suspect-window localization
- coarse path-based subsystem bucketing
- JSON + HTML report generation

### Deliberately excluded

- GitHub / CI provider APIs
- organization-wide incident management
- AI-written root-cause analysis
- AST-aware topology inference
- automatic fixes or patch generation

## Quickstart

Find the first commit that broke a test:

```bash
cargo run -p faultline-cli -- \
  --repo . \
  --good abc1234 \
  --bad def5678 \
  --kind test \
  --timeout-seconds 300 \
  --cmd "cargo test -p my_crate failing_test" \
  --output-dir ./faultline-report
```

Or use direct exec mode (no shell wrapper):

```bash
cargo run -p faultline-cli -- \
  --repo . \
  --good abc1234 \
  --bad def5678 \
  --kind build \
  --timeout-seconds 300 \
  --program cargo \
  --arg build \
  --arg --workspace \
  --output-dir ./faultline-report
```

Replace `abc1234` and `def5678` with real commit SHAs from your repository.
On success, faultline writes `analysis.json` and `index.html` to the output directory and prints a summary:

```
run-id       a1b2c3d4
observations 7
output-dir   ./faultline-report
artifacts    ./faultline-report/analysis.json
             ./faultline-report/index.html
history      ancestry-path
outcome      FirstBad  last_good=abc1234 first_bad=def5678 confidence=95(high)
```

### Additional flags

- `--resume` — continue an interrupted run (default behavior)
- `--force` — discard cached observations and re-probe
- `--fresh` — delete the entire run directory and start from scratch
- `--no-render` — skip HTML report generation, produce only `analysis.json`
- `--shell <sh|cmd|powershell>` — select the shell for `--cmd` predicates
- `--env KEY=VALUE` — inject environment variables into the predicate (repeatable)
- `--first-parent` — use first-parent linearization instead of ancestry-path
- `--max-probes <n>` — cap probe executions (default: 64)
- `--retries <n>` — number of probe retries for flake detection (default: 0)
- `--stability-threshold <f>` — minimum proportion of consistent results to classify as stable (0.0–1.0, default: 1.0)
- `--markdown` — also write a Markdown dossier (`dossier.md`) alongside HTML/JSON
- `--unsafe-include-env` — include raw environment variable values in shareable artifacts (UNSAFE: may leak secrets)
- `--unsafe-include-output` — include raw stdout/stderr output without secret scrubbing (UNSAFE: may leak tokens)

Run `cargo run -p faultline-cli -- --help` for the full flag reference.

### Environment variable injection

Use `--env KEY=VALUE` to inject environment variables into the predicate process. The flag is repeatable:

```bash
faultline \
  --good abc1234 \
  --bad def5678 \
  --cmd "make test" \
  --env DB_HOST=localhost \
  --env DB_PORT=5432
```

Injected variables are recorded in the report and reproduction capsules. By default, their values are redacted with `[REDACTED]` in all shareable artifacts (`analysis.json`, `index.html`, `dossier.md`, SARIF, JUnit). Pass `--unsafe-include-env` to include raw values.

### Shell selection

The `--shell` flag selects the shell used to execute `--cmd` predicates:

| Value | Shell |
|-------|-------|
| `sh` | POSIX `/bin/sh` |
| `cmd` | Windows `cmd.exe` |
| `powershell` | PowerShell |

When omitted, faultline uses the platform default. The shell choice is also recorded in reproduction capsules.

### Flake-aware probing

When `--retries` is set to a value greater than 0, faultline runs the predicate multiple times per revision to detect flaky tests. Each revision is probed `1 + retries` times, and the results are aggregated using majority-vote logic.

```bash
faultline \
  --good abc1234 \
  --bad def5678 \
  --cmd "cargo test flaky_test" \
  --retries 2 \
  --stability-threshold 0.8
```

The `--stability-threshold` flag (range: 0.0 to 1.0) determines the minimum proportion of the most-frequent result class needed to classify an observation as stable. For example, with `--retries 2` (3 total runs) and `--stability-threshold 0.8`, at least 3/3 results must agree for the observation to be marked stable. With `--stability-threshold 0.6`, 2/3 agreement suffices.

Unstable observations (where the threshold is not met) reduce confidence in the localization outcome and may cause faultline to return a suspect window instead of an exact first-bad commit.

See [FlakeSignal](#flakesignal) below for the data structure produced by flake-aware probing.

### Markdown dossier generation

Pass `--markdown` to produce a Markdown dossier (`dossier.md`) alongside the standard `analysis.json` and `index.html`:

```bash
faultline \
  --good abc1234 \
  --bad def5678 \
  --cmd "cargo test" \
  --markdown \
  --output-dir ./faultline-report
```

The dossier is a self-contained Markdown document suitable for pasting into issues or sharing with teammates.

### Redaction and safety flags

By default, all shareable artifacts redact environment variable values and scrub secret-like patterns (GitHub tokens, AWS keys, Stripe keys, Bearer tokens, `password=` values) from stdout/stderr excerpts.

Two independent flags override this behavior:

- `--unsafe-include-env` — preserves raw environment variable values in all shareable artifacts. Use only when you know the values are safe to share.
- `--unsafe-include-output` — disables secret pattern scrubbing in stdout/stderr fields. Use only when you need full output for debugging.

These flags are independent: you can include raw env values while still scrubbing output secrets, or vice versa.

## Defaults

| Setting | Default | Notes |
|---------|---------|-------|
| `max_probes` | 64 | Maximum number of probe executions before returning a suspect window |
| `timeout_seconds` | 300 | Per-probe timeout; exceeded probes are classified as Indeterminate |
| `output_truncation` | 64 KiB | Probe stdout/stderr truncated in observations; full output saved to log files |
| Output directory | `faultline-report` | Configurable via `--output-dir` |

## Operator contract

The predicate should be monotonic enough across the selected history range.
If it flakes, times out, or depends on mutable external state, `faultline` will reduce confidence and may return a suspect window instead of an exact first-bad commit.

## CI and Coverage

Codecov is execution-surface telemetry only; see [Coverage](docs/ci/coverage.md) for what the badge does and does not claim.

## Packaging

faultline v0.1 is a **source-only release**. Install by cloning the repository and building with `cargo build --release`. See [BUILDING.md](BUILDING.md) for prerequisites and commands. Prebuilt binaries are not provided for v0.1.

## Export commands

All export commands use the shared Report Locator, which accepts directories containing either `report.json` or `analysis.json`. When a directory contains both, `report.json` takes precedence and a diagnostic note is emitted to stderr.

### cargo xtask export-markdown

Export a Markdown dossier from a completed run:

```bash
cargo xtask export-markdown --run-dir .faultline/runs/<fingerprint>
```

Write to a file instead of stdout:

```bash
cargo xtask export-markdown --run-dir .faultline/runs/<fingerprint> --output dossier.md
```

### cargo xtask export-sarif

Export SARIF v2.1.0 output from a completed run:

```bash
cargo xtask export-sarif --run-dir .faultline/runs/<fingerprint>
```

Write to a file:

```bash
cargo xtask export-sarif --run-dir .faultline/runs/<fingerprint> --output results.sarif.json
```

### cargo xtask export-junit

Export JUnit XML output from a completed run:

```bash
cargo xtask export-junit --run-dir .faultline/runs/<fingerprint>
```

Write to a file:

```bash
cargo xtask export-junit --run-dir .faultline/runs/<fingerprint> --output results.junit.xml
```

### CLI export-markdown

The CLI also provides a direct `export-markdown` subcommand:

```bash
faultline export-markdown --run-dir .faultline/runs/<fingerprint>
```

All export commands apply the default redaction policy (env values masked, secrets scrubbed). The `--unsafe-include-env` and `--unsafe-include-output` flags on the CLI override this for `export-markdown` and `reproduce`.

## Reproduction capsules

A `ReproductionCapsule` captures everything needed to reproduce a probe at a specific commit:

| Field | Description |
|-------|-------------|
| `commit` | The Git commit SHA to check out |
| `predicate` | The probe specification (shell command or exec program + args) |
| `env` | Environment variables injected into the predicate |
| `working_dir` | Working directory for the probe |
| `timeout_seconds` | Timeout for the probe execution |

### reproduce subcommand

Extract reproduction capsules from a completed run:

```bash
# Print a summary of boundary commit capsules
faultline reproduce --run-dir .faultline/runs/<fingerprint>

# Target a specific commit
faultline reproduce --run-dir .faultline/runs/<fingerprint> --commit abc1234

# Emit a shell script to stdout
faultline reproduce --run-dir .faultline/runs/<fingerprint> --shell
```

Example `--shell` output:

```bash
#!/bin/sh
set -e
cd '/path/to/repo'
git checkout abc1234
export DB_HOST='[REDACTED]'
export DB_PORT='[REDACTED]'
timeout 300 sh -c 'make test'
```

Environment variable values in shell script output are **redacted by default**. Pass `--unsafe-include-env` to include raw values:

```bash
faultline reproduce --run-dir .faultline/runs/<fingerprint> --shell --unsafe-include-env
```

## FlakeSignal

When `--retries` is greater than 0, each observation includes a `FlakeSignal` recording the results of repeated probes:

| Field | Type | Description |
|-------|------|-------------|
| `total_runs` | `u32` | Total number of probe executions (1 + retries) |
| `pass_count` | `u32` | Number of runs that exited 0 (pass) |
| `fail_count` | `u32` | Number of runs that exited non-zero (fail) |
| `skip_count` | `u32` | Number of runs that exited 125 (skip) |
| `indeterminate_count` | `u32` | Number of runs that timed out (indeterminate) |
| `is_stable` | `bool` | Whether the most-frequent class meets the stability threshold |

### Majority-vote logic

The observation's final classification is determined by the most-frequent result class across all runs. For example, with 3 runs producing [pass, fail, pass], the majority class is `pass` and the observation is classified as passing.

### Stability threshold

The `is_stable` field is computed as:

```
is_stable = (max_class_count / total_runs) >= stability_threshold
```

With `--stability-threshold 1.0` (the default), all runs must agree for the observation to be stable. With `--stability-threshold 0.67`, a 2/3 majority suffices.

### Effect on confidence scoring

Observations marked `is_stable: false` reduce confidence in the localization outcome. When unstable observations appear in the critical path between good and bad boundaries, faultline may return a `SuspectWindow` instead of an exact `FirstBad` result, reflecting the uncertainty introduced by flaky behavior.
