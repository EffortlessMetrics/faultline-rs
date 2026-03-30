# Project Structure

## Architecture

Hexagonal architecture with two pure domains (localization, surface) and infrastructure adapters.

```
faultline-cli
    ↓
faultline-app (orchestration)
    ↓                    ↓
  Domain              Ports → Adapters
```

## Crate Organization

All crates live under `crates/`:

### Domain (Pure)

- `faultline-codes` - shared diagnostic/ambiguity vocabulary (enums)
- `faultline-types` - pure value objects, report model, error types
- `faultline-localization` - regression-window search engine
- `faultline-surface` - coarse path-based change bucketing

### Ports

- `faultline-ports` - outbound hexagonal port traits (HistoryPort, CheckoutPort, ProbePort, RunStorePort)

### Adapters (Infrastructure)

- `faultline-git` - Git CLI adapter (history + checkout via worktrees)
- `faultline-probe-exec` - process execution adapter
- `faultline-store` - filesystem-backed run persistence
- `faultline-render` - JSON + HTML artifact writers

### Application

- `faultline-app` - use-case orchestration, owns lifecycle/policy

### Entry Point

- `faultline-cli` - operator-facing CLI

### Testing

- `faultline-fixtures` - fixture builders for BDD-style scenarios

## Dependency Direction

- Adapters depend inward on ports
- Application depends on ports + domain
- Domain depends only on `faultline-codes` and `faultline-types`
- No domain crate imports infrastructure

## Runtime Directories

- `.faultline/scratch/` - disposable worktrees
- `.faultline/runs/` - persisted observation store
