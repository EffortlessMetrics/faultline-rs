# Playbook: Bumping `schema_version`

This playbook covers when and how to bump the `schema_version` field in `AnalysisReport`.

## When a Bump Is Needed

Bump `schema_version` when the serialized JSON structure of `AnalysisReport` changes in a way that existing consumers would notice:

- **Adding a required field** (without `#[serde(default)]`) → bump required
- **Adding an optional field** (with `#[serde(default)]`) → bump recommended (consumers may want to detect new capabilities)
- **Removing a field** → bump required
- **Renaming a field** → bump required
- **Changing a field's type** → bump required
- **Adding a new enum variant** that appears in serialized output → bump required

If the change is purely internal (no serialization impact), no bump is needed.

## Step-by-Step

### 1. Update `default_schema_version()` in `faultline-types`

Edit `crates/faultline-types/src/lib.rs`:

```rust
fn default_schema_version() -> String {
    "0.3.0".to_string()  // was "0.2.0"
}
```

Use semver: minor bump for additive changes, major bump for breaking changes.

### 2. Regenerate the JSON Schema

```bash
cargo xtask generate-schema
```

This updates `schemas/analysis-report.schema.json` from the Rust types via `schemars`. CI will fail with `"schema drift detected"` if you skip this.

### 3. Update Golden Snapshots

Run the tests to see which snapshots changed:

```bash
cargo test -p faultline-render
cargo test -p faultline-cli
```

Review and accept the changes:

```bash
cargo insta review
```

Commit the updated `.snap` files alongside your code change.

### 4. Update Export Adapters

Check if the schema change affects serialized output consumed by export adapters:

- `faultline-sarif` — uses `report.schema_version` as the SARIF tool version
- `faultline-junit` — may reference report fields in `system-out` or properties

Run their tests:

```bash
cargo test -p faultline-sarif
cargo test -p faultline-junit
```

### 5. Ensure Backward Compatibility

New fields on `AnalysisReport` should use `#[serde(default)]` so that older JSON files (with the previous `schema_version`) still deserialize correctly:

```rust
#[serde(default)]
pub new_field: Vec<NewType>,
```

The `schema_evolution_forward_compat` BDD scenario in `faultline-types` tests this — it deserializes an old-version report into the current struct.

### 6. Update Documentation

If the schema change is significant, update:
- `docs/book/src/reference/artifact-schema.md` — artifact schema reference
- `AGENTS.md` — if the change affects the artifact contracts section

### 7. Run Full CI

```bash
cargo xtask ci-full
```

This runs formatting, linting, tests, golden checks, and schema drift detection.

## Checklist

- [ ] `default_schema_version()` updated in `faultline-types`
- [ ] `cargo xtask generate-schema` run
- [ ] Golden snapshots reviewed and accepted (`cargo insta review`)
- [ ] Export adapters tested (`faultline-sarif`, `faultline-junit`)
- [ ] New fields use `#[serde(default)]` for backward compatibility
- [ ] `cargo xtask ci-full` passes

## Cross-References

| Document | Purpose |
|----------|---------|
| [AGENTS.md — Artifact Contracts](../../AGENTS.md) | JSON Schema contract and golden test workflow |
| [RELEASE.md](../../RELEASE.md) | Version bump and release process |
| [Pattern Catalog — Artifact-First Boundary](../patterns/catalog.md) | Schema changes require explicit review |
| [Pattern Catalog — Golden Artifact Contract](../patterns/catalog.md) | Snapshot tests guard artifact stability |
