# TESTING.md — faultline Verification Guide

This document explains how to test the faultline codebase: which techniques apply to which crates, how to run each CI tier locally, and how to add new tests.

See [AGENTS.md](AGENTS.md) for the repo overview and [MAINTAINERS.md](MAINTAINERS.md) for review expectations.

## Verification Matrix by Crate Tier

Each crate tier has specific verification techniques assigned. The full matrix is at [docs/verification-matrix.md](docs/verification-matrix.md).

### Domain Crates (pure logic, no I/O)

| Crate | Property | Unit | Mutation |
|-------|----------|------|----------|
| `faultline-codes` | ✓ (via types) | ✓ | — |
| `faultline-types` | ✓ | ✓ | — |
| `faultline-localization` | ✓ | ✓ | ✓ |
| `faultline-surface` | ✓ | ✓ | — |

Domain property tests run against pure logic with no I/O. Minimum 100 cases per property.

### Adapter Crates (infrastructure boundaries)

| Crate | Property | Unit | Golden | Fuzz |
|-------|----------|------|--------|------|
| `faultline-git` | — | ✓ | — | — |
| `faultline-probe-exec` | ✓ | ✓ | — | — |
| `faultline-store` | ✓ | ✓ | — | — |
| `faultline-render` | ✓ | ✓ | ✓ | — |
| `faultline-sarif` | ✓ | ✓ | ✓ | — |
| `faultline-junit` | ✓ | ✓ | ✓ | — |

Adapter property tests may use `tempfile` for filesystem isolation.

### App, Entry, and Tooling Crates

| Crate | Property | Unit | Golden | Mutation | Smoke |
|-------|----------|------|--------|----------|-------|
| `faultline-app` | ✓ | ✓ | — | ✓ | — |
| `faultline-cli` | ✓ | ✓ | ✓ | — | ✓ |
| `xtask` | — | ✓ | — | — | — |

## Running CI Tiers Locally

### ci-fast (every push)

Runs formatting check, linting, and the full test suite:

```bash
cargo xtask ci-fast
# or
just ci
```

This executes in sequence, stopping on first failure:
1. `cargo fmt --check`
2. `cargo clippy --workspace -- -D warnings`
3. `cargo test --workspace` (uses `cargo-nextest` if installed, falls back to `cargo test`)

Target time: under 5 minutes.

### ci-full (pull requests)

Runs ci-fast plus golden artifact checks and schema validation:

```bash
cargo xtask ci-full
# or
just ci-full
```

Additional checks beyond ci-fast:
- Golden/snapshot test verification (via `insta`)
- JSON Schema drift detection against `schemas/analysis-report.schema.json`

Target time: under 10 minutes.

### ci-extended (manual / release branches)

Mutation testing and supply-chain checks, run manually or on release branches:

```bash
cargo xtask mutants        # or: just mutants
cargo xtask release-check  # or: just release-check
```

Requires external tools: `cargo-mutants`, `cargo-deny`, `cargo-audit`, `cargo-semver-checks`. The xtask will tell you what to install if anything is missing.

## How to Add a New Property Test

1. Choose the crate. Domain logic tests go in domain crates; boundary behavior tests go in adapter crates.

2. Add `proptest` to the crate's `[dev-dependencies]` if not already present:
   ```toml
   [dev-dependencies]
   proptest = { workspace = true }
   ```

3. Write the property test with at least 100 cases:
   ```rust
   use proptest::prelude::*;

   proptest! {
       #![proptest_config(ProptestConfig { cases: 100, .. ProptestConfig::default() })]

       #[test]
       fn prop_my_invariant(input in arb_my_input()) {
           let result = my_function(input);
           prop_assert!(result.is_valid());
       }
   }
   ```

4. If you need generators, add them to `faultline-fixtures/src/arb.rs` for reuse across crates.

5. Add an entry to [docs/scenarios/scenario_index.md](docs/scenarios/scenario_index.md) with the scenario name, problem description, generator, crate, and invariant.

6. Run `cargo test -p <crate>` to verify, then `cargo xtask ci-fast` for the full suite.

## How to Add a New Fixture Scenario

1. Use builders from `faultline-fixtures`:
   ```rust
   use faultline_fixtures::RevisionSequenceBuilder;

   let seq = RevisionSequenceBuilder::new()
       .with_commits(5)
       .build();
   ```

2. Write the test as a `#[test]` function in the crate's `tests/` directory or inline `#[cfg(test)]` module.

3. Add an entry to [docs/scenarios/scenario_index.md](docs/scenarios/scenario_index.md).

4. If the scenario maps to a requirement, update [docs/scenarios/behavior_map.md](docs/scenarios/behavior_map.md).

5. Run `cargo xtask ci-fast` to verify.

## How to Update Golden Artifacts

Golden tests use `insta` snapshots to guard artifact stability. Three artifacts are covered:
- `analysis.json` — in `faultline-render`
- `index.html` — in `faultline-render`
- CLI `--help` text — in `faultline-cli`

When a golden test fails after a code change:

1. Run the tests to see the diff:
   ```bash
   cargo test -p faultline-render
   # or
   cargo test -p faultline-cli
   ```

2. Review the changes interactively:
   ```bash
   cargo insta review
   ```

3. Accept the new snapshots if the changes are intentional.

4. Commit the updated `.snap` files alongside your code change.

Or use the xtask shortcut:
```bash
cargo xtask golden
# or
just golden
```

## How to Run Mutation Tests

Mutation testing verifies that the test suite detects meaningful code changes. Currently configured for:
- `faultline-localization` — core narrowing logic and outcome classification
- `faultline-app` — orchestration loop and boundary validation

```bash
cargo xtask mutants
# or
just mutants
```

This runs `cargo mutants -p faultline-localization -- --lib`. Surviving mutants indicate gaps in test coverage.

Requires `cargo-mutants`:
```bash
cargo install cargo-mutants
```

## How to Run Fuzz Tests

Fuzz testing exercises deserialization and parsing paths with arbitrary input. Currently configured for:
- `AnalysisReport` JSON deserialization in `faultline-types`

```bash
cargo xtask fuzz --duration 60
# or
just fuzz 60
```

Default duration is 60 seconds. Requires `cargo-fuzz`:
```bash
cargo install cargo-fuzz
```

## Updating the JSON Schema

If you change any type transitively referenced by `AnalysisReport`, regenerate the schema:

```bash
cargo xtask generate-schema
```

CI will catch schema drift with the message `"schema drift detected"` if you forget.

## Cross-References

| Document | Purpose |
|----------|---------|
| [AGENTS.md](AGENTS.md) | Repo overview, command surface, escalation rules |
| [RELEASE.md](RELEASE.md) | Release process and supply-chain checks |
| [MAINTAINERS.md](MAINTAINERS.md) | Code ownership and review expectations |
| [docs/verification-matrix.md](docs/verification-matrix.md) | Full per-crate verification matrix |
| [docs/scenarios/scenario_index.md](docs/scenarios/scenario_index.md) | Scenario atlas — every test indexed |
| [docs/patterns/catalog.md](docs/patterns/catalog.md) | Pattern catalog including Proof-Carrying Change |
