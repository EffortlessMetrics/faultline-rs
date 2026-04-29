# Xtask Tests

Tests for scaffold commands, schema drift checks, and CI-contract helper functions.

Primary files:
- `ci_messages.rs`
- `doc_structure.rs`
- `schema_drift.rs`
- `scenarios.rs`

Responsibilities:
- Keep contract messages and doc structure assumptions synchronized with actual command behavior.
- Update tests when commands or generated output formats change.

Validation:
- `cargo test -p xtask --tests`

