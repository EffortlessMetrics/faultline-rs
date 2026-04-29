# faultline-sarif

Adapter for SARIF v2.1.0 export from `AnalysisReport`.

Primary files:
- `src/lib.rs`

Responsibilities:
- Preserve SARIF schema fields (`schema`, `version`, `runs`) and result levels.
- Keep suspect-surface entries and change-based locations stable.
- Ensure changes remain readable by pipeline consumers expecting SARIF JSON.

Validation:
- `cargo test -p faultline-sarif`

