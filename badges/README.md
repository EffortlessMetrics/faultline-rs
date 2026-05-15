# Badge Endpoints

This directory contains generated Shields endpoint JSON for public repo-scoped
verification badges.

Badges summarize repository state only. They are not PR evidence, coverage,
mutation proof, release readiness, or merge readiness. Diff-scoped evidence is
generated under `target/ripr/` by `cargo xtask ripr-pr`,
`cargo xtask ripr-review-comments`, and related commands.

Regenerate and check:

```bash
cargo xtask badges
cargo xtask badges --check
```

`badges/ripr.json` is generated from `ripr check --format repo-badge-shields`.
This repository does not publish a `ripr+` endpoint until it has the required
repo-scoped test-efficiency proof.
