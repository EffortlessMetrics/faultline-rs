# faultline-cli

This crate is the user-facing CLI entrypoint and owns argument parsing, command dispatch, and request construction for `faultline-app`.

Primary files:
- `src/main.rs`

Responsibilities:
- Keep CLI input validation behavior compatible with existing flags and exit codes.
- Preserve subcommands: `reproduce`, `diff-runs`, `export-markdown`.
- Build `AnalysisRequest` and `SearchPolicy` from CLI arguments.

Behavior notes:
- `--help` is a golden artifact; update snapshots only when message/format changes are intentional.
- Input conflicts (`resume/force/fresh`) must remain rejected before invoking app orchestration.

Validation:
- `cargo test -p faultline-cli --test smoke`
- `cargo run -p faultline-cli -- --help`
- `cargo test -p faultline-cli` and `cargo insta test -p faultline-cli` when snapshots change

