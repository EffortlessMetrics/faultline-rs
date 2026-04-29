# faultline-store

Filesystem-backed run store adapter for run metadata, observations, and reports.

Primary files:
- `src/lib.rs`

Responsibilities:
- Preserve atomic write semantics via `.tmp` + rename.
- Preserve lock file behavior in `prepare_run` / `save_report`.
- Keep observation deduplication and deterministic sorting by `sequence_index`.
- Keep full probe log persistence in `save_probe_logs`.

Validation:
- `cargo test -p faultline-store`
- If mutating lock/path behavior, run tests that touch stale lock cleanup and resume paths.

