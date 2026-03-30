# ADR-0001: Hexagonal architecture and bounded contexts

## Status
Accepted.

## Decision

`faultline` is organized as a hexagonal application with two small pure domains:

1. localization
2. surface summarization

Everything else is orchestration or infrastructure.

## Consequences

- Git, processes, storage, and rendering stay outside the domain core.
- The search engine is easy to test with synthetic revision sequences.
- Infrastructure can change without rewriting the localization model.
