# fuzz_targets

Per-target fuzz harnesses using `libfuzzer-sys`.

Primary files:
- `fuzz_analysis_report.rs`
- `fuzz_cli_args.rs`
- `fuzz_git_diff_parse.rs`
- `fuzz_html_escape.rs`
- `fuzz_junit_export.rs`
- `fuzz_sarif_export.rs`
- `fuzz_store_json.rs`

Responsibilities:
- Maintain target coverage for parser/input/formatting edges.
- Keep fuzz corpus assumptions explicit and reproducible.

Validation:
- `cargo fuzz run fuzz_analysis_report -- -max_total_time=60`
- `cargo fuzz run fuzz_cli_args -- -max_total_time=60`

