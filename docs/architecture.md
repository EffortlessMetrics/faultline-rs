# Architecture

## High-level shape

faultline follows a hexagonal (ports-and-adapters) architecture. Two pure domain crates own all business logic. Four port traits define the outbound boundary. Six infrastructure adapters implement those ports or provide export functionality. One shared infrastructure helper crate (`faultline-loader`) provides report loading for entry-point crates. One application crate orchestrates the use-case lifecycle. One CLI crate provides the operator entry point.

```text
faultline-cli (+reproduce, +diff-runs, +export-markdown)
    |
    v
faultline-app  -----------------------------.
    |                                       |
    | uses                                  | calls through ports
    v                                       v
faultline-localization                HistoryPort (+codeowners, +blame)
faultline-surface                     CheckoutPort
faultline-types                       ProbePort
faultline-codes                       RunStorePort

Adapters (6):
- faultline-git        (git CLI: history + checkout + CODEOWNERS + blame)
- faultline-probe-exec (process execution)
- faultline-store      (filesystem-backed run store)
- faultline-render     (JSON + HTML + Markdown)
- faultline-sarif      (SARIF v2.1.0 export)
- faultline-junit      (JUnit XML export)

Shared infrastructure helper:
- faultline-loader     (report file resolution + loading for CLI and xtask)

Testing utility:
- faultline-fixtures   (BDD harness + arbitrary generators — not an adapter)
```

## Crate boundaries

### Domain (pure, no I/O)

| Crate | Responsibility |
|-------|---------------|
| `faultline-codes` | Shared diagnostic and ambiguity vocabulary — `ObservationClass`, `ProbeKind`, `AmbiguityReason`, `OperatorCode` enums. No business logic. |
| `faultline-types` | Pure value objects (`CommitId`, `RevisionSequence`, `ProbeObservation`, `AnalysisReport`, `FlakePolicy`, `FlakeSignal`, `ReproductionCapsule`, `RunComparison`, `SuspectEntry`), error types, serialization contracts, `compare_runs` pure function, and `to_shell_script` capsule generation. The canonical data model for the entire system. |
| `faultline-localization` | Regression-window search engine. Drives binary narrowing over a `RevisionSequence` using recorded observations. Produces `FirstBad`, `SuspectWindow`, or `Inconclusive` outcomes. Supports flake-aware probing via `FlakePolicy` with confidence degradation for unstable observations. |
| `faultline-surface` | Ranked suspect surface analysis. Groups changed files into subsystem buckets, assigns surface kinds, and ranks paths by investigation priority with execution-surface weighting, rename/delete scoring, and optional owner hints. |

### Ports (trait definitions)

| Crate | Responsibility |
|-------|---------------|
| `faultline-ports` | Four outbound port traits: `HistoryPort` (linearize history, compute diffs, parse CODEOWNERS, derive blame-frequency owner hints), `CheckoutPort` (create/destroy worktrees), `ProbePort` (run predicates), `RunStorePort` (persist observations and reports). |

### Adapters (infrastructure — six total)

| Crate | Responsibility |
|-------|---------------|
| `faultline-git` | Implements `HistoryPort` (including CODEOWNERS parsing and blame-frequency owner hints) and `CheckoutPort` by shelling out to the system `git` binary. Manages disposable linked worktrees under `.faultline/scratch/`. |
| `faultline-probe-exec` | Implements `ProbePort`. Spawns the operator's predicate command, captures stdout/stderr, enforces timeouts, classifies exit codes. |
| `faultline-store` | Implements `RunStorePort`. Filesystem-backed persistence under `.faultline/runs/{fingerprint}/` with `request.json`, `observations.json`, and `report.json`. |
| `faultline-render` | Writes `analysis.json` (canonical JSON artifact), `index.html` (self-contained HTML report with ranked suspect surface), and Markdown dossier from an `AnalysisReport`. |
| `faultline-sarif` | SARIF v2.1.0 export adapter. Converts an `AnalysisReport` into SARIF JSON for integration with code scanning tools. |
| `faultline-junit` | JUnit XML export adapter. Converts an `AnalysisReport` into JUnit XML for CI system integration. |

### Shared infrastructure helper

| Crate | Responsibility |
|-------|---------------|
| `faultline-loader` | Report file resolution and loading. Implements the shared `Report_Locator` logic used by both `faultline-cli` and `xtask` to locate and deserialize an `AnalysisReport` from either a directory path or a direct file path. Not a port adapter — a convenience helper at the same dependency tier as adapters, depended on only by entry-point crates (CLI, xtask). |

