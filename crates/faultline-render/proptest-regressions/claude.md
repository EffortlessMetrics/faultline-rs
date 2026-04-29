# Proptest Regressions

Stored proptest failure inputs for regression testing.

Primary files:
- snapshot files generated in this folder

Responsibilities:
- Preserve each file as authoritative failing input until the corresponding bug is fixed and tests are re-baselined.
- Remove or rename only with explicit intent and updated test expectations.

Validation:
- `cargo test -p faultline-render`
- `cargo test -p faultline-render -- --nocapture` while triaging a new failure entry

