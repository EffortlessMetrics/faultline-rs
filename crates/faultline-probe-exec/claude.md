# faultline-probe-exec

Probe execution adapter implementing `ProbePort`.

Primary files:
- `src/lib.rs`

Responsibilities:
- Preserve timeout semantics (`timeout_seconds`) and `ShellKind` handling.
- Keep output truncation behavior consistent with store-backed log persistence.
- Keep command construction deterministic for reproducibility.

Validation:
- `cargo test -p faultline-probe-exec`
- If changing probe command formation, run with representative shell variants on each supported platform.

