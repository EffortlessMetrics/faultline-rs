# Requirements Document

## Introduction

This document captures the requirements for the faultline repo operating system layer — the infrastructure that turns a hardened v0.1 codebase into a delegation-ready, self-teaching system. The v0.1 scaffold (12 crates, hexagonal architecture, working localization engine, persistence, CLI, renderer) and the v01-hardening pass (frozen contract, proven core, real fixtures, safe resumability, hardened adapters, polished operator surface) are complete. This layer adds six reinforcing capabilities: a shared pattern language, a scenario atlas for behavior-indexed discovery, a standard evidence bus with artifact contracts, an agent-facing teaching layer, a reference template ecosystem, and contract-aware CI feedback.

The repo operating system is not a new product feature — it is the meta-infrastructure that makes the repo behavior-indexed by scenarios, audited by mutation, generalized by properties, hardened by fuzzing, stabilized by artifacts, explained by Diátaxis docs, and operated through deterministic local commands. Every requirement below targets the repo itself as the system under change.

Key observations about the current state that motivate these requirements:
- The repo has no formal pattern catalog — internal conventions (truth core / translation edge, artifact-first boundary, proof-carrying change) exist implicitly but are not documented or discoverable
- BDD scenarios exist as unit tests and property tests but are not indexed, cross-referenced, or discoverable as a semantic map
- Artifacts (`analysis.json`, `index.html`) have no JSON Schema, no versioned contract, and no export adapters for SARIF or JUnit XML
- No agent-facing documentation exists (no AGENTS.md, TESTING.md, RELEASE.md, or crate map)
- No `cargo xtask` command surface exists — repo rituals are ad-hoc `cargo` invocations
- No mutation testing, fuzz testing, or golden artifact discipline is wired
- CI runs build/test/fmt/clippy but has no contract-aware checks (schema drift, missing scenarios, golden staleness)
- No Diátaxis-structured documentation site exists
- No `cargo-generate` templates or scaffold packs exist for the repo style

## Glossary

- **Repo_Operating_System**: The collective meta-infrastructure (patterns, scenarios, artifacts, docs, commands, CI checks) that makes the faultline repository delegation-ready and self-teaching.
- **Pattern_Catalog**: A documented collection of named internal patterns with definitions, examples, anti-examples, and cross-references to ADRs and scenarios.
- **Scenario_Atlas**: A first-class, discoverable index of BDD and property-test scenarios, cross-referenced to requirements, ADRs, fixtures, crates, and artifacts.
- **Evidence_Bus**: The standardized internal artifact model with JSON Schema, versioning, deterministic ordering, and export adapters for external consumption formats.
- **Artifact_Contract**: A versioned schema and golden-test discipline for each machine-readable output (JSON reports, HTML reports, CLI help text, terminal summaries).
- **Teaching_Layer**: Agent-facing and contributor-facing documentation (AGENTS.md, TESTING.md, RELEASE.md, MAINTAINERS.md, crate map) that enables delegation without oral tradition.
- **Xtask**: A Rust-native `cargo xtask` binary that serves as the control plane for repo rituals (CI tiers, golden updates, mutation runs, fuzz targets, doc checks, release checks).
- **Justfile**: An ergonomic alias layer (via `just`) that wraps `cargo xtask` commands for quick invocation.
- **Golden_Test**: A snapshot or golden-file test (via `insta`) that compares current output against a committed reference artifact, failing on unexpected drift.
- **Mutation_Testing**: Automated source-code mutation (via `cargo-mutants`) that verifies the test suite detects injected faults.
- **Fuzz_Target**: A `cargo-fuzz` or `libfuzzer` harness that exercises parser, store, probe, or render boundaries with random inputs.
- **Verification_Matrix**: A per-crate-tier mapping of which verification techniques (properties, BDD, fuzz, mutation, smoke, golden) apply to each crate.
- **Diataxis_Site**: A documentation site structured according to the Diátaxis framework (tutorials, how-to guides, explanations, reference) built with mdBook.
- **CI_Tier**: A named CI workflow stage (ci-fast, ci-full, golden, mutants, fuzz, smoke, release-check) with defined scope and feedback semantics.
- **Contract_Check**: A CI step that validates a specific repo contract (schema compatibility, golden freshness, scenario coverage, doc completeness) and reports which contract was broken.
- **ADR**: Architecture Decision Record, a lightweight document capturing a design decision, its context, and consequences.
- **Crate_Map**: A machine-readable and human-readable index of all workspace crates with their tier (domain, ports, adapters, app, entry, testing), dependencies, and applicable verification techniques.
- **Scaffold_Pack**: A `cargo-generate` template or `xtask init` subcommand that generates boilerplate for a new crate, ADR, scenario, or doc page in the faultline style.

