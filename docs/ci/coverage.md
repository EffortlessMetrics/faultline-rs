# Coverage

Codecov coverage is Rust execution-surface evidence.

It answers:
> Did tests execute this Rust surface?

It does not answer:
- whether regression localization is correct,
- whether the supplied predicate is valid,
- whether Git history traversal is correct,
- whether disposable worktree isolation is correct,
- whether process execution isolation is correct,
- whether SARIF/JUnit/HTML export behavior is complete,
- whether scenario coverage is complete,
- whether mutation adequacy is strong,
- whether release readiness is proven.

Those are separate proof lanes.

## Workflow triggers

The Coverage workflow runs on:
- push to `main`,
- `workflow_dispatch`,
- PRs labeled `coverage`, `full-ci`, or `ci:full`.

## Artifacts and receipts

Codecov comments are disabled. Durable receipts are:
- `coverage.json` — machine-readable coverage data from `cargo-llvm-cov`,
- `coverage.txt` — human-readable coverage summary,
- `lcov.info` — standard LCOV format for integration with tools,
- the GitHub Actions coverage artifact — retention policy: 14 days,
- the Codecov dashboard — public read-only visibility.
