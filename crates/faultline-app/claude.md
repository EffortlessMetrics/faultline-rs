# faultline-app

This crate owns orchestration of regression localization for one run. Keep orchestration logic and persistence/policy flow here, not probe execution details or history transport.

Primary files:
- `src/lib.rs`

Responsibilities:
- Coordinate ports (`HistoryPort`, `CheckoutPort`, `ProbePort`, `RunStorePort`).
- Build revision sequence, replay/cache observations, enforce boundaries, and emit `AnalysisReport`.
- Compute and forward owner hints and suspect surface scoring inputs.
- Save reports to the run store via `FileRunStore`.

Key behaviors to preserve:
- `LocalizeOptions` controls `--force`, `--fresh`, and `--no-render` semantics.
- Observation replay and `sequence_index` handling must stay monotonic for deterministic reporting.
- `compute_flake_signal` is applied only when retries are enabled.

Validation:
- `cargo test -p faultline-app`
- `cargo test -p faultline-app -- --nocapture` for local behavior traces when needed

