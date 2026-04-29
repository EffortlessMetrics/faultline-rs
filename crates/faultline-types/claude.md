# faultline-types

Shared domain model and error type for the entire workspace.

Primary files:
- `src/lib.rs`

Responsibilities:
- Preserve serialization contracts (`serde` + `schemars`) for `AnalysisRequest`, `AnalysisReport`, and related types.
- Keep `FaultlineError` variants and `Result` alias stable unless coordinated schema/API changes exist.
- Keep hash/fingerprint functions stable because they drive run identity.

Validation:
- `cargo test -p faultline-types`
- If schema-affecting types change, run `cargo xtask generate-schema` and accept updated schema snapshots.

