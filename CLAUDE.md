# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What This Is

faultline is a local-first regression archaeologist for Git repositories. Given good/bad commits and a predicate command, it narrows the regression window via binary search over Git history and emits JSON + HTML artifacts showing where to investigate. Design principles: honest over impressive, predicate-native, local-first, artifact-first.

## Build & Test Commands

```bash
# Build
cargo build --workspace

# Test (full workspace)
cargo test --workspace

# Test a single crate
cargo test -p faultline-localization

# Format
cargo fmt --all --check    # verify
cargo fmt --all            # fix

# Lint
cargo clippy --workspace -- -D warnings

# CI tiers (use these before submitting work)
cargo xtask ci-fast        # fmt + clippy + test (~5 min)
cargo xtask ci-full        # ci-fast + golden + schema-check (~10 min)

# Golden snapshot tests
cargo insta review          # interactive review of changed snapshots
cargo xtask golden          # run + update golden tests

# Schema regeneration (after changing AnalysisReport or transitive types)
cargo xtask generate-schema

# Mutation testing
cargo xtask mutants
cargo xtask mutants --crate faultline-localization  # single crate

# Smoke test
cargo xtask smoke

# Run the CLI
cargo run -p faultline-cli -- --help

# CLI subcommands
cargo run -p faultline-cli -- reproduce --run-dir <path>
cargo run -p faultline-cli -- diff-runs --left a.json --right b.json [--json]
cargo run -p faultline-cli -- export-markdown --run-dir <path>

# Xtask export commands (SARIF and JUnit exports are xtask subcommands)
cargo xtask export-sarif --run-dir <path> [--output file.sarif]
cargo xtask export-junit --run-dir <path> [--output file.xml]
cargo xtask export-markdown --run-dir <path> [--output file.md]
```

`just` aliases exist for most xtask commands (e.g., `just ci`, `just ci-full`, `just mutants`).

## Architecture

Hexagonal (ports-and-adapters) with strict dependency direction:

```
faultline-cli (entry point, clap)
    |
faultline-app (orchestration)
    |                           |
  Domain (pure, no I/O)      Ports (traits) -> Adapters (I/O)
  - faultline-codes             faultline-ports
  - faultline-types               -> faultline-git
  - faultline-localization        -> faultline-probe-exec
  - faultline-surface             -> faultline-store
                                  -> faultline-render
                                  -> faultline-sarif
                                  -> faultline-junit
```

**Key invariant:** Domain crates have zero I/O. All infrastructure is behind port traits in adapter crates. Adapters depend inward on ports+types; app depends on ports+domain; domain depends only on codes+types.

### Crate Responsibilities

- **faultline-codes**: Diagnostic enums (ObservationClass, ProbeKind, AmbiguityReason, OperatorCode)
- **faultline-types**: Value objects, AnalysisReport, FaultlineError, RunComparison, FlakeSignal, ReproductionCapsule, serialization contracts
- **faultline-localization**: Binary narrowing engine, flake-aware confidence degradation, FirstBad/SuspectWindow/Inconclusive outcomes
- **faultline-surface**: Ranked suspect surface with scoring (execution surface, deletes, renames), bucketing, CODEOWNERS owner hints
- **faultline-ports**: Trait definitions only (HistoryPort, CheckoutPort, ProbePort, RunStorePort)
- **faultline-app**: Orchestration: linearize -> validate boundaries -> binary narrow -> rank suspect surface -> generate capsules -> build report
- **faultline-cli**: clap-based CLI entry point with subcommands (reproduce, diff-runs, export-markdown)
- **faultline-git**: HistoryPort + CheckoutPort impl, shells out to system `git`, worktree management, CODEOWNERS parsing, blame frequency
- **faultline-probe-exec**: ProbePort impl, process execution with timeout, flake retry loop
- **faultline-store**: RunStorePort impl, filesystem under `.faultline/runs/`, atomic writes with lock files
- **faultline-render**: JSON + HTML + Markdown dossier artifact generation, suspect surface rendering
- **faultline-sarif**: SARIF v2.1.0 export adapter (`to_sarif(&report)`)
- **faultline-junit**: JUnit XML export adapter (`to_junit_xml(&report)`)
- **faultline-fixtures**: Test builders, proptest generators (`arb.rs`), fixture repo construction
- **xtask**: Dev tooling (CI tiers, scaffolding, schema gen, smoke tests, export commands, scenario checks, docs-check with link validation)