## Requirements

### Requirement 1: Pattern Language and Shared Vocabulary

**User Story:** As a contributor or delegated agent, I want a documented catalog of named internal patterns with definitions, examples, and anti-examples, so that I can apply the repo's conventions without oral tradition.

#### Acceptance Criteria

1. THE Repo_Operating_System SHALL include a `docs/patterns/catalog.md` file containing at least the following named patterns: Truth Core / Translation Edge, Scenario Atlas, Artifact-First Boundary, Proof-Carrying Change, Replayable Run Memory, Operator Dossier, Human Review Gate After Failing Property, Mutation-On-Diff, Golden Artifact Contract, and Delegation-Safe Crate Seam.
2. WHEN a pattern is documented in the Pattern_Catalog, THE entry SHALL include: a one-sentence definition, a "when to use" section, a concrete example from the faultline codebase, at least one anti-example showing misuse, and cross-references to related ADRs and scenarios.
3. THE Repo_Operating_System SHALL include an ADR template at `docs/adr/TEMPLATE.md` that follows the existing ADR format (Status, Context, Decision, Consequences) and adds a "Related Patterns" section.
4. THE Pattern_Catalog SHALL include a glossary section that defines all pattern names used in the catalog, consistent with the Glossary in this requirements document.
5. WHEN a new ADR is created, THE ADR SHALL reference applicable patterns from the Pattern_Catalog in its "Related Patterns" section.
6. THE Repo_Operating_System SHALL include an architecture handbook entry at `docs/handbook/README.md` that links to the Pattern_Catalog, ADR index, Scenario_Atlas, and Crate_Map.

### Requirement 2: Scenario Atlas — Behavior-Indexed Discovery

**User Story:** As a contributor, I want a discoverable map of all test scenarios cross-referenced to requirements, ADRs, fixtures, crates, and artifacts, so that I can find the right test for any behavior question.

#### Acceptance Criteria

1. THE Repo_Operating_System SHALL include a `docs/scenarios/scenario_index.md` file that lists every BDD scenario and property test in the workspace.
2. WHEN a scenario is listed in the Scenario_Atlas, THE entry SHALL include: scenario name, one-sentence problem description, fixture or generator it uses, crate(s) it exercises, artifact(s) it produces or validates, invariant or property it asserts, and related ADR or doc references.
3. THE Scenario_Atlas SHALL organize scenarios by crate tier: domain scenarios, adapter scenarios, app-integration scenarios, CLI smoke scenarios, and cross-cutting property tests.
4. THE Repo_Operating_System SHALL include a `docs/scenarios/behavior_map.md` file that provides a five-way cross-reference: requirement → ADR/explanation → BDD scenario → fixture/harness → artifact/output.
5. THE Scenario_Atlas SHALL be verifiable: a CI check SHALL detect when a test file exists in the workspace that is not listed in the scenario index, or when a scenario index entry references a test that does not exist.
6. WHEN a new test scenario is added to the workspace, THE contributor SHALL add a corresponding entry to the Scenario_Atlas, and THE CI check from criterion 2.5 SHALL enforce this.
7. THE Scenario_Atlas SHALL document the testing framework choice for each scenario tier: `proptest` for domain property tests, unit tests with `#[test]` for fixture scenarios, integration tests for app-level flows, and CLI smoke tests via `std::process::Command`.

### Requirement 3: Standard Evidence Bus and Artifact Contracts

**User Story:** As a maintainer, I want a stable, versioned artifact model with JSON Schema, golden tests, and export adapters, so that artifacts are machine-usable, drift is caught, and external tools can consume faultline output.

#### Acceptance Criteria

