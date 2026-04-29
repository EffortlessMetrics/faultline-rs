# faultline-render Source

Rendering implementation module.

Primary files:
- `crates/faultline-render/src/lib.rs`
- `crates/faultline-render/src/markdown.rs`

Keep report formatting deterministic and safe against missing or new fields in model types.

Validation:
- `cargo test -p faultline-render`
- `cargo insta test -p faultline-render`

