# Changelog

## 0.0.1 — Unreleased

### Added

- Initial crates.io release candidate for the `faultline-cli` package and its 13 runtime dependency crates.
- Package descriptions, repository/readme metadata and inherited Rust 1.93 requirements.
- Registry version requirements for internal runtime dependencies and a documented publication order.

### Fixed

- Upgrade quick-xml, anyhow and rand to the minimum patched versions for the advisories found by the release security scan.
- Repair three documentation links and update cargo-deny configuration for the installed checker.

- Restore the complete declared Apache-2.0 license and include it in all runtime package archives.
- Version the six public runtime-crate development dependencies; keep private fixtures unpublished.

### Changed

- Align the workspace package version to the initial release target, 0.0.1.
- Keep `xtask` and `faultline-fixtures` unpublished.
- Apply user-approved policy settings for unicode-ident =1.0.24 and local development/private paths.
- Renew only the 58 existing no-panic allowances through 2026-11-14; track cleanup in [#54](https://github.com/EffortlessMetrics/faultline-rs/issues/54).
- Lead the README and first-run guide with the user's problem, use path, evidence and limits.
- Preserve the existing `faultline-cli` executable name and report schema contracts.

Release tracking: [#52](https://github.com/EffortlessMetrics/faultline-rs/issues/52).
