# Clippy / lint policy

This repository follows the Effortless Metrics Rust policy stack. The
short form:

> **Deny by default. Allow by receipt. Expire exceptions. Measure drift.**

## How the rails compose

```
Clippy lints
  catch local bad Rust shapes; live in workspace [lints]; staged via policy/

semantic no-panic checker
  owns location + reason allowlists for panic-family debt
  policy/no-panic-allowlist.toml + cargo xtask check-no-panic-family

non-Rust file policy checker
  owns location + reason allowlists for non-Rust surfaces
  policy/non-rust-allowlist.toml + cargo xtask check-file-policy

lint-policy checker
  verifies every crate inherits the shared rules
  policy/clippy-lints.toml + cargo xtask check-lint-policy
```

The semantic no-panic checker is the **authoritative** exception mechanism
for panic-family debt. Clippy is the immediate, source-shape detector.
Source-level suppressions, when needed, must be `#[expect(..., reason = "policy:...")]`
and never bare `#[allow(...)]`.

## Policy file: `policy/clippy-lints.toml`

The file is the single source of truth for:

- workspace MSRV (currently `1.93`),
- which Clippy/rustc lints are *active* and at what level,
- which lints are *planned* but waiting for a future MSRV,
- repo-level policy flags (no test carveouts, no blanket category profiles,
  panic-free tests, suppression style).

`cargo xtask check-lint-policy` verifies:

1. workspace MSRV matches `policy/clippy-lints.toml` `msrv =`;
2. every crate has `[lints] workspace = true`;
3. every active lint is wired into the workspace `[lints]` table;
4. there is no `clippy.toml` with test carveouts (`allow-unwrap-in-tests`, etc.);
5. no bare `#[allow(...)]` attributes appear in tracked source files.

## Staging

We do not flip everything to `deny` on day one. Panic-family lints land at
`warn` while debt is being baselined and burned down. The blocking gate
during this period is `cargo xtask check-no-panic-family`, which compares
the live findings against `policy/no-panic-allowlist.toml`.

Promotion path for any panic-family lint:

```
warn  ->  semantic checker burns the debt  ->  deny
```

For numeric lints (`cast_possible_truncation`, etc.), the staging is the
same: warn first, debt entries land in `policy/clippy-debt.toml` with an
owner and an expiry, then promote.

## Suppression style

When a lint genuinely must be suppressed inline:

```rust
#[expect(
    clippy::indexing_slicing,
    reason = "policy:no-panic:panic-0042 — bounded by upstream loop"
)]
let head = bytes[..n];
```

- Always use `#[expect(...)]`, never `#[allow(...)]`. (`clippy::allow_attributes`
  enforces this.)
- Always include `reason = "..."`. (`clippy::allow_attributes_without_reason`
  enforces this.)
- Reference the policy receipt id in the reason when one exists.

## Planned lints

Planned lints are listed in `policy/clippy-lints.toml` with an
`activate_when_msrv` field. The MSRV bump PR is the natural place to flip
each one. `check-lint-policy` will warn if a planned lint is mistakenly
activated before its target MSRV.

## See also

- `docs/NO_PANIC_POLICY.md` — semantic no-panic checker and allowlist schema.
- `docs/FILE_POLICY.md` — non-Rust file policy and allowlist schema.
- `policy/clippy-lints.toml` — current active + planned lint set.
- `policy/clippy-debt.toml` — staged warn-level debt with owner and expiry.