## CLI Subcommands

The `faultline-cli` binary provides these subcommands:

| Subcommand | Purpose |
|------------|---------|
| *(default)* | Run localization: `--good <sha> --bad <sha> --cmd <predicate>` |
| `reproduce` | Extract a reproduction capsule from a completed run (`--run-dir`, `--commit`, `--shell`) |
| `diff-runs` | Compare two analysis runs side-by-side (`--left`, `--right`, `--json`) |
| `export-markdown` | Export a Markdown dossier from a completed run (`--run-dir`) |

Notable flags on the default (localize) command: `--retries` and `--stability-threshold` for flake-aware probing, `--markdown` to emit a Markdown dossier alongside HTML/JSON, `--shell` to select predicate shell (sh/cmd/powershell), `--env` for environment variable injection, `--resume`/`--force`/`--fresh` for run mode control.

SARIF and JUnit exports are available via `cargo xtask export-sarif` and `cargo xtask export-junit` respectively, not as CLI subcommands.

## Testing Rules

- **Property tests**: Minimum 100 cases per property via `proptest!`. Shared generators live in `faultline-fixtures/src/arb.rs`.
- **Golden tests**: `insta` snapshots guard `analysis.json`, `index.html`, and CLI `--help`. Accept changes with `cargo insta review`.
- **Scenario atlas**: Every test must have an entry in `docs/scenarios/scenario_index.md`. CI enforces this via `cargo xtask check-scenarios`.
- **Human Review Gate**: Never suppress, skip, or weaken a failing property test without maintainer approval.
- **Fuzz targets**: Six fuzz targets in `fuzz/` covering git diff parsing, store JSON, HTML escaping, CLI args, SARIF export, and JUnit export.

## Change Workflows

- **Schema-breaking change**: Bump `schema_version` in `AnalysisReport`, run `cargo xtask generate-schema`, update golden snapshots, update export adapters if affected.
- **Adding a test**: Write the test, add scenario atlas entry, run `cargo xtask ci-fast`.
- **New crate**: Use `cargo xtask scaffold crate faultline-<name> --tier <tier>`, then add to crate-map and verification-matrix docs.
- **Golden test failure**: Run `cargo insta review`, accept if intentional, commit `.snap` files with code change.

## Error Handling

All crates use `FaultlineError` (in faultline-types) with variants: InvalidInput, InvalidBoundary, Git, Probe, Store, Render, Domain, Io, Serde. Adapters map infrastructure errors to the appropriate variant. CLI exits with code 0 (success), 1 (suspect window), 2 (execution error), 3 (inconclusive), or 4 (invalid input).

## Key Documentation

- `AGENTS.md` -- Full contributor/agent onboarding, architecture, escalation rules
- `TESTING.md` -- Verification matrix, CI tiers, how-to guides for each test type
- `MAINTAINERS.md` -- Code ownership, review gates, supply-chain policy
- `RELEASE.md` -- Version bumps, changelog, release checks
- `BUILDING.md` -- Prerequisites and build instructions
- `docs/architecture.md` -- Detailed architecture and crate boundaries
- `docs/crate-map.md` -- Every crate with tier, deps, responsibility
- `docs/scenarios/` -- Scenario atlas and behavior map
- `docs/handbook/` -- Maintainer playbooks and worked examples (schema bumps, test technique decisions, flake detection, suspect surface)
- `docs/adr/` -- Architecture Decision Records
- `docs/patterns/catalog.md` -- Named patterns governing the repo (Human Review Gate, etc.)
