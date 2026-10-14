# RELEASE.md — faultline Release Process

This document describes how to prepare, validate, and publish a faultline release.

See [AGENTS.md](AGENTS.md) for the repo overview and [MAINTAINERS.md](MAINTAINERS.md) for who approves releases.

## Version Bump

faultline uses a single workspace version in the root `Cargo.toml`:

```toml
[workspace.package]
version = "0.0.1"
```

To bump the version:

1. Update `version` in `[workspace.package]` in the root `Cargo.toml`.
2. If the `AnalysisReport` schema has changed, ensure `schema_version` in the report struct matches the new version.
3. Regenerate the JSON Schema if types changed:
   ```bash
   cargo xtask generate-schema
   ```
4. Run `cargo xtask ci-full` to verify nothing is broken.

### Versioning Policy

- Patch bump (`0.1.x`): bug fixes, documentation, internal refactors with no public API changes.
- Minor bump (`0.x.0`): new features, new crates, additive public API changes.
- Major bump (`x.0.0`): breaking public API changes, schema incompatibilities.

Public API surface for semver purposes: `faultline-types`, `faultline-codes`, `faultline-ports`, and the CLI flag set.

## Changelog Update

Before tagging a release, update the changelog:

1. Create or update `CHANGELOG.md` at the repo root.
2. Add a section for the new version with the release date.
3. Group changes under: Added, Changed, Fixed, Removed.
4. Reference relevant PRs or commits.

## Release Check

Run the full supply-chain and compatibility validation:

```bash
cargo xtask release-check
# or
just release-check
```

This executes in sequence:

1. `cargo deny check` — license compliance, advisory database, banned crates, yanked dependencies. Policy is defined in `deny.toml`.
2. `cargo audit` — known vulnerability scan against the RustSec advisory database.
3. `cargo semver-checks` — public API compatibility verification for `faultline-types`, `faultline-codes`, and `faultline-ports`.

All three must pass before tagging.

### Resolving Failures

| Check | Failure | Resolution |
|-------|---------|------------|
| `cargo deny` — banned license | A dependency uses a license not in the allow list | Replace the dependency or add the license to `deny.toml` with maintainer approval |
| `cargo deny` — advisory | A dependency has a known vulnerability | Update the dependency or apply a patch |
| `cargo deny` — yanked | A dependency version has been yanked | Update to a non-yanked version |
| `cargo audit` | Known vulnerability | Update the affected dependency |
| `cargo semver-checks` | Breaking API change | Bump the major/minor version or revert the breaking change |

## Tag Creation

After the version bump, changelog update, and release check all pass:

1. Commit the version bump and changelog:
   ```bash
   git add Cargo.toml Cargo.lock CHANGELOG.md
   git commit -m "release: v0.2.0"
   ```

2. Create an annotated tag:
   ```bash
   git tag -a v0.2.0 -m "v0.2.0"
   ```

3. Push the tag:
   ```bash
   git push origin v0.2.0
   ```

The tag push triggers the release CI workflow (`.github/workflows/release.yml`), which runs `cargo xtask release-check` before building release artifacts.

## crates.io Publication

The initial registry release targets `0.0.1`. Publish these packages in dependency
order, all at the workspace version:

1. `faultline-codes`
2. `faultline-types`
3. `faultline-localization`
4. `faultline-surface`
5. `faultline-ports`
6. `faultline-app`
7. `faultline-git`
8. `faultline-probe-exec`
9. `faultline-store`
10. `faultline-render`
11. `faultline-junit`
12. `faultline-loader`
13. `faultline-sarif`
14. `faultline-cli`

`faultline-fixtures` and `xtask` are private workspace tooling and must not be
published. Internal runtime dependencies include both a local path and a registry
version; unversioned development dependencies on fixtures are omitted by Cargo
when packaging.

The release workflow runs checks only. Use an existing authenticated Cargo
session for publication after the release checks and independent review pass.
Check each package with `cargo package --locked -p <package>` and
`cargo publish --dry-run --locked -p <package>`, then publish with
`cargo publish --locked -p <package>`. Dependent package checks require the earlier
packages to be available in the registry. Confirm each new version's repository
identity on crates.io before proceeding to the next package.

After publication, verify installation with:

```bash
cargo install faultline-cli --version 0.0.1 --locked
faultline-cli --help
```

The installed executable retains the existing name `faultline-cli`; its help
uses the product name `faultline`. The report schema version is independent of
the package version and remains unchanged when only release metadata changes.

## Binary Distribution

The crates.io package installs by compiling Rust source. Prebuilt platform binaries are not provided; binary distribution decisions are deferred until the project reaches a stable release.

When binary distribution is adopted:
- The release CI workflow will build platform binaries (Linux, macOS, Windows).
- Binaries will be attached to GitHub Releases.
- `cargo install faultline-cli` will remain the primary install path.

## Pre-Release Checklist

- [ ] All tests pass: `cargo xtask ci-full`
- [ ] Version bumped in workspace `Cargo.toml`
- [ ] `schema_version` updated if schema changed
- [ ] JSON Schema regenerated if types changed: `cargo xtask generate-schema`
- [ ] Golden snapshots updated: `cargo insta review`
- [ ] Changelog updated
- [ ] Release check passes: `cargo xtask release-check`
- [ ] Tag created and pushed

## Cross-References

| Document | Purpose |
|----------|---------|
| [AGENTS.md](AGENTS.md) | Repo overview, artifact contracts, escalation rules |
| [TESTING.md](TESTING.md) | Verification matrix, CI tiers, golden artifact workflow |
| [MAINTAINERS.md](MAINTAINERS.md) | Who approves releases, escalation path |
| [docs/verification-matrix.md](docs/verification-matrix.md) | Per-crate verification techniques |
| [docs/patterns/catalog.md](docs/patterns/catalog.md) | Artifact-First Boundary and Golden Artifact Contract patterns |