1. THE Repo_Operating_System SHALL include a JSON Schema file at `schemas/analysis-report.schema.json` that describes the `AnalysisReport` structure, generated from the Rust types using `schemars`.
2. THE JSON Schema SHALL include a `$schema` draft identifier, a `title`, a `description`, and a `version` field matching the `schema_version` in `AnalysisReport`.
3. WHEN the `AnalysisReport` structure changes, THE JSON Schema SHALL be regenerated and THE schema version SHALL be reviewed for compatibility.
4. THE Repo_Operating_System SHALL include golden tests (via `insta`) for: the `analysis.json` output of a canonical fixture scenario, the `index.html` output of a canonical fixture scenario, and the CLI `--help` text.
5. WHEN a golden test fails, THE CI SHALL report which artifact contract was broken and provide instructions for reviewing and accepting the change (`cargo insta review`).
6. THE Repo_Operating_System SHALL include a SARIF export adapter that converts an `AnalysisReport` into a valid SARIF v2.1.0 document, so that faultline results can be consumed by GitHub Code Scanning and other SARIF-compatible tools.
7. THE Repo_Operating_System SHALL include a JUnit XML export adapter that converts an `AnalysisReport` into a valid JUnit XML document, so that faultline results can be consumed by CI dashboards that understand JUnit format.
8. THE Evidence_Bus SHALL use deterministic ordering for all collection fields in serialized artifacts (observations by sequence_index, changed_paths by path, buckets by name).
9. THE Repo_Operating_System SHALL add `schemars` as a workspace dependency and derive `JsonSchema` on all types in `faultline-types` that appear in `AnalysisReport`.

### Requirement 4: Agent-Facing Teaching Layer

**User Story:** As a delegated agent or new contributor, I want structured onboarding documents that explain how to navigate, test, change, and release the repo, so that I can contribute without requiring synchronous guidance.

#### Acceptance Criteria

1. THE Repo_Operating_System SHALL include an `AGENTS.md` file at the repository root that describes: the repo's purpose, architecture overview, crate map with dependency direction, scenario atlas location, command surface (xtask commands by tier), artifact contracts, and escalation rules.
2. THE Repo_Operating_System SHALL include a `TESTING.md` file at the repository root that describes: the verification matrix by crate tier, how to run each CI tier locally, how to add a new property test, how to add a new fixture scenario, how to update golden artifacts, and how to run mutation and fuzz tests.
3. THE Repo_Operating_System SHALL include a `RELEASE.md` file at the repository root that describes: the release process, version bumping, changelog generation, tag creation, and binary distribution.
4. THE Repo_Operating_System SHALL include a `MAINTAINERS.md` file at the repository root that describes: code ownership by crate, review expectations, the human review gate for property-test failures, and the escalation path for breaking changes.
5. THE Repo_Operating_System SHALL include a Crate_Map at `docs/crate-map.md` that lists every workspace crate with: name, tier (domain/ports/adapters/app/entry/testing), direct dependencies, applicable verification techniques, and a one-sentence responsibility summary.
6. THE `AGENTS.md` SHALL include examples of good changes: adding a property test, updating a golden artifact, adding a fixture scenario, making a breaking type change with schema version bump, and adding a new crate.
7. THE Teaching_Layer documents SHALL be cross-referenced: `AGENTS.md` links to `TESTING.md`, `RELEASE.md`, `MAINTAINERS.md`, the Crate_Map, the Scenario_Atlas, and the Pattern_Catalog.

### Requirement 5: DevEx Command Surface — Xtask and Justfile

**User Story:** As a contributor, I want a Rust-native `cargo xtask` control plane and ergonomic `just` aliases for all repo rituals, so that every operation is a deterministic, discoverable command.

#### Acceptance Criteria

1. THE Repo_Operating_System SHALL include an `xtask` crate at `crates/xtask/` that is a Cargo workspace member and provides a `cargo xtask` binary.
2. THE Xtask binary SHALL support the following subcommands: `ci-fast` (fmt + clippy + test), `ci-full` (ci-fast + golden + schema-check), `smoke` (build CLI + run against fixture repo), `golden` (run and update golden/snapshot tests), `mutants` (run cargo-mutants on configured crate surfaces), `fuzz` (run fuzz targets for configured duration), `docs-check` (build docs + link check), and `release-check` (semver-checks + deny + audit).
3. WHEN `cargo xtask ci-fast` is run, THE Xtask SHALL execute `cargo fmt --check`, `cargo clippy --workspace -- -D warnings`, and `cargo test --workspace` in sequence, stopping on first failure.
4. THE Repo_Operating_System SHALL include a `Justfile` at the repository root that provides short aliases for each xtask subcommand (e.g., `just ci` → `cargo xtask ci-fast`, `just golden` → `cargo xtask golden`).
5. THE Xtask binary SHALL print a help message listing all subcommands with one-sentence descriptions when invoked without arguments or with `--help`.
6. THE Repo_Operating_System SHALL include a `.mise.toml` or `.tool-versions` file at the repository root that pins the Rust toolchain version and any required tool versions (cargo-mutants, cargo-fuzz, cargo-insta, just, bacon, cargo-nextest).
7. WHEN a required tool is not installed, THE Xtask subcommand that needs the tool SHALL print a clear error message naming the missing tool and providing an install command.
8. THE Repo_Operating_System SHALL configure `cargo-nextest` as the default test runner in the Xtask `ci-fast` and `ci-full` subcommands, falling back to `cargo test` if nextest is not installed.

