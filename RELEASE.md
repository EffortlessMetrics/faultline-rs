# RELEASE.md — faultline-rs Release Process

This document describes how to prepare, validate, and publish a faultline-rs release.

See [AGENTS.md](AGENTS.md) for the repo overview and [MAINTAINERS.md](MAINTAINERS.md) for who approves releases.

## Version Bump

faultline-rs uses a single workspace version in the root `Cargo.toml`:

```toml
[workspace.package]
version = "0.0.1"
```

To bump the version:

1. Update `version` in `[workspace.package]` in the root `Cargo.toml`.
2. If the `AnalysisReport` schema changes, bump its `schema_version` according to the report contract. The schema version is independent of the package version.
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

For the first registry release there is no crates.io API baseline. Compare the
public crates with the reviewed pre-release source revision instead:

```bash
cargo xtask release-check --baseline-rev 9f6cba2cbd6ac381d732ebdd4f226704ec716217
```

This still runs deny and audit, then semver-checks for the documented public
surfaces using strict patch-compatible API checks. The prototype's workspace
version was not a published release; it is not used to relax compatibility.
Subsequent releases use the default registry baseline by omitting the flag.

### Resolving Failures

| Check | Failure | Resolution |
|-------|---------|------------|
| `cargo deny` — banned license | A dependency uses a license not in the allow list | Replace the dependency or add the license to `deny.toml` with maintainer approval |
| `cargo deny` — advisory | A dependency has a known vulnerability | Update the dependency or apply a patch |
| `cargo deny` — yanked | A dependency version has been yanked | Update to a non-yanked version |
| `cargo audit` | Known vulnerability | Update the affected dependency |
| `cargo semver-checks` | Breaking API change | Bump the major/minor version or revert the breaking change |

## Corrected 0.0.1 naming and partial publication

The public package and executable are both `faultline-rs`. The thirteen
supporting library package names remain `faultline-*`. Source still lives in
`crates/faultline-cli`; that directory is not the published package name.

