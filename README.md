# faultline

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

Run `cargo run -p faultline-cli -- --help` for the full flag reference.

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

## Packaging

faultline v0.1 is a **source-only release**. Install by cloning the repository and building with `cargo build --release`. See [BUILDING.md](BUILDING.md) for prerequisites and commands. Prebuilt binaries are not provided for v0.1.
