# faultline-codes

This crate contains the shared enums and operator-level diagnostics used across all layers.

Primary files:
- `src/lib.rs`

Responsibilities:
- Keep enum variants stable for persisted/snapshoted contract surfaces.
- Preserve serde and schemars annotations since these values are serialized.

Validation:
- `cargo test -p faultline-codes`
- `cargo test -p faultline-codes -- --nocapture` for quick enum behavior verification

