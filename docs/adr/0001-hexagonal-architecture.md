# ADR-0001: Hexagonal architecture and bounded contexts

## Status
Accepted.

## Context

faultline needs to run operator-supplied predicates against Git history, persist results, and produce artifacts. The core localization logic (binary narrowing, outcome classification) is pure and deterministic, but the surrounding infrastructure (Git CLI, process spawning, filesystem I/O) is inherently side-effectful.

We need an architecture that keeps the search engine easy to test with synthetic data while allowing infrastructure to evolve independently.

## Decision

`faultline` is organized as a hexagonal (ports-and-adapters) application with two small pure domains:

1. **Localization** — regression-window search engine over ordered revision sequences
2. **Surface summarization** — coarse path-based change bucketing

Everything else is orchestration (`faultline-app`) or infrastructure (adapters behind port traits).

Four port traits (`HistoryPort`, `CheckoutPort`, `ProbePort`, `RunStorePort`) define the outbound boundary. Infrastructure adapters implement these traits and are injected at the CLI entry point.

## Consequences

- Git, processes, storage, and rendering stay outside the domain core.
- The search engine is easy to test with synthetic revision sequences and mock ports.
- Infrastructure can change (e.g., switching from git CLI to libgit2) without rewriting the localization model.
- All property-based tests run against pure domain logic with no I/O.
