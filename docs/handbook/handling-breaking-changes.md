# Playbook: Handling Breaking Changes

This playbook covers how to introduce, validate, and ship a breaking change in the faultline workspace.

## What Counts as Breaking

| Change | Breaking? | Semver Impact |
|--------|-----------|---------------|
| Remove a public type or function | Yes | Major bump |
| Remove or rename a field in `AnalysisReport` | Yes | Major bump + schema version bump |
| Change a trait signature in `faultline-ports` | Yes | Major bump |
| Remove or rename a CLI flag | Yes | Major bump |
| Add a required field to `AnalysisReport` (no `#[serde(default)]`) | Yes | Major bump + schema version bump |
| Add an optional field with `#[serde(default)]` | No (additive) | Minor bump + schema version bump |
| Add a new CLI flag or subcommand | No (additive) | Minor bump |
| Internal refactor with no public API change | No | Patch bump |

Public API surface for semver: `faultline-types`, `faultline-codes`, `faultline-ports`, and the CLI flag set.

## Step-by-Step

### 1. Assess the Impact

Before writing code, identify what breaks:

- Which crates are affected? Check dependents with `cargo tree --invert -p <crate>`.
- Does the JSON artifact format change? If yes, see [Bumping schema_version](bumping-schema-version.md).
- Do export adapters (`faultline-sarif`, `faultline-junit`) need updates?
- Does the CLI flag set change? If yes, the `--help` golden snapshot must be updated.

### 2. Preserve Backward Compatibility Where Possible

Use `#[serde(default)]` on new `AnalysisReport` fields so older JSON files still deserialize:

```rust
#[serde(default)]
pub new_field: Vec<NewType>,
```

For enum variants, adding a new variant is generally safe for deserialization (existing JSON won't contain it). Removing a variant breaks deserialization of old data.

For trait changes in `faultline-ports`, consider adding a new method with a default implementation rather than changing an existing signature:

```rust
pub trait HistoryPort {
    // existing methods unchanged...

    fn new_capability(&self, args: &[String]) -> Result<Vec<String>> {
        Ok(vec![]) // default: no-op
    }
}
```

### 3. Make the Change

1. Update the types or traits.
2. Fix all compilation errors across the workspace.
3. Update or add tests (property tests for domain invariants, unit tests for edge cases).
4. If `AnalysisReport` changed, follow the [Bumping schema_version](bumping-schema-version.md) playbook.

### 4. Run Semver Checks

```bash
cargo semver-checks
```

This compares the current public API against the last published version. If it reports breaking changes, you need a version bump.

If `cargo semver-checks` is not installed:

```bash
cargo install cargo-semver-checks
```

### 5. Bump the Version

Update `version` in the root `Cargo.toml`:

```toml
[workspace.package]
version = "0.3.0"  # was "0.2.0"
```

Follow semver:
- Patch (`0.2.x`) — bug fixes, no API changes
- Minor (`0.x.0`) — additive changes, new features
- Major (`x.0.0`) — breaking changes

### 6. Update Golden Snapshots

```bash
cargo test          # see what changed
cargo insta review  # accept intentional changes
```

### 7. Update Documentation

Breaking changes typically require updates to:
- `CHANGELOG.md` — document the breaking change under "Changed" or "Removed"
- `AGENTS.md` — if artifact contracts or escalation rules changed
- `docs/crate-map.md` — if crate responsibilities shifted
- `docs/book/src/reference/artifact-schema.md` — if the artifact format changed

### 8. Run Full Validation

```bash
cargo xtask ci-full        # formatting, linting, tests, golden, schema
cargo xtask release-check  # cargo-deny, cargo-audit, cargo-semver-checks
```

Both must pass before the change can be merged.

### 9. Get Review

Breaking changes require Level 3 review per [MAINTAINERS.md](../../MAINTAINERS.md):

- `cargo semver-checks` pass or explicit version bump
- Schema regeneration and golden snapshot update
- Review of downstream impact on export adapters
- One approval from a maintainer

Architectural changes (new crate boundaries, pattern changes) require Level 4: two maintainer approvals plus an ADR.

## CI Checks That Catch Breaking Changes

| Check | What It Catches |
|-------|----------------|
| `cargo semver-checks` | Public API incompatibilities |
| Schema drift detection | `AnalysisReport` changes without schema regeneration |
| Golden snapshot tests | Artifact format changes without explicit review |
| `cargo deny check` | License or supply-chain issues from new dependencies |

## Cross-References

| Document | Purpose |
|----------|---------|
| [RELEASE.md](../../RELEASE.md) | Full release process and pre-release checklist |
| [MAINTAINERS.md](../../MAINTAINERS.md) | Escalation levels for breaking changes |
| [Bumping schema_version](bumping-schema-version.md) | Schema version bump procedure |
| [Pattern Catalog — Artifact-First Boundary](../patterns/catalog.md) | Artifact changes require schema review |
