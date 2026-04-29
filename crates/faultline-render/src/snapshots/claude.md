# Render Snapshots

Snapshot outputs for rendered artifacts (`analysis.json`, `index.html`) checked by `insta`.

Primary files:
- `*.snap`

Responsibilities:
- Treat these as golden test baselines; update only intentionally and via `cargo insta review`.
- Keep snapshots aligned with template and rendering logic changes in `src/lib.rs` and `src/markdown.rs`.

Validation:
- `cargo insta test -p faultline-render`
- `cargo insta review` after intentional HTML/JSON rendering changes

