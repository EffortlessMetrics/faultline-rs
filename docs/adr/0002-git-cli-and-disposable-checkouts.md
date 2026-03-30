# ADR-0002: Git CLI adapter and disposable checkouts

## Status
Accepted.

## Decision

The v0.1 Git adapter shells out to the system `git` binary and uses disposable linked worktrees per probe.

## Rationale

- defers Git semantics to the tool users already trust
- avoids mutating the operator's main checkout
- keeps the adapter small and replaceable
