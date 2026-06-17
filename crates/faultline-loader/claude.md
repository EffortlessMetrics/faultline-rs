# faultline-loader

Shared report-loading infrastructure helper crate for faultline.

Primary files:
- `src/lib.rs`

Responsibilities:
- Implement deterministic report file resolution (report.json > analysis.json).
- Load and deserialize `AnalysisReport` from directory paths or direct file paths.
- Emit diagnostics when both report.json and analysis.json are present.
- Return appropriate errors (exit-code-2-compatible) when no loadable report is found.

Dependents:
- `faultline-cli` — all subcommands that load reports
- `xtask` — all export commands that load reports

Validation:
- `cargo test -p faultline-loader`
