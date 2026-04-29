# faultline-git

Adapter crate around local Git operations and metadata extraction.

Primary files:
- `src/lib.rs`

Responsibilities:
- Treat this crate as an infrastructure adapter behind `HistoryPort` and `CheckoutPort`.
- Keep command invocation and process behavior explicit and platform-aware.
- Preserve scratch-worktree cleanup and CODEOWNERS parsing behavior.

Behavior notes:
- Most tests assume a real git binary is available on `PATH`.
- Stale scratch worktree cleanup and codeowner parsing should stay resilient to malformed repo state.

Validation:
- `cargo test -p faultline-git`
- Adapter tests may require actual git fixtures from `faultline-fixtures`

