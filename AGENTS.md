# AGENTS.md — faultline Contributor & Agent Onboarding

This is the primary entry point for agents and new contributors working in the faultline repository. Read this first, then follow links to deeper documents as needed.

## Purpose and Mission

faultline is a local-first regression archaeologist for Git repositories. Given a known-good commit, a known-bad commit, and a predicate (test/build command), it narrows the regression window and produces portable artifacts showing where to start investigating.

Core promise: walk history safely, run the operator's trusted predicate at candidate revisions, emit JSON + HTML artifacts explaining the narrowest credible regression window.

Design principles: honest over impressive, predicate-native, local-first, artifact-first, narrow core with thick edges.

See [docs/mission-and-vision.md](docs/mission-and-vision.md) for the full mission statement and [docs/non-goals.md](docs/non-goals.md) for what faultline is not.

## Architecture Overview

faultline follows a hexagonal (ports-and-adapters) architecture:

```text
faultline-cli (entry)
    ↓
faultline-app (orchestration)
    ↓                         ↓
  Domain                    Ports → Adapters
  ├─ faultline-codes          faultline-ports
  ├─ faultline-types            ├─ faultline-git
  ├─ faultline-localization     ├─ faultline-probe-exec
  └─ faultline-surface          ├─ faultline-store
                                ├─ faultline-render
                                ├─ faultline-sarif
                                └─ faultline-junit

Testing: faultline-fixtures
Tooling: xtask
```

Key invariant: domain logic lives in pure crates with no I/O. All infrastructure concerns are pushed to adapter crates behind port traits.

Full details: [docs/architecture.md](docs/architecture.md)

## Crate Map

Every workspace crate with its tier, dependencies, verification techniques, and responsibility is documented in the crate map.

See: [docs/crate-map.md](docs/crate-map.md)

Dependency direction rules:
- Adapters depend inward on ports and types
- Application depends on ports + domain
- Domain depends only on `faultline-codes` and `faultline-types`
- No domain crate imports infrastructure

## Scenario Atlas

Every test in the workspace is cataloged in a flat index with its problem statement, fixture, crate, artifact, invariant, and cross-references.

- Scenario index: [docs/scenarios/scenario_index.md](docs/scenarios/scenario_index.md)
- Behavior map (five-way cross-reference): [docs/scenarios/behavior_map.md](docs/scenarios/behavior_map.md)

When you add a test, you must add a corresponding entry to the scenario index. CI enforces this.

## Command Surface

All repo operations are available through `cargo xtask` subcommands, with ergonomic `just` aliases in the [Justfile](Justfile).

### CI Tiers

| Tier | Command | Just Alias | Trigger | Scope |
|------|---------|------------|---------|-------|
| Fast | `cargo xtask ci-fast` | `just ci` | Every push | fmt + clippy + test |
| Full | `cargo xtask ci-full` | `just ci-full` | Pull requests | ci-fast + golden + schema-check |
| Extended | `cargo xtask mutants` / `cargo xtask release-check` | `just mutants` / `just release-check` | Manual / release | mutation + fuzz + supply-chain |

### All Xtask Commands

| Command | Purpose |
|---------|---------|
| `cargo xtask ci-fast` | Run fmt + clippy + test (fast CI tier) |
| `cargo xtask ci-full` | Run ci-fast + golden + schema-check (full CI tier) |
| `cargo xtask smoke` | Build CLI and run against fixture repo |
| `cargo xtask golden` | Run and update golden/snapshot tests |
| `cargo xtask mutants` | Run cargo-mutants on configured surfaces |
| `cargo xtask fuzz --duration <secs>` | Run fuzz targets (default 60s) |
| `cargo xtask docs-check` | Build docs and check links |
| `cargo xtask release-check` | Run cargo-deny + cargo-audit + cargo-semver-checks |
| `cargo xtask scaffold crate <name> --tier <tier>` | Scaffold a new crate |
| `cargo xtask scaffold adr <title>` | Scaffold a new ADR |
| `cargo xtask scaffold scenario <name> --crate <crate>` | Scaffold a new test scenario |
| `cargo xtask scaffold doc <title> --section <section>` | Scaffold a new doc page |

Tool versions are pinned in [.mise.toml](.mise.toml).

### CLI Subcommands

The `faultline` binary exposes these subcommands for post-analysis workflows:

| Subcommand | Purpose |
|------------|---------|
| `faultline reproduce --run-dir <path> [--commit <sha>] [--shell]` | Extract a reproduction capsule from a completed run |
| `faultline diff-runs --left <path> --right <path> [--json] [--markdown]` | Compare two analysis runs side by side |
| `faultline export-markdown --run-dir <path> [--output <file>] [--compact]` | Export a run as a Markdown report |
| `faultline export-sarif --run-dir <path> [--output <file>]` | Export a run as SARIF v2.1.0 |
| `faultline export-junit --run-dir <path> [--output <file>]` | Export a run as JUnit XML |
| `faultline list-runs [--repo <path>] [--json]` | List all stored analysis runs |
| `faultline clean [--repo <path>] [--older-than-days <N>] [--all] [--dry-run]` | Remove old run artifacts |
| `faultline completions <shell>` | Emit shell completions (bash, zsh, fish, etc.) |

### CLI Flags

Top-level flags accepted by the main `faultline` command:

