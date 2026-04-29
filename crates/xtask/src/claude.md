# xtask Source

Module implementations for CI, schema checks, scaffolding, smoke tests, docs checks, and export helpers.

Primary files:
- `crates/xtask/src/ci.rs`
- `crates/xtask/src/schema.rs`
- `crates/xtask/src/scaffold.rs`
- `crates/xtask/src/smoke.rs`
- `crates/xtask/src/tools.rs`
- `crates/xtask/src/docs_check.rs`

Keep command contracts and user-facing output stable when changing workflows.

Validation:
- `cargo test -p xtask`
- `cargo run -p xtask -- --help`

