# CLI Tests

Integration tests for end-to-end CLI behavior and smoke paths.

Primary files:
- `smoke.rs`

Responsibilities:
- Keep smoke assertions aligned with real command output and exit codes.
- Preserve environment assumptions (`fixture` repo layout and available binary paths).
- Treat this folder as contract tests for user-visible CLI behavior.

Validation:
- `cargo test -p faultline-cli --test smoke`