| Flag | Purpose |
|------|---------|
| `--json` | Emit machine-readable JSON output instead of human-friendly text |
| `--markdown` | Emit Markdown-formatted output |
| `--compact` | Use a condensed output layout (fewer blank lines, shorter tables) |
| `--retries <N>` | Number of times to retry a flaky predicate before recording a verdict |
| `--stability-threshold <N>` | Minimum consecutive consistent results before accepting a verdict |
| `--resume` | Resume an interrupted analysis run from its last checkpoint |
| `--force` | Overwrite existing run data without prompting |
| `--fresh` | Ignore cached results and re-run from scratch |
| `--no-render` | Skip HTML report generation after analysis |
| `--shell` | Drop into an interactive shell at the candidate revision (useful with `reproduce`) |
| `--env <KEY=VALUE>` | Inject an environment variable into the predicate execution environment |

## Artifact Contracts

faultline produces machine-readable artifacts with versioned contracts:

### JSON Schema

The `AnalysisReport` structure has a JSON Schema at `schemas/analysis-report.schema.json`, auto-generated from Rust types via `schemars`. CI detects schema drift — if you change `AnalysisReport` or its transitive types, regenerate the schema:

```bash
cargo xtask generate-schema
```

### Golden Tests

Snapshot tests (via `insta`) guard artifact stability:
- `analysis.json` output — in `faultline-render`
- `index.html` output — in `faultline-render`
- CLI `--help` text — in `faultline-cli`

When a golden test fails, review and accept changes:

```bash
cargo insta review
```

### Export Adapters

- SARIF v2.1.0 export: `faultline-sarif` crate (`to_sarif(&report)`)
- JUnit XML export: `faultline-junit` crate (`to_junit_xml(&report)`)

## Escalation Rules

1. Property test failures require human review. Do not suppress, skip, or weaken a failing property test without maintainer approval. See the Human Review Gate pattern in [docs/patterns/catalog.md](docs/patterns/catalog.md).
2. Schema-breaking changes require a `schema_version` bump in `AnalysisReport` and regeneration of `schemas/analysis-report.schema.json`.
3. Breaking public API changes require `cargo semver-checks` to pass or an explicit version bump with maintainer sign-off. See [RELEASE.md](RELEASE.md).
4. Supply-chain issues flagged by `cargo deny` or `cargo audit` must be resolved before merge. See [MAINTAINERS.md](MAINTAINERS.md) for the escalation path.

## Examples of Good Changes

### Adding a property test

1. Write the `proptest!` test in the appropriate crate (domain crates for pure logic, adapter crates for boundary behavior).
2. Use `ProptestConfig { cases: 100, .. }` minimum.
3. Add an entry to [docs/scenarios/scenario_index.md](docs/scenarios/scenario_index.md).
4. Run `cargo xtask ci-fast` to verify.

### Updating a golden artifact

1. Make your code change.
2. Run `cargo test` — the golden test will fail showing the diff.
3. Run `cargo insta review` to inspect and accept the new snapshot.
4. Commit the updated `.snap` file alongside your code change.

### Adding a fixture scenario

1. Use `faultline-fixtures` builders (`RevisionSequenceBuilder`, etc.) to construct test data.
2. Write the test in the target crate's `tests/` directory or inline `#[cfg(test)]` module.
3. Add an entry to [docs/scenarios/scenario_index.md](docs/scenarios/scenario_index.md).
4. Update [docs/scenarios/behavior_map.md](docs/scenarios/behavior_map.md) if the scenario maps to a requirement.

### Making a breaking type change with schema version bump

1. Update the type in `faultline-types`.
2. Bump `schema_version` in `AnalysisReport`.
3. Run `cargo xtask generate-schema` to regenerate the JSON Schema.
4. Run `cargo insta review` to accept updated golden snapshots.
5. Update export adapters (`faultline-sarif`, `faultline-junit`) if the change affects serialized output.
6. Run `cargo xtask ci-full` to verify all contracts pass.

### Adding a new crate

1. Run `cargo xtask scaffold crate faultline-<name> --tier <domain|adapter|app>`.
2. Implement the crate's logic.
3. Add the crate to [docs/crate-map.md](docs/crate-map.md) and [docs/verification-matrix.md](docs/verification-matrix.md).
4. Add tests and corresponding entries to the scenario atlas.
5. Run `cargo xtask ci-fast` to verify workspace compilation.

## Cross-References

| Document | Location | Purpose |
|----------|----------|---------|
| Testing guide | [TESTING.md](TESTING.md) | Verification matrix, CI tiers, how-to guides |
| Release process | [RELEASE.md](RELEASE.md) | Version bumps, changelog, tag creation |
| Maintainers | [MAINTAINERS.md](MAINTAINERS.md) | Code ownership, review gates, escalation |
| Crate Map | [docs/crate-map.md](docs/crate-map.md) | Every crate with tier, deps, responsibility |
| Scenario Atlas | [docs/scenarios/scenario_index.md](docs/scenarios/scenario_index.md) | Every test indexed by behavior |
| Behavior Map | [docs/scenarios/behavior_map.md](docs/scenarios/behavior_map.md) | Requirement → test → fixture → artifact |
| Pattern Catalog | [docs/patterns/catalog.md](docs/patterns/catalog.md) | 10 named patterns governing the repo |
| Verification Matrix | [docs/verification-matrix.md](docs/verification-matrix.md) | Per-crate verification techniques |
| Architecture | [docs/architecture.md](docs/architecture.md) | Hexagonal architecture and crate boundaries |
| Architecture Handbook | [docs/handbook/README.md](docs/handbook/README.md) | Entry point for architecture understanding |
