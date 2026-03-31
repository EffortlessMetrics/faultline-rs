# Crate Map

Every workspace crate with its tier, direct dependencies, applicable verification techniques, and one-sentence responsibility.

## Overview

```text
faultline-cli (entry)
    ↓
faultline-app (app)
    ↓                         ↓
  Domain                    Ports → Adapters
  ├─ faultline-codes          faultline-ports
  ├─ faultline-types            ├─ faultline-git
  ├─ faultline-localization     ├─ faultline-probe-exec
  └─ faultline-surface          ├─ faultline-store
                                ├─ faultline-render
                                ├─ faultline-sarif
                                └─ faultline-junit

Testing: faultline-fixtures
Tooling: xtask
```

## Crate Table

| Crate | Tier | Dependencies | Verification | Responsibility |
|-------|------|-------------|--------------|----------------|
| `faultline-codes` | domain | `serde`, `schemars` | property (via types), unit | Shared diagnostic vocabulary — `ObservationClass`, `ProbeKind`, `AmbiguityReason`, `OperatorCode` enums. |
| `faultline-types` | domain | `faultline-codes`, `serde`, `serde_json`, `schemars`, `thiserror` | property, unit | Pure value objects (`CommitId`, `RevisionSequence`, `ProbeObservation`, `AnalysisReport`, etc.), error types, and serialization contracts. |
| `faultline-localization` | domain | `faultline-codes`, `faultline-types` | property, unit, mutation | Regression-window search engine — binary narrowing over a `RevisionSequence` using recorded observations, producing `FirstBad`, `SuspectWindow`, or `Inconclusive` outcomes. |
| `faultline-surface` | domain | `faultline-types` | property, unit | Coarse path-based change bucketing — groups changed files into subsystem buckets by top-level directory and assigns surface kinds. |
| `faultline-ports` | ports | `faultline-types` | — | Four outbound hexagonal port traits: `HistoryPort`, `CheckoutPort`, `ProbePort`, `RunStorePort`. |
| `faultline-app` | app | `faultline-ports`, `faultline-localization`, `faultline-surface`, `faultline-types` | property, integration, mutation | Use-case orchestration — wires port implementations, drives the localization loop, and enforces policy (max probes, boundary validation). |
| `faultline-git` | adapter | `faultline-ports`, `faultline-types` | BDD/unit | Git CLI adapter — implements `HistoryPort` and `CheckoutPort` by shelling out to the system `git` binary with disposable linked worktrees. |
| `faultline-probe-exec` | adapter | `faultline-ports`, `faultline-types`, `faultline-codes` | property, BDD/unit | Process execution adapter — implements `ProbePort`, spawns the operator's predicate command, captures output, enforces timeouts, classifies exit codes. |
| `faultline-store` | adapter | `faultline-ports`, `faultline-types`, `faultline-codes` | property, BDD/unit | Filesystem-backed run persistence — implements `RunStorePort` with atomic writes, lock files, and per-commit probe log storage. |
| `faultline-render` | adapter | `faultline-types`, `faultline-codes` | property, unit, golden | JSON + HTML artifact writers — produces `analysis.json` and self-contained `index.html` from an `AnalysisReport`. |
| `faultline-cli` | entry | `faultline-app`, `faultline-git`, `faultline-probe-exec`, `faultline-store`, `faultline-render`, `faultline-types`, `faultline-codes`, `clap` | property, unit, golden, smoke | Operator-facing CLI — parses arguments, constructs adapters, invokes `FaultlineApp::localize`, renders artifacts, prints summary. |
| `faultline-fixtures` | testing | `faultline-types` | — | Fixture builders for BDD-style test scenarios — `RevisionSequenceBuilder` and `GitRepoBuilder` for constructing synthetic test data. |
| `faultline-sarif` | adapter | `faultline-types`, `serde`, `serde_json` | property, unit, golden | SARIF v2.1.0 export adapter — converts an `AnalysisReport` into a SARIF document for GitHub Code Scanning and other SARIF-compatible tools. |
| `faultline-junit` | adapter | `faultline-types`, `quick-xml` | property, unit, golden | JUnit XML export adapter — converts an `AnalysisReport` into JUnit XML for CI dashboards. |
| `xtask` | tooling | `faultline-types`, `schemars`, `serde_json`, `clap`, `anyhow` | unit | Repo operations binary — provides `cargo xtask` subcommands for CI, golden tests, schema generation, mutation testing, fuzzing, docs, release checks, and scaffolding. |

## Dependency Direction

- Adapters depend inward on ports and types
- Application depends on ports + domain
- Domain depends only on `faultline-codes` and `faultline-types`
- No domain crate imports infrastructure
- Export adapters (`faultline-sarif`, `faultline-junit`) depend only on `faultline-types`
- `xtask` depends on `faultline-types` for schema generation

See also: [Architecture](architecture.md), [Verification Matrix](verification-matrix.md)
