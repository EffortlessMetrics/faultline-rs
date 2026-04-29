# faultline-surface

Heuristic scoring and bucketing for changed paths.

Primary files:
- `src/lib.rs`

Responsibilities:
- Preserve path bucketing and `surface_kind` / `is_execution_surface` heuristics.
- Keep suspect ranking deterministic and stable for reporter expectations.
- Treat owner hints as supplemental metadata only.

Validation:
- `cargo test -p faultline-surface`

