# faultline

**faultline** is a local-first regression localization tool for Git repositories.

Point it at a known-good commit, a known-bad commit, and the test (or build) command you already trust.
Faultline binary-searches the history, runs your predicate at each candidate, and produces a portable report showing the narrowest credible regression window.

## Quickstart

```bash
# Install from source (requires Rust stable, edition 2024)
cargo install --path crates/faultline-cli

# Find the regression: which commit between v1.0 and HEAD broke `cargo test`?
faultline-cli --repo . --good v1.0 --bad HEAD --cmd "cargo test" --kind test

# View the report
open faultline-report/index.html
```

Or run directly from the workspace without installing:

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

Direct exec mode (no shell wrapper):

```bash
faultline-cli \
  --repo . \
  --good abc1234 \
  --bad def5678 \
  --kind build \
  --program cargo \
  --arg build \
  --arg --workspace
```

Replace `abc1234` / `def5678` with real commit SHAs or tags from your repository.

## Example output

A successful run prints a summary like this:

```
run-id       a1b2c3d4
observations 7
output-dir   ./faultline-report
artifacts    ./faultline-report/analysis.json
             ./faultline-report/index.html
history      ancestry-path
outcome      FirstBad  last_good=abc1234 first_bad=def5678 confidence=100(high)
```

When the history is ambiguous, faultline returns a suspect window instead of a single commit:

```
outcome      SuspectWindow  lower=abc1234 upper=ghi7890 confidence=62(medium) reasons=[non-linear merge topology]
```

## What you get

Every run produces up to three artifacts in the output directory:

| Artifact | File | Purpose |
|----------|------|---------|
| **JSON** | `analysis.json` | Machine-readable report for CI pipelines and downstream tools |
| **HTML** | `index.html` | Human-readable dossier with timeline, diff links, and suspect ranking |
| **Markdown** | `dossier.md` | PR-friendly summary (opt-in via `--markdown`) |

## Key features

- **Honest outcomes** -- exact first-bad commit, ranked suspect window, or explicit inconclusive; never fake precision
- **Ranked suspect surface** -- coarse path-based bucketing of changed files across the suspect window
- **Flake-aware probing** -- configurable retries (`--retries`) and stability threshold to handle noisy predicates
- **Reproduction capsules** -- `faultline-cli reproduce` extracts a self-contained capsule to re-run the failing predicate at the boundary commit
- **Run-to-run comparison** -- `faultline-cli diff-runs` compares two analysis runs side-by-side to see if a fix actually moved the regression window
- **Resumable runs** -- observations are persisted to disk; interrupted runs pick up where they left off

## Subcommands

| Command | Description |
|---------|-------------|
| *(default)* | Localize the regression between `--good` and `--bad` |
| `reproduce` | Extract a reproduction capsule from a completed run |
| `diff-runs` | Compare two analysis runs side-by-side |
| `export-markdown` | Export a Markdown dossier from a completed run |

## CLI flags

| Flag | Default | Notes |
|------|---------|-------|
| `--repo` | `.` | Path to the Git repository |
| `--good` | *required* | Known-good boundary (SHA, tag, or branch) |
| `--bad` | *required* | Known-bad boundary (SHA, tag, or branch) |
| `--cmd` | -- | Shell predicate to run (mutually exclusive with `--program`) |
| `--program` / `--arg` | -- | Direct exec predicate (no shell wrapper) |
| `--kind` | `custom` | Predicate kind: `build`, `test`, `lint`, `perf-threshold`, `custom` |
| `--timeout-seconds` | `300` | Per-probe timeout; exceeded probes are classified as Indeterminate |
| `--output-dir` | `faultline-report` | Where to write artifacts |
| `--max-probes` | `64` | Cap probe executions before returning a suspect window |
| `--retries` | `0` | Per-commit retry count for flake detection |
| `--stability-threshold` | `1.0` | Minimum proportion of consistent results to classify as stable |
| `--first-parent` | `false` | Use first-parent linearization instead of ancestry-path |
| `--shell` | auto | Shell for `--cmd` predicates: `sh`, `cmd`, `powershell` |
| `--env KEY=VALUE` | -- | Inject environment variables into the predicate (repeatable) |
| `--resume` | default | Continue an interrupted run |
| `--force` | -- | Discard cached observations and re-probe |
| `--fresh` | -- | Delete the entire run directory and start from scratch |
| `--no-render` | -- | Skip HTML generation, produce only `analysis.json` |
| `--markdown` | -- | Also write a Markdown dossier alongside HTML/JSON |

Run `faultline-cli --help` for the full flag reference.

## Operator contract

The predicate should be monotonic enough across the selected history range.
If it flakes, times out, or depends on mutable external state, faultline will reduce confidence and may return a suspect window instead of an exact first-bad commit.

## Workspace layout

| Crate | Role |
|-------|------|
| `faultline-cli` | Operator-facing CLI |
| `faultline-app` | Use-case orchestration |
| `faultline-localization` | Regression-window engine |
| `faultline-surface` | Coarse changed-surface bucketing |
| `faultline-types` | Pure shared value objects and report model |
| `faultline-codes` | Shared diagnostic / ambiguity vocabulary |
| `faultline-ports` | Outbound hexagonal ports |
| `faultline-git` | Git history and checkout adapters |
| `faultline-probe-exec` | Process execution adapter |
| `faultline-store` | Filesystem-backed run store |
| `faultline-render` | JSON + HTML artifact writers |
| `faultline-sarif` | SARIF export |
| `faultline-junit` | JUnit XML export |
| `faultline-fixtures` | Fixture builders for BDD-style scenarios |

For architecture details, design principles, and contributor guidelines, see [AGENTS.md](AGENTS.md).

## Current scope

This is a v0.1 release, deliberately narrow.

### Included

- Known-good / known-bad explicit boundaries
- Ancestry-path and first-parent history linearization
- Disposable Git worktrees per probe
- Structured probe execution with timeout handling
- Persistent observation store for reruns
- Exact boundary or suspect-window localization
- Coarse path-based subsystem bucketing
- JSON + HTML + Markdown report generation
- Reproduction capsules and run comparison

### Deliberately excluded

- GitHub / CI provider APIs
- Organization-wide incident management
- AI-written root-cause analysis
- AST-aware topology inference
- Automatic fixes or patch generation

## Building from source

See [BUILDING.md](BUILDING.md) for prerequisites and commands.
