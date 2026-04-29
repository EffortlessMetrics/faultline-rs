# fuzz

Standalone fuzzing crate for faultline adapters and serializers.

Primary files:
- `Cargo.toml`
- `fuzz_targets/*.rs`

Responsibilities:
- Keep fuzz targets focused on parser/input stability and export adapter robustness.
- Run fuzzing through approved CI or local `cargo xtask fuzz` wrapper when available.

Validation:
- `cargo xtask fuzz` (from workspace root)
- `cargo fuzz run fuzz_analysis_report -- -max_total_time=60`