The initial upload stopped on a crates.io rate limit after publishing five
supporting `0.0.1` versions: `faultline-codes`, `faultline-types`, `faultline-junit`,
`faultline-loader` and `faultline-localization`. Their original checksums and
source remain immutable. Neither `faultline` nor `faultline-rs` was attempted in
that run. See the [partial-publication receipt](https://github.com/EffortlessMetrics/faultline-rs/issues/52#issuecomment-6191584145).
That historical snapshot is not a current registry check or a completed release.

The source-controlled [0.0.1 plan](.github/release-0.0.1.json) records those five
original hashes and source identities, plus the dependency order for the nine
remaining packages:

1. `faultline-surface`
2. `faultline-ports`
3. `faultline-app`
4. `faultline-git`
5. `faultline-probe-exec`
6. `faultline-store`
7. `faultline-render`
8. `faultline-sarif`
9. `faultline-rs`

All remain at `0.0.1`. `faultline-fixtures` and `xtask` are private and must never
be published. Do not rebuild the five occupied versions and relabel them as
corrected-source archives. Their embedded README describes the earlier source;
changing an occupied crate's landing copy requires a new version.

### Qualification

The release helper is deliberately scoped to this release. It validates exact
package sets, unique plan entries, versions, checksum formats and dependency
order. Branch pushes to `main` and `release/**` qualify without publishing:

1. Run generated-fixture workflow contracts before tool installation or packaging.
2. Validate clean checked-out source and preserve the original annotated tag.
3. Run the existing supply-chain and compatibility checks.
4. Recoverably quarantine cached `target/package` before Cargo can uplift files
   without truncating stale destinations.
5. Check the five original registry identities and retain their exact archives.
6. Build-verify only the nine selected packages against the actual registry for
   excluded dependencies, then dry-run the same nine-package graph. Both Cargo
   operations run without publishing credentials.
7. Inspect all fourteen archives with separate old/new source, README and
   manifest provenance. Reject incomplete gzip streams, second members, tails,
   wrong source bytes and differences between canonical and dry-run archives.
8. Derive the complete internal lock closure from packaged manifests, including
   root versioned dev and target dependencies, excluding transitive dev-only
   dependencies. Require exact records, edges, versions, registry sources and
   full-archive checksums against the nine fresh plus five original hashes.
9. Reconcile all version-specific registry identities. Occupied versions must
   match their exact qualified checksum and repository; absence is allowed only
   for the nine fresh packages. Upload the mixed-source artifact and receipt.

A dry-run warning that a version already exists is not success or proof of
publishability. The independent archive and registry checks must establish its
exact identity. Normal package build verification remains enabled; no dry-run
is accepted as a substitute for tests, lint or compatibility qualification.

Obtain an independent exact-head source review and required checks before
merging. Then independently qualify the fresh merged-main mixed-source artifact.
Earlier source receipts must not be reused as proof for a newer source. The
five original anchors retain their explicitly different provenance.

### Corrected source tag and publication

The old annotated `v0.0.1` tag remains fixed at object
`bd1d904ef84f6f0bc6f2b361de9bc6931bc74fa0`, source
`a2c59ef1cedfee120762a4e7727b62224e57a8e7`, tree
`66deea4bc3e0a490eda8e08f4392dead3f62c177`. Never move or delete it.

After corrected merged-main source and the mixed graph qualify, and publication
is explicitly authorized, the release operator creates the distinct annotated
`faultline-rs-v0.0.1` tag on that exact qualified source. Its suffix matches the
workspace version. Push only that new tag through the normal tag workflow.
The workflow checks its raw annotated object against the remote, its peeled
commit and tree against checked-out source, and main ancestry before any upload.
The SHA-only checkout preserves the annotated ref.

The obsolete old-source recovery dispatch is removed. The current controller
rejects old `v0.0.1`, `faultline-cli`, arbitrary source selection and dispatch
publication. This does not rewrite historical workflow runs: do not rerun the
old controller, which still contains its old code. The original checkout failure
and default-off recovery remain historical evidence in
[release tracking](https://github.com/EffortlessMetrics/faultline-rs/issues/52#issuecomment-6191064907).
They do not qualify or publish the renamed source.

Tag execution repeats all qualification. The existing repository
`CARGO_REGISTRY_TOKEN` secret is bound only to the publishing step. The helper
removes it from inherited environments and gives it only to actual Cargo upload
subprocesses, never builds, dry-runs, checkout, tools, artifact uploads or registry
reads. GitHub permissions remain `contents: read`, with checkout credentials not
persisted. No token appears on a command line or in logs.

The upload loop follows the nine-package dependency order. For an absent version,
it first dry-runs that single package without credentials after its dependencies
are confirmed present, compares the complete temporary and canonical archive
bytes with its qualified receipt, and uploads with `--no-verify`. The earlier
credential-free verification builds remain authoritative; no dependency build
script receives publishing credentials. Each successful upload must be confirmed
by its version-specific registry checksum before the next package can proceed.
New uploads are separated by at least sixty seconds. This spacing does not
promise to avoid crates.io limits.

### Partial success, rate limits and resume

Publication is not a transaction. Stop on any upload failure, unknown
confirmation, identity mismatch or HTTP 429. Report `Retry-After` when available
and do not loop uploads or blindly rerun at an advertised time. First reconcile
actual registry state against the immutable mixed-source receipt. A failure or
partial success is not a completed release.

A reviewed rerun of the same corrected tag repeats qualification and verifies
all existing versions. The loop skips an already-present version only after its
repository, non-yanked version and exact qualified checksum match; it never
silently treats an occupied name as success or attempts to overwrite it. A
mismatching occupied version blocks further uploads. Never move the tag, yank
versions or bump versions to conceal a partial result.

After all fourteen registry identities match, a fresh install directory verifies:

```bash
cargo install faultline-rs --version 0.0.1 --locked
faultline-rs --help
faultline-rs --version
```

The installed executable is named `faultline-rs`. Keep the release issue open
until registry and installation verification succeed. The report schema,
`faultline-report` output directory, cache locations and serialized report-format
identifiers are unchanged by this naming repair.

## Binary Distribution

The crates.io package installs by compiling Rust source. Prebuilt platform binaries are not provided; binary distribution decisions are deferred until the project reaches a stable release.

When binary distribution is adopted:
- The release CI workflow will build platform binaries (Linux, macOS, Windows).
- Binaries will be attached to GitHub Releases.
- `cargo install faultline-rs` will remain the primary install path.

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