### Requirement 6: Verification Matrix and Advanced Testing

**User Story:** As a maintainer, I want a per-crate verification matrix that maps domain crates to property tests, adapter crates to BDD + fuzz, app crates to scenario + mutation, and CLI to smoke + golden, so that every crate tier has appropriate proof coverage.

#### Acceptance Criteria

1. THE Repo_Operating_System SHALL include a verification matrix at `docs/verification-matrix.md` that maps each workspace crate to its applicable verification techniques.
2. THE Verification_Matrix SHALL assign techniques by crate tier: domain crates (codes, types, localization, surface) get property tests; adapter crates (git, probe-exec, store, render) get BDD scenarios, fuzz targets, and golden tests; the app crate gets integration scenarios and mutation testing; the CLI crate gets smoke tests and golden tests for help text.
3. THE Repo_Operating_System SHALL include at least one `cargo-mutants` configuration that targets the `faultline-localization` crate's core narrowing and outcome logic, verifiable via `cargo xtask mutants`.
4. THE Repo_Operating_System SHALL include at least one `cargo-fuzz` target that exercises the `AnalysisReport` JSON deserialization path in `faultline-types`, verifiable via `cargo xtask fuzz`.
5. THE Repo_Operating_System SHALL include `insta` as a workspace dev-dependency and use it for golden tests of `analysis.json`, `index.html`, and CLI `--help` output.
6. WHEN `cargo xtask mutants` is run, THE Xtask SHALL invoke `cargo-mutants` with the configured crate targets and report surviving mutants as failures.
7. WHEN `cargo xtask fuzz` is run, THE Xtask SHALL invoke `cargo-fuzz` with the configured targets for a default duration of 60 seconds, configurable via a `--duration` flag.
8. THE Verification_Matrix SHALL document the minimum property-test iteration count (100) and the mutation testing budget (time or mutant count) for each applicable crate.

### Requirement 7: Diátaxis Documentation Site

**User Story:** As a contributor or user, I want a structured documentation site following the Diátaxis framework, so that I can find tutorials, how-to guides, explanations, and reference material in predictable locations.

#### Acceptance Criteria

1. THE Repo_Operating_System SHALL include an mdBook configuration at `docs/book/book.toml` that builds a documentation site from Markdown sources under `docs/book/src/`.
2. THE Diataxis_Site SHALL organize content into four sections following the Diátaxis framework: Tutorials (learning-oriented), How-To Guides (task-oriented), Explanations (understanding-oriented), and Reference (information-oriented).
3. THE Diataxis_Site SHALL include at least one tutorial: "Your First Faultline Run" that walks through installing, running against a sample repo, and reading the output artifacts.
4. THE Diataxis_Site SHALL include at least one how-to guide: "Adding a New Property Test" that walks through creating a proptest, wiring it to a requirement, and updating the Scenario_Atlas.
5. THE Diataxis_Site SHALL include at least one explanation: "How Localization Works" that explains the binary narrowing algorithm, outcome classification, and confidence scoring.
6. THE Diataxis_Site SHALL include reference pages for: the CLI flag reference, the artifact schema reference (linking to the JSON Schema), the predicate contract, and the exit code table.
7. WHEN `cargo xtask docs-check` is run, THE Xtask SHALL build the mdBook site and verify that all internal links resolve, reporting broken links as failures.
8. THE Diataxis_Site SHALL link to the Pattern_Catalog, Scenario_Atlas, Crate_Map, and ADR index from its navigation sidebar.

### Requirement 8: Contract-Aware CI Feedback

**User Story:** As a maintainer, I want CI checks that detect broken contracts (schema drift, missing scenarios, stale goldens, undocumented changes) and report which contract was broken, so that contributors get actionable feedback instead of generic test failures.

#### Acceptance Criteria

