# xtask

Repository task runner and project automation.

Primary files:
- `src/main.rs`
- `src/ci.rs`
- `src/scaffold.rs`
- `src/schema.rs`
- `src/smoke.rs`
- `src/docs_check.rs`
- `src/tools.rs`

Responsibilities:
- Keep `ci-fast`, `ci-full`, `fuzz`, and `release-check` contracts intact.
- Preserve scenario checks and schema regeneration workflows.
- Treat scaffold commands as user-facing generators; keep templates deterministic.

Validation:
- `cargo run -p xtask -- ci-fast`
- `cargo run -p xtask -- ci-full`
- `cargo run -p xtask -- docs-check`
- `cargo run -p xtask -- check-scenarios`

