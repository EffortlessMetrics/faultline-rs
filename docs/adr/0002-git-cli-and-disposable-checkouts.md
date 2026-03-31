# ADR-0002: Git CLI adapter and disposable checkouts

## Status
Accepted.

## Context

faultline needs to linearize commit history, check out specific revisions for probing, compute diffs, and clean up after each probe. We considered two approaches: linking against libgit2 (via the `git2` crate) or shelling out to the system `git` binary.

Each probe must run in isolation so the operator's working copy is never mutated.

## Decision

The v0.1 Git adapter shells out to the system `git` binary and uses disposable linked worktrees per probe.

- History linearization via `git rev-list --reverse --ancestry-path [--first-parent]`
- Revision resolution via `git rev-parse --verify`
- Ancestry validation via `git merge-base --is-ancestor`
- Checkout via `git worktree add --detach --force` under `.faultline/scratch/`
- Cleanup via `git worktree remove --force` with fallback to directory deletion
- Diff computation via `git diff --name-status`

Worktree directories are named `{sha12}-{timestamp_ms}-{counter}` to prevent collisions.

## Rationale

- Defers Git semantics to the tool operators already trust
- Avoids mutating the operator's main checkout
- Keeps the adapter small and replaceable behind `HistoryPort` and `CheckoutPort`
- Linked worktrees are lightweight and share the object store with the main repo

## Consequences

- Requires `git` to be installed and on PATH
- Subprocess overhead per Git operation (acceptable for v0.1 probe counts)
- Adapter can be swapped for libgit2 in a future version without touching domain logic
