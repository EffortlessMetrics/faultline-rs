# Render Tests

Behavior tests for report rendering, BDD scenario coverage, and snapshot outputs.

Primary files:
- `bdd_scenarios.rs`

Responsibilities:
- Keep scenario coverage in sync with fixture expectations.
- Validate that new report rendering branches are covered in regression tests.

Validation:
- `cargo test -p faultline-render --test bdd_scenarios`

