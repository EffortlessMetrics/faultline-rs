# Crate Map

Every workspace crate with its tier, direct dependencies, applicable verification techniques, and one-sentence responsibility.

## Overview

```text
faultline-cli (entry: +reproduce, +diff-runs, +export-markdown, --retries, --stability-threshold, --markdown)
    ↓
faultline-app (app: +flake retry loop, +capsule emit, +suspect surface wiring)
    ↓                         ↓
  Domain                    Ports → Adapters
  ├─ faultline-codes          faultline-ports (+codeowners_for_paths, +blame_frequency)
  ├─ faultline-types            ├─ faultline-git (+CODEOWNERS parser, +blame frequency)
  │   (+FlakePolicy,           ├─ faultline-probe-exec
  │    +FlakeSignal,           ├─ faultline-store
  │    +ReproductionCapsule,   ├─ faultline-render (+markdown dossier, +suspect surface)
  │    +RunComparison,         ├─ faultline-sarif (+suspect surface locations)
  │    +SuspectEntry)          └─ faultline-junit (+suspect surface in system-out)
  ├─ faultline-localization
  │   (+FlakePolicy awareness, +confidence degradation)
  └─ faultline-surface
      (+rank_suspect_surface, +SuspectEntry scoring)

Testing: faultline-fixtures (+arb generators for new types)
Tooling: xtask (+smoke, +docs-check, +check-scenarios, +export-*)
```

## Crate Table

| Crate | Tier | Dependencies | Verification | Responsibility |
|-------|------|-------------|--------------|----------------|
| `faultline-codes` | domain | `serde`, `schemars` | property (via types), unit | Shared diagnostic vocabulary — `ObservationClass`, `ProbeKind`, `AmbiguityReason`, `OperatorCode` enums. |
| `faultline-types` | domain | `faultline-codes`, `serde`, `serde_json`, `schemars`, `thiserror` | property, unit, fuzz | Pure value objects (`CommitId`, `RevisionSequence`, `ProbeObservation`, `AnalysisReport`, `FlakePolicy`, `FlakeSignal`, `ReproductionCapsule`, `RunComparison`, `SuspectEntry`), error types, serialization contracts, `compare_runs` pure function, and `to_shell_script` capsule generation. |
| `faultline-localization` | domain | `faultline-codes`, `faultline-types` | property, unit, mutation | Regression-window search engine — binary narrowing over a `RevisionSequence` using recorded observations, producing `FirstBad`, `SuspectWindow`, or `Inconclusive` outcomes. Supports flake-aware probing via `FlakePolicy` with confidence degradation for unstable observations. |
| `faultline-surface` | domain | `faultline-types` | property, unit, mutation | Ranked suspect surface — groups changed files into subsystem buckets, assigns surface kinds, and ranks paths by investigation priority with execution-surface weighting, rename/delete scoring, and optional owner hints via `rank_suspect_surface`. |
| `faultline-ports` | ports | `faultline-types` | — | Four outbound hexagonal port traits: `HistoryPort` (including `codeowners_for_paths` and `blame_frequency`), `CheckoutPort`, `ProbePort`, `RunStorePort`. |
| `faultline-app` | app | `faultline-ports`, `faultline-localization`, `faultline-surface`, `faultline-types` | property, integration, mutation | Use-case orchestration — wires port implementations, drives the localization loop with flake retry support, generates reproduction capsules, populates ranked suspect surface, and enforces policy (max probes, boundary validation). |
| `faultline-git` | adapter | `faultline-ports`, `faultline-types` | property, BDD/unit, fuzz, mutation | Git CLI adapter — implements `HistoryPort` (including CODEOWNERS parsing and blame-frequency owner hints) and `CheckoutPort` by shelling out to the system `git` binary with disposable linked worktrees. |
| `faultline-probe-exec` | adapter | `faultline-ports`, `faultline-types`, `faultline-codes` | property, BDD/unit, mutation | Process execution adapter — implements `ProbePort`, spawns the operator's predicate command, captures output, enforces timeouts, classifies exit codes. |
| `faultline-store` | adapter | `faultline-ports`, `faultline-types`, `faultline-codes` | property, BDD/unit, fuzz, mutation | Filesystem-backed run persistence — implements `RunStorePort` with atomic writes, lock files, and per-commit probe log storage. |
| `faultline-render` | adapter | `faultline-types`, `faultline-codes` | property, unit, golden, fuzz, mutation | JSON + HTML + Markdown artifact writers — produces `analysis.json`, self-contained `index.html`, and Markdown dossier from an `AnalysisReport`. Renders ranked suspect surface with owner hints in all output formats. |
| `faultline-cli` | entry | `faultline-app`, `faultline-git`, `faultline-probe-exec`, `faultline-store`, `faultline-render`, `faultline-types`, `faultline-codes`, `clap` | property, unit, golden, fuzz, smoke | Operator-facing CLI — parses arguments (including `--retries`, `--stability-threshold`, `--markdown`), constructs adapters, invokes `FaultlineApp::localize`, renders artifacts, prints summary. Subcommands: `reproduce`, `diff-runs`, `export-markdown`. |
| `faultline-fixtures` | testing | `faultline-types` | — | Fixture builders for BDD-style test scenarios — `RevisionSequenceBuilder` and `GitRepoBuilder` for constructing synthetic test data. Arbitrary generators for all new types (`SuspectEntry`, `FlakeSignal`, `FlakePolicy`, `ReproductionCapsule`, `RunComparison`). |
| `faultline-sarif` | adapter | `faultline-types`, `serde`, `serde_json` | property, unit, golden, fuzz, mutation | SARIF v2.1.0 export adapter — converts an `AnalysisReport` (including suspect surface locations) into a SARIF document for GitHub Code Scanning and other SARIF-compatible tools. |
| `faultline-junit` | adapter | `faultline-types`, `quick-xml` | property, unit, golden, fuzz, mutation | JUnit XML export adapter — converts an `AnalysisReport` (including suspect surface in system-out) into JUnit XML for CI dashboards. |
| `xtask` | tooling | `faultline-types`, `schemars`, `serde_json`, `clap`, `anyhow` | unit | Repo operations binary — provides `cargo xtask` subcommands for CI, golden tests, schema generation, mutation testing, fuzzing, docs checking, release checks, scaffolding, smoke testing, scenario checking, and export commands (`export-markdown`, `export-sarif`, `export-junit`). |

## Dependency Direction

- Adapters depend inward on ports and types
- Application depends on ports + domain
- Domain depends only on `faultline-codes` and `faultline-types`
- No domain crate imports infrastructure
- Export adapters (`faultline-sarif`, `faultline-junit`) depend only on `faultline-types`
- `xtask` depends on `faultline-types` for schema generation

See also: [Architecture](architecture.md), [Verification Matrix](verification-matrix.md)
