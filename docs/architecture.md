# Architecture

## High-level shape

faultline follows a hexagonal (ports-and-adapters) architecture. Two pure domain crates own all business logic. Four port traits define the outbound boundary. Five infrastructure adapters implement those ports. One application crate orchestrates the use-case lifecycle. One CLI crate provides the operator entry point.

```text
faultline-cli
    |
    v
faultline-app  -----------------------------.
    |                                       |
    | uses                                  | calls through ports
    v                                       v
faultline-localization                HistoryPort
faultline-surface                     CheckoutPort
faultline-types                       ProbePort
faultline-codes                       RunStorePort

Adapters:
- faultline-git        (git CLI: history + checkout)
- faultline-probe-exec (process execution)
- faultline-store      (filesystem-backed run store)
- faultline-render     (JSON + HTML)
- faultline-fixtures   (BDD harness)
```

## Crate boundaries

### Domain (pure, no I/O)

| Crate | Responsibility |
|-------|---------------|
| `faultline-codes` | Shared diagnostic and ambiguity vocabulary — `ObservationClass`, `ProbeKind`, `AmbiguityReason`, `OperatorCode` enums. No business logic. |
| `faultline-types` | Pure value objects (`CommitId`, `RevisionSequence`, `ProbeObservation`, `AnalysisReport`, etc.), error types, and serialization contracts. The canonical data model for the entire system. |
| `faultline-localization` | Regression-window search engine. Drives binary narrowing over a `RevisionSequence` using recorded observations. Produces `FirstBad`, `SuspectWindow`, or `Inconclusive` outcomes. |
| `faultline-surface` | Coarse path-based change bucketing. Groups changed files into subsystem buckets by top-level directory and assigns surface kinds (source, tests, docs, etc.). |

### Ports (trait definitions)

| Crate | Responsibility |
|-------|---------------|
| `faultline-ports` | Four outbound port traits: `HistoryPort` (linearize history, compute diffs), `CheckoutPort` (create/destroy worktrees), `ProbePort` (run predicates), `RunStorePort` (persist observations and reports). |

### Adapters (infrastructure)

| Crate | Responsibility |
|-------|---------------|
| `faultline-git` | Implements `HistoryPort` and `CheckoutPort` by shelling out to the system `git` binary. Manages disposable linked worktrees under `.faultline/scratch/`. |
| `faultline-probe-exec` | Implements `ProbePort`. Spawns the operator's predicate command, captures stdout/stderr, enforces timeouts, classifies exit codes. |
| `faultline-store` | Implements `RunStorePort`. Filesystem-backed persistence under `.faultline/runs/{fingerprint}/` with `request.json`, `observations.json`, and `report.json`. |
| `faultline-render` | Writes `analysis.json` (canonical JSON artifact) and `index.html` (self-contained HTML report) from an `AnalysisReport`. |

### Application

| Crate | Responsibility |
|-------|---------------|
| `faultline-app` | Use-case orchestration. Wires port implementations, drives the localization loop (linearize → validate boundaries → binary narrow → produce report), and enforces policy (max probes, boundary validation). |

### Entry point

| Crate | Responsibility |
|-------|---------------|
| `faultline-cli` | Operator-facing CLI via clap. Parses arguments, constructs adapters, invokes `FaultlineApp::localize`, renders artifacts, prints summary. Exit 0 on success, exit 2 on error. |

### Testing

| Crate | Responsibility |
|-------|---------------|
| `faultline-fixtures` | Fixture builders for BDD-style test scenarios. `RevisionSequenceBuilder` constructs synthetic revision sequences without real Git repos. |

## Bounded contexts

### Localization
Pure domain model over:
- ordered revision sequence
- observations (pass, fail, skip, indeterminate)
- search policy (max probes, edge refine threshold)
- outcome semantics (FirstBad, SuspectWindow, Inconclusive)

### Surface
Coarse path-based summarization of the suspect change surface.
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
