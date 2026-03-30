# Architecture

## High-level shape

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

## Bounded contexts

### Localization
Pure domain model over:
- ordered revision sequence
- observations
- search policy
- outcome semantics

### Surface
Coarse path-based summarization of the suspect change surface.
No AST, no semantic ownership model.

### Application
Use-case orchestration. Owns lifecycle and policy enforcement, but not localization semantics.

### Infrastructure
Git history, checkout creation, probe execution, persistence, and artifact rendering.

## Dependency direction

- adapters depend inward
- application depends on ports + domain
- domain depends only on pure shared types / codes
