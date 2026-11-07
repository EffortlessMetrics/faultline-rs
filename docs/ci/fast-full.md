# Fast and full CI maintenance

## Scope and plan

Keep the existing push Fast and pull-request Full triggers and proof commands.
Cancel superseded runs for the same workflow and branch/PR, cap jobs at 20
minutes, and grant only read access to repository contents. Cache the two Full
workflow CLI tools by platform and exact version; verify their versions even
after a cache hit. No tests, policy gates, schema checks, snapshots, or RIPR
evidence commands are removed.

This is separate from the late-qualification pilot in PR #71: it does not
change admission, required checks, labels, release workflows, or merge policy.

## Evidence and cost boundary

In [Full run 38853135607](https://github.com/EffortlessMetrics/faultline-rs/actions/runs/38853135607),
installing cargo-insta took 45 seconds and installing ripr took 112 seconds,
while `cargo xtask ci-full` took 72 seconds. Reusing their exact binaries avoids
those source builds on a cache hit. A cache miss still installs both tools with
`--locked`; version checks fail closed if the restored tool is unexpected.
This does not claim a measured post-change runtime or dollar saving.

The Rust build cache remains independent of the CLI tool cache. The tool cache
contains only `cargo-insta` and `ripr`, not credentials or unrelated Cargo tools.
Change the pinned version variables when intentionally updating either tool;
they are also part of the cache key. RIPR version updates must retain the
schema review described in the workflow.

## Verification and rollback

Validate workflow syntax with `actionlint .github/workflows/ci.yml
.github/workflows/ci-full.yml`; run `cargo xtask ci-full` for the unchanged
repository contract floor. Hosted runs establish cache restoration, concurrency,
and Linux execution behavior. Revert these two workflow changes to roll back;
no persisted application data or artifact schema changes are involved.