### Application

| Crate | Responsibility |
|-------|---------------|
| `faultline-app` | Use-case orchestration. Wires port implementations, drives the localization loop (linearize → validate boundaries → binary narrow → produce report) with flake retry support, generates reproduction capsules, populates ranked suspect surface, and enforces policy (max probes, boundary validation). |

### Entry point

| Crate | Responsibility |
|-------|---------------|
| `faultline-cli` | Operator-facing CLI via clap. Parses arguments (including `--retries`, `--stability-threshold`, `--markdown`), constructs adapters, invokes `FaultlineApp::localize`, renders artifacts, prints summary. Subcommands: `reproduce`, `diff-runs`, `export-markdown`. Exit 0 on success, exit 2 on error. |

### Testing utility

| Crate | Responsibility |
|-------|---------------|
| `faultline-fixtures` | Testing utility crate (not an infrastructure adapter). Provides fixture builders for BDD-style test scenarios, `RevisionSequenceBuilder` for constructing synthetic revision sequences without real Git repos, arbitrary generators (`proptest` strategies) for all domain types, and a secret-pattern fixture corpus for redaction testing. Used as a `dev-dependency` by other crates. |

## Two-tier artifact model

faultline uses a two-tier artifact model that separates internal persistence from shareable output:

| Tier | File | Location | Purpose |
|------|------|----------|---------|
| **Internal** | `report.json` (Report_JSON) | `.faultline/runs/{fingerprint}/` | Full-fidelity `AnalysisReport` serialization for local persistence and reproducibility. Written by `faultline-store` via `RunStorePort`. Always unredacted. |
| **Shareable** | `analysis.json` (Analysis_JSON) | User-specified output directory | `AnalysisReport` serialization intended for distribution outside the local machine. Written by `faultline-render`. Redacted by default. |

Additional shareable artifacts produced at render/export time: `index.html`, `dossier.md`, SARIF output, JUnit XML output.

### Architectural invariant: redaction boundary

> **Run_Store contents are always unredacted. Shareable_Artifacts are redacted projections produced at render/export time.**

The `AnalysisReport` in memory and in the Run_Store is always the full-truth, unredacted representation. Redaction is never a mutation of stored data — it is a projection applied at serialization boundaries (renderer, export adapters, shell script generation). This ensures:

- Local reproducibility is never degraded by redaction
- The same stored report can be re-rendered under different policies
- Redaction is deterministic and idempotent

## Report loading strategy

All CLI subcommands and xtask export commands that need to load a previously-generated report delegate to the shared `Report_Locator` implemented in `faultline-loader`. This eliminates duplicated loading logic and ensures consistent behavior across all entry points.

### Historical context

Before unification, CLI commands loaded `report.json` (internal tier) while xtask export commands loaded `analysis.json` (shareable tier). The shared Report_Locator resolves this divergence with deterministic precedence: when given a directory, it prefers `report.json` (full-fidelity) and falls back to `analysis.json`. When given a direct file path, it loads that file regardless of name.

### Report loading table

| Command | Report Resolution |
|---------|-------------------|
| `faultline` (localize) | N/A (generates report) |
| `faultline reproduce` | Shared Report_Locator |
| `faultline export-markdown` | Shared Report_Locator |
| `faultline diff-runs` | Shared Report_Locator (direct file) |
| `faultline inspect-run` | Direct directory walk |
| `faultline bundle` | Shared Report_Locator |
| `cargo xtask export-markdown` | Shared Report_Locator |
| `cargo xtask export-sarif` | Shared Report_Locator |
| `cargo xtask export-junit` | Shared Report_Locator |

All commands using the shared Report_Locator emit diagnostic messages to stderr (never stdout) to avoid corrupting piped output streams.

## Bounded contexts

### Localization
Pure domain model over:
- ordered revision sequence
- observations (pass, fail, skip, indeterminate)
- search policy (max probes, edge refine threshold)
- outcome semantics (FirstBad, SuspectWindow, Inconclusive)

### Surface
Ranked suspect surface analysis of the change surface between regression boundary commits.
Scoring by investigation priority with execution-surface weighting, rename/delete scoring, and owner hints.
No AST, no semantic ownership model.

### Application
Use-case orchestration. Owns lifecycle and policy enforcement, but not localization semantics.

### Infrastructure
Git history, checkout creation, probe execution, persistence, and artifact rendering.

## Dependency direction

- Adapters depend inward on ports and types
- Application depends on ports + domain
- Domain depends only on pure shared types / codes
- No domain crate imports infrastructure
