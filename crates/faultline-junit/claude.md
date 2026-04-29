# faultline-junit

Adapter crate exporting `AnalysisReport` to JUnit XML.

Primary files:
- `src/lib.rs`

Responsibilities:
- Keep output schema-compatible with current JUnit expectations.
- Keep failure/status message semantics aligned with `LocalizationOutcome`.
- Preserve ordering and summaries used by downstream consumers.

Validation:
- `cargo test -p faultline-junit`