1. THE Repo_Operating_System SHALL include a CI workflow that runs `cargo xtask ci-full` on every pull request, covering fmt, clippy, tests, golden checks, and schema compatibility checks.
2. WHEN a golden test fails in CI, THE CI output SHALL include the artifact name, the expected vs. actual diff, and the command to accept the change (`cargo insta review`).
3. WHEN the `AnalysisReport` JSON Schema is out of date relative to the Rust types, THE CI SHALL fail with a message stating "schema drift detected: regenerate schemas/analysis-report.schema.json".
4. WHEN a test file exists in the workspace that is not listed in the Scenario_Atlas, THE CI SHALL fail with a message stating "missing scenario index entry for {test_file}".
5. WHEN a semantic surface (localization outcome logic, artifact serialization, CLI flag set) is changed without updating the corresponding golden test, THE CI SHALL fail with a message identifying the stale golden artifact.
6. THE CI workflow SHALL be organized into tiers: `ci-fast` (runs on every push, completes in under 5 minutes), `ci-full` (runs on PRs, includes golden and schema checks), and optional `ci-extended` (mutation, fuzz, triggered manually or on release branches).
7. WHEN a CI check fails, THE failure message SHALL state which contract was broken and link to the relevant documentation in the Teaching_Layer for remediation.
8. THE Repo_Operating_System SHALL include a `cargo xtask ci-fast` GitHub Actions workflow at `.github/workflows/ci.yml` that replaces or extends the existing CI configuration.

### Requirement 9: Release and Supply-Chain Governance

**User Story:** As a maintainer, I want automated release tooling, supply-chain auditing, and semver compatibility checks, so that releases are safe, reproducible, and API-compatible.

#### Acceptance Criteria

1. THE Repo_Operating_System SHALL configure `cargo-deny` with a `deny.toml` at the repository root that checks for: duplicate dependencies, banned licenses, known advisories, and yanked crates.
2. WHEN `cargo xtask release-check` is run, THE Xtask SHALL execute `cargo deny check`, `cargo audit`, and `cargo semver-checks` in sequence, reporting all findings.
3. THE Repo_Operating_System SHALL include a `cargo-semver-checks` configuration that verifies public API compatibility for all crates that export public types (at minimum: `faultline-types`, `faultline-codes`, `faultline-ports`).
4. THE `RELEASE.md` SHALL document the release process: version bump via workspace Cargo.toml, changelog update, `cargo xtask release-check`, tag creation, and binary distribution decision.
5. THE Repo_Operating_System SHALL include a `.github/workflows/release.yml` workflow that triggers on tag push and runs `cargo xtask release-check` before building release artifacts.
6. WHEN `cargo deny check` finds a banned license or known advisory, THE CI SHALL fail with a message identifying the problematic dependency and the reason for rejection.

### Requirement 10: Reference Templates and Scaffold Packs

**User Story:** As a contributor, I want scaffold commands and templates for common repo operations (new crate, new ADR, new scenario, new doc page), so that new additions follow the established patterns without manual boilerplate.

#### Acceptance Criteria

1. THE Xtask binary SHALL support a `scaffold` subcommand with sub-subcommands: `scaffold crate <name> --tier <domain|adapter|app>`, `scaffold adr <title>`, `scaffold scenario <name> --crate <crate>`, and `scaffold doc <title> --section <tutorial|howto|explanation|reference>`.
2. WHEN `cargo xtask scaffold crate <name> --tier domain` is run, THE Xtask SHALL generate a new crate directory under `crates/` with: `Cargo.toml` inheriting workspace metadata, `src/lib.rs` with a module doc comment, and an entry in the workspace `Cargo.toml` members list.
3. WHEN `cargo xtask scaffold adr <title>` is run, THE Xtask SHALL generate a new ADR file under `docs/adr/` with the next sequential number, using the ADR template from Requirement 1.3, and pre-filled title.
4. WHEN `cargo xtask scaffold scenario <name> --crate <crate>` is run, THE Xtask SHALL generate a test file stub in the target crate's `tests/` directory and add a placeholder entry to the Scenario_Atlas.
5. WHEN `cargo xtask scaffold doc <title> --section tutorial` is run, THE Xtask SHALL generate a new Markdown file in the appropriate Diátaxis section under `docs/book/src/` and add it to the mdBook `SUMMARY.md`.
6. THE scaffold subcommands SHALL validate inputs: crate names must be valid Rust identifiers prefixed with `faultline-`, ADR titles must be non-empty, scenario names must be non-empty, and doc sections must be one of the four Diátaxis categories.
