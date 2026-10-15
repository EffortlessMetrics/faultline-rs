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

## Tag Creation

After the version bump, changelog update, and release check all pass:

1. Commit the version bump and changelog:
   ```bash
   git add Cargo.toml Cargo.lock CHANGELOG.md
   git commit -m "release: v0.0.1"
   ```

2. Create an annotated tag:
   ```bash
   git tag -a v0.0.1 -m "v0.0.1"
   ```

3. Push the tag:
   ```bash
   git push origin v0.0.1
   ```

The annotated tag must match the workspace version and point to a reviewed commit
merged into `main`. The tag push triggers `.github/workflows/release.yml`, which
runs release checks, builds and dry-runs the complete public package graph, and
inspects clean source-bound archives before publishing to crates.io. Branch
pushes to `main` and `release/**` run qualification only; they never publish.

Workflow changes are source changes. After merging a release-workflow fix, use
the resulting reviewed main SHA as the tag target and obtain fresh qualification
receipts for it. Earlier source-SHA receipts are not reused or relabeled.

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

The release workflow uses the existing repository `CARGO_REGISTRY_TOKEN` secret
only in its publication step after a matching tag or explicitly enabled recovery. It has read-only GitHub contents access;
checkout does not persist Git credentials. Do not print, copy, or pass the token
on the command line. Independent review and required checks must pass before
creating the release tag. Cargo 1.93 supports checking the complete graph before
uploading any package:

```bash
cargo package --workspace --exclude xtask --exclude faultline-fixtures --locked
cargo publish --dry-run --workspace --exclude xtask --exclude faultline-fixtures --locked
```

On a matching annotated tag, the workflow publishes the reviewed graph with
`cargo publish --workspace --exclude xtask --exclude faultline-fixtures --registry
crates-io --locked --jobs 1 --no-verify`. The two complete verification builds
above run before the token is available; `--no-verify` avoids running dependency
build scripts again with publishing credentials. Packaging and upload still run.
The workflow then checks all 14 version-specific registry identities and
checksums against the qualified archive receipts and verifies the exact-version
CLI installation in a fresh install directory. These checks do not receive the
publishing token. A failure is not a completed release; keep the release issue
open until registry and installation verification succeed.

### Immutable v0.0.1 checkout recovery

The first v0.0.1 tag run failed before packaging or upload because checkout's
implicit tag-plus-commit fallback replaced only the runner-local annotated tag
ref with its peeled commit. The remote tag remains immutable. New runs use a
SHA-only controller checkout, preserving the fetched annotated tag object.

After this workflow repair is reviewed, merged and qualified, the release
operator can dispatch `release.yml` from `main` with `publish=false` (the default).
This narrowly pinned recovery checks the existing v0.0.1 object
`bd1d904ef84f6f0bc6f2b361de9bc6931bc74fa0`, source commit
`a2c59ef1cedfee120762a4e7727b62224e57a8e7` and tree
`66deea4bc3e0a490eda8e08f4392dead3f62c177`. There are no arbitrary tag or source
inputs. The fixed controller's tests run before checkout selects the unchanged
release source. Validation still comes from the executing fixed workflow.
Controller SHA and release-source SHA are separate identities; receipts and
artifact names use the validated release source.

The default dispatch reruns all checks, cache isolation, package verification,
publication dry-run and archive inspection without publishing. Independently
compare its 14 archives and complete internal lock graph with the qualified
merged-main artifact from Release Check run `38725082016`. Recheck that all 14
registry versions remain absent before the authorized operator dispatches the
same reviewed main workflow with `publish=true`. This repeats qualification
before the existing credential-bound upload and registry/install verification.
A default-off dispatch cannot be treated as publication. If any version is
already present, follow the partial-publication process below instead.

Do not rerun the old failed tag workflow, move the tag or select a newer package
source to recover this failure. Dispatch and tag runs share the same release
concurrency group. Source-workflow maintenance does not itself authorize the
operator to publish.

### Interrupted publication and reruns

Workspace publication is not a transaction and is not an idempotent retry.
Cargo 1.93 refuses an already-published selected version before starting a new
workspace upload. A registry confirmation timeout can occur after a successful
upload. Do not blindly rerun the job, delete or move the release tag, or bump
versions to conceal a partial result.

First inspect every selected version on crates.io and compare repository,
checksum, archive source SHA and clean VCS metadata with the same tag's receipts.
If all 14 are present and match, rerun only read-only registry/install
verification. If only some match, retain the same reviewed tag/source and
independently review a recovery plan selecting only the unpublished packages in
the dependency order above. Use an existing authenticated Cargo session with
`cargo publish --locked --jobs 1 --registry crates-io -p <unpublished-package>`;
dependencies must already be available in the registry. Any occupied version
with different identity or contents blocks recovery and requires maintainer
investigation. Never treat an existing version as success without verification.

Before publication, confirm that each package archive contains the final README
and package description. These are the crates.io landing-page copy.

After publication, verify installation with:

```bash
cargo install faultline-cli --version 0.0.1 --locked
faultline-cli --help
```

The installed executable retains the existing name `faultline-cli`.
The report schema version is independent of
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
