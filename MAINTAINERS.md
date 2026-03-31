# MAINTAINERS.md — faultline Code Ownership and Review

This document defines code ownership by crate, review expectations, and escalation paths.

See [AGENTS.md](AGENTS.md) for the repo overview and [TESTING.md](TESTING.md) for verification procedures.

## Code Ownership by Crate

### Domain Crates — highest review bar

| Crate | Responsibility | Review Notes |
|-------|---------------|--------------|
| `faultline-codes` | Shared diagnostic vocabulary (enums) | Changes affect the entire codebase. Requires careful review of downstream impact. |
| `faultline-types` | Pure value objects, report model, serialization contracts | Schema-affecting changes require `schema_version` bump and JSON Schema regeneration. |
| `faultline-localization` | Regression-window search engine (binary narrowing) | Core algorithm. Property test failures here are critical — see Human Review Gate below. |
| `faultline-surface` | Coarse path-based change bucketing | Changes to bucketing logic affect report output. |

### Port and Application Crates

| Crate | Responsibility | Review Notes |
|-------|---------------|--------------|
| `faultline-ports` | Outbound hexagonal port traits | Trait signature changes are breaking API changes. Requires semver review. |
| `faultline-app` | Use-case orchestration, policy enforcement | Mutation testing coverage required. Changes to the localization loop need extra scrutiny. |

### Adapter Crates

| Crate | Responsibility | Review Notes |
|-------|---------------|--------------|
| `faultline-git` | Git CLI adapter (history + checkout) | Security-sensitive: spawns external processes. Review command construction carefully. |
| `faultline-probe-exec` | Process execution adapter | Security-sensitive: runs operator-provided commands. Review timeout and exit code handling. |
| `faultline-store` | Filesystem-backed run persistence | Review atomic write and lock file handling. |
| `faultline-render` | JSON + HTML artifact writers | Golden tests must be updated for any output change. |
| `faultline-sarif` | SARIF v2.1.0 export | Output must conform to SARIF spec. Golden tests guard structure. |
| `faultline-junit` | JUnit XML export | Output must be well-formed XML. Golden tests guard structure. |

### Entry, Testing, and Tooling Crates

| Crate | Responsibility | Review Notes |
|-------|---------------|--------------|
| `faultline-cli` | Operator-facing CLI | Flag changes are public API. Golden test for `--help` must be updated. |
| `faultline-fixtures` | Fixture builders for tests | Changes may affect many test files across the workspace. |
| `xtask` | Repo operations binary | Changes to CI commands affect the entire development workflow. |

## Review Expectations

### All Changes

- Every PR must pass `cargo xtask ci-fast` (formatting, linting, tests).
- New tests must have a corresponding entry in [docs/scenarios/scenario_index.md](docs/scenarios/scenario_index.md).
- New domain logic must include property tests (minimum 100 cases per property).

### Domain Changes

- Must include or update property tests covering the changed behavior.
- Must not introduce I/O dependencies — domain crates stay pure.
- Mutation testing should be run for `faultline-localization` and `faultline-app` changes.

### Schema-Affecting Changes

- Must bump `schema_version` in `AnalysisReport` if the serialized format changes.
- Must regenerate `schemas/analysis-report.schema.json` via `cargo xtask generate-schema`.
- Must update golden snapshots via `cargo insta review`.
- Must update export adapters (`faultline-sarif`, `faultline-junit`) if affected.

### Public API Changes

- Must pass `cargo semver-checks` or include an explicit version bump.
- Public API surface: `faultline-types`, `faultline-codes`, `faultline-ports`, CLI flags.

## Human Review Gate for Property-Test Failures

Property test failures are never auto-suppressed. When a property test fails:

1. The CI run fails with the counterexample that triggered the failure.
2. A human maintainer must triage the failure:
   - **Bug in the code**: fix the implementation, not the test.
   - **Bug in the test**: fix the test with a clear explanation of why the property was wrong.
   - **Specification gap**: discuss with the team whether the acceptance criteria need updating.
3. The fix must be reviewed by a second maintainer before merge.
4. The counterexample should be added as a regression test to prevent recurrence.

This is the Human Review Gate pattern — see [docs/patterns/catalog.md](docs/patterns/catalog.md) for the full definition.

Automated agents must not:
- Delete or skip failing property tests
- Reduce the iteration count below 100
- Weaken property assertions to make tests pass
- Mark property test failures as "expected"

## Escalation Path for Breaking Changes

### Level 1 — Adapter or tooling change

Standard PR review. One approval required.

### Level 2 — Domain logic change

PR must include property test updates. Mutation testing recommended. One approval from a domain-familiar reviewer.

### Level 3 — Schema or public API change

Requires:
- `cargo semver-checks` pass or explicit version bump
- Schema regeneration and golden snapshot update
- Review of downstream impact on export adapters
- One approval from a maintainer

### Level 4 — Architectural change

Requires:
- A new or updated ADR in `docs/adr/` (use `cargo xtask scaffold adr "<title>"`)
- Update to [docs/architecture.md](docs/architecture.md)
- Update to [docs/patterns/catalog.md](docs/patterns/catalog.md) if a pattern is affected
- Two approvals from maintainers

## Supply-Chain Escalation

When `cargo deny` or `cargo audit` flags an issue:

1. Check if a patched version of the dependency exists. If so, update.
2. If no patch exists, evaluate the severity and whether the vulnerability is reachable in faultline's usage.
3. For critical/high severity with no patch: open an issue, document the risk, and consider removing or replacing the dependency.
4. License violations require replacing the dependency — do not add non-approved licenses to `deny.toml` without maintainer discussion.

Supply-chain policy is defined in `deny.toml` at the repo root.

## Cross-References

| Document | Purpose |
|----------|---------|
| [AGENTS.md](AGENTS.md) | Repo overview, command surface, escalation rules |
| [TESTING.md](TESTING.md) | Verification matrix, CI tiers, how-to guides |
| [RELEASE.md](RELEASE.md) | Release process, version bumps, supply-chain checks |
| [docs/crate-map.md](docs/crate-map.md) | Every crate with tier, deps, responsibility |
| [docs/patterns/catalog.md](docs/patterns/catalog.md) | Pattern catalog including Human Review Gate |
| [docs/verification-matrix.md](docs/verification-matrix.md) | Per-crate verification techniques |
