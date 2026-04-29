# faultline-render

Artifact rendering adapter for report JSON, HTML, and optional Markdown.

Primary files:
- `src/lib.rs`
- `src/markdown.rs`
- `tests/bdd_scenarios.rs`
- `proptest-regressions/*.snap`

Responsibilities:
- Keep report schema compatibility in `analysis.json`.
- Preserve HTML structure expected by consumers and snapshots.
- Keep Markdown rendering aligned with `faultline-cli` and `xtask export-markdown`.
- Snapshot artifacts (`.snap` / `analysis.json` / `index.html`) are part of golden contract.

Validation:
- `cargo test -p faultline-render`
- `cargo insta test -p faultline-render` for snapshot-sensitive edits
- `cargo xtask golden` and `cargo insta review` only when artifact diffs are intentional

