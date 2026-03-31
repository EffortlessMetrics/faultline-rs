# Implementation Plan: faultline Repo Operating System

## Overview

This plan implements the meta-infrastructure layer that turns the hardened v0.1 faultline codebase into a delegation-ready, self-teaching system. Tasks are organized into 10 dependency-ordered waves. The workspace must compile after each wave. All code is Rust (stable, edition 2021).

## Tasks

- [x] 1. Wave 1 — Foundation: xtask crate skeleton, workspace deps, mise.toml, Justfile
  - [x] 1.1 Create `crates/xtask/Cargo.toml` and `crates/xtask/src/main.rs` with clap-derived CLI skeleton
    - Define `Cli` struct with `#[derive(Parser)]` and `Command` enum with all subcommands: `ci-fast`, `ci-full`, `smoke`, `golden`, `mutants`, `fuzz`, `docs-check`, `release-check`, `scaffold`
    - Define `ScaffoldKind` sub-enum with `crate`, `adr`, `scenario`, `doc` variants
    - Each subcommand handler prints `"not yet implemented"` as a placeholder
    - Add `crates/xtask` to workspace `Cargo.toml` members list
    - Add `anyhow` as a workspace dependency for xtask error handling
    - _Requirements: 5.1, 5.2, 5.5_

  - [x] 1.2 Add workspace dependencies for new crates
    - Add `schemars = "0.8"` to `[workspace.dependencies]`
    - Add `insta = { version = "1.40", features = ["json"] }` to `[workspace.dependencies]`
    - Add `quick-xml = "0.37"` to `[workspace.dependencies]`
    - Add `anyhow = "1.0"` to `[workspace.dependencies]`
    - _Requirements: 3.9, 6.5_

  - [x] 1.3 Create `.mise.toml` at repo root
    - Pin `rust = "stable"` and `just = "latest"`
    - Set `CARGO_TERM_COLOR = "always"` in `[env]`
    - _Requirements: 5.6_

  - [x] 1.4 Create `Justfile` at repo root
    - Add aliases for all xtask subcommands: `ci`, `ci-fast`, `ci-full`, `smoke`, `golden`, `mutants`, `fuzz`, `docs`, `release-check`, `scaffold`
    - Include `default` recipe that runs `just --list`
    - _Requirements: 5.4_

  - [x] 1.5 Write unit test for xtask help completeness
    - **Property 43: Xtask Help Completeness**
    - Invoke `Cli::command()` with `--help`, verify all subcommand names present: `ci-fast`, `ci-full`, `smoke`, `golden`, `mutants`, `fuzz`, `docs-check`, `release-check`, `scaffold`
    - Verify `scaffold --help` lists `crate`, `adr`, `scenario`, `doc`
    - **Validates: Requirements 5.2, 5.5, 10.1**

- [x] 2. Checkpoint — Wave 1 complete
  - Ensure `cargo build --workspace` succeeds, `cargo xtask --help` prints all subcommands. Ask the user if questions arise.

- [x] 3. Wave 2 — Schema & Evidence: schemars derive, JSON Schema generation, golden tests via insta
  - [x] 3.1 Add `schemars` derive to `faultline-types`
    - Add `schemars = { workspace = true }` to `faultline-types/Cargo.toml` dependencies
    - Derive `JsonSchema` on all types transitively referenced by `AnalysisReport`: `AnalysisReport`, `AnalysisRequest`, `RevisionSequence`, `ProbeObservation`, `LocalizationOutcome`, `Confidence`, `PathChange`, `SurfaceSummary`, `SubsystemBucket`, `CommitId`, `RevisionSpec`, `HistoryMode`, `ProbeSpec`, `SearchPolicy`, `ShellKind`, `ChangeStatus`, `ProbeKind`, `ObservationClass`, `AmbiguityReason`
    - For types in `faultline-codes`, add `schemars` dependency to `faultline-codes/Cargo.toml` and derive `JsonSchema` on `ObservationClass`, `ProbeKind`, `AmbiguityReason`, `OperatorCode`
    - _Requirements: 3.1, 3.9_

  - [x] 3.2 Create `schemas/` directory and implement schema generation in xtask
    - Create `crates/xtask/src/schema.rs` with `generate_schema()` and `check_schema()` functions
    - `generate_schema()` uses `schemars::schema_for!(AnalysisReport)` and writes to `schemas/analysis-report.schema.json`
    - `check_schema()` compares current file against freshly generated schema, fails with `"schema drift detected"` message
    - Wire `ci-full` subcommand to call `check_schema()`
    - Add `faultline-types` as a dependency of `crates/xtask`
    - Run `generate_schema()` to create the initial `schemas/analysis-report.schema.json`
    - _Requirements: 3.1, 3.2, 3.3, 8.3_

  - [x] 3.3 Add `insta` golden tests for `analysis.json` and `index.html` in `faultline-render`
    - Add `insta = { workspace = true }` to `faultline-render/Cargo.toml` `[dev-dependencies]`
    - Create a `canonical_fixture_report()` helper that builds a deterministic `AnalysisReport`
    - Write `golden_analysis_json` test: serialize report to JSON, `insta::assert_snapshot!`
    - Write `golden_index_html` test: render HTML, `insta::assert_snapshot!`
    - Run `cargo insta test --accept` to generate initial snapshots
    - _Requirements: 3.4, 6.5_

  - [x] 3.4 Add `insta` golden test for CLI `--help` in `faultline-cli`
    - Add `insta = { workspace = true }` to `faultline-cli/Cargo.toml` `[dev-dependencies]`
    - Write `golden_cli_help` test: capture `Cli::command().write_long_help()`, `insta::assert_snapshot!`
    - Run `cargo insta test --accept` to generate initial snapshot
    - _Requirements: 3.4_

  - [x] 3.5 Write property test for JSON Schema validation of reports
    - **Property 40: JSON Schema Validates All Valid Reports**
    - Reuse existing `arb_analysis_report()` generator from `faultline-types`
    - Serialize report to JSON, validate against generated schema using `jsonschema` crate or manual structural checks
    - Verify schema contains `$schema` draft identifier and `title` field
    - **Validates: Requirements 3.1, 3.2**

  - [x] 3.6 Write property test for schema drift detection
    - **Property 45: Schema Drift Detection**
    - Generate schema, write a modified version to a temp file, verify `check_schema()` returns error containing `"schema drift detected"`
    - **Validates: Requirements 8.3**

- [x] 4. Checkpoint — Wave 2 complete
  - Ensure `cargo test --workspace` passes, `schemas/analysis-report.schema.json` exists and is valid JSON, golden snapshots are committed. Ask the user if questions arise.

- [x] 5. Wave 3 — Export Adapters: faultline-sarif, faultline-junit crates
  - [x] 5.1 Create `crates/faultline-sarif/` crate
    - Create `Cargo.toml` with dependencies on `faultline-types`, `serde`, `serde_json` (all workspace)
    - Implement `to_sarif(report: &AnalysisReport) -> Result<String, serde_json::Error>`
    - Map `AnalysisReport` → SARIF v2.1.0 structure: one `run`, tool name `"faultline"`, outcome → `result` with appropriate `level` (`error`/`warning`/`note`), `changed_paths` → `locations`
    - Define internal SARIF structs (`SarifLog`, `SarifRun`, `SarifTool`, `SarifResult`, `SarifLocation`) with `Serialize`
    - Add `crates/faultline-sarif` to workspace members
    - _Requirements: 3.6_

  - [x] 5.2 Create `crates/faultline-junit/` crate
    - Create `Cargo.toml` with dependencies on `faultline-types` (workspace) and `quick-xml` (workspace)
    - Implement `to_junit_xml(report: &AnalysisReport) -> String`
    - Map: one `<testsuite name="faultline">`, one `<testcase>`, `FirstBad`/`SuspectWindow` → `<failure>`, observations in `<system-out>`
    - Add `crates/faultline-junit` to workspace members
    - _Requirements: 3.7_

  - [x] 5.3 Write property test for SARIF export structural validity
    - **Property 41: SARIF Export Structural Validity**
    - Reuse `arb_analysis_report()`, call `to_sarif()`, verify: valid JSON, `version == "2.1.0"`, `$schema` present, tool name `"faultline"`, result level matches outcome type
    - **Validates: Requirements 3.6**

  - [x] 5.4 Write property test for JUnit XML export structural validity
    - **Property 42: JUnit XML Export Structural Validity**
    - Reuse `arb_analysis_report()`, call `to_junit_xml()`, verify: well-formed XML, `<testsuites>` root, `<testsuite name="faultline">`, `<testcase>` present, `<failure>` for non-Inconclusive
    - **Validates: Requirements 3.7**

- [x] 6. Checkpoint — Wave 3 complete
  - Ensure `cargo test --workspace` passes, both export adapter crates compile and their tests pass. Ask the user if questions arise.

- [x] 7. Wave 4 — Documentation Layer: pattern catalog, scenario atlas, verification matrix, crate map, ADR template, handbook
  - [x] 7.1 Create `docs/patterns/catalog.md` with 10 named patterns
    - Each pattern entry includes: one-sentence definition, "when to use" section, concrete faultline example, at least one anti-example, cross-references to related ADRs and scenarios
    - Patterns: Truth Core / Translation Edge, Scenario Atlas, Artifact-First Boundary, Proof-Carrying Change, Replayable Run Memory, Operator Dossier, Human Review Gate After Failing Property, Mutation-On-Diff, Golden Artifact Contract, Delegation-Safe Crate Seam
    - Include a glossary section at the end consistent with the requirements glossary
    - _Requirements: 1.1, 1.2, 1.4_

  - [x] 7.2 Create `docs/adr/TEMPLATE.md`
    - Follow existing ADR format: Status, Context, Decision, Consequences
    - Add "Related Patterns" section referencing `docs/patterns/catalog.md`
    - _Requirements: 1.3, 1.5_

  - [x] 7.3 Create `docs/scenarios/scenario_index.md`
    - List every BDD scenario and property test in the workspace
    - Each entry: scenario name, problem description, fixture/generator, crate(s), artifact(s), invariant/property, related references
    - Organize by crate tier: domain, adapter, app-integration, CLI smoke, cross-cutting property tests
    - _Requirements: 2.1, 2.2, 2.3_

  - [x] 7.4 Create `docs/scenarios/behavior_map.md`
    - Five-way cross-reference: requirement → ADR/explanation → BDD scenario → fixture/harness → artifact/output
    - Cover all 10 requirements from this spec plus key requirements from prior specs
    - _Requirements: 2.4_

  - [x] 7.5 Create `docs/verification-matrix.md`
    - Map each workspace crate to applicable verification techniques (property, BDD/unit, golden, fuzz, mutation, smoke)
    - Document minimum property-test iteration count (100) and mutation testing budget
    - _Requirements: 6.1, 6.2, 6.8_

  - [x] 7.6 Create `docs/crate-map.md`
    - List every workspace crate with: name, tier, direct dependencies, applicable verification techniques, one-sentence responsibility
    - Include the new crates: `faultline-sarif`, `faultline-junit`, `xtask`
    - _Requirements: 4.5_

  - [x] 7.7 Create `docs/handbook/README.md`
    - Architecture handbook entry linking to Pattern Catalog, ADR index, Scenario Atlas, and Crate Map
    - _Requirements: 1.6_

  - [x] 7.8 Write property test for pattern entry structural completeness
    - **Property 37: Pattern Entry Structural Completeness**
    - Parse `docs/patterns/catalog.md`, verify each pattern has all five required sections: definition, "when to use", example, anti-example, cross-references
    - **Validates: Requirements 1.2**

  - [x] 7.9 Write property test for scenario entry structural completeness
    - **Property 38: Scenario Entry Structural Completeness**
    - Parse `docs/scenarios/scenario_index.md`, verify each entry has all seven required fields
    - **Validates: Requirements 2.2**

- [x] 8. Checkpoint — Wave 4 complete
  - Ensure `cargo test --workspace` passes, all documentation files exist and are well-formed. Ask the user if questions arise.

- [x] 9. Wave 5 — Teaching Layer: AGENTS.md, TESTING.md, RELEASE.md, MAINTAINERS.md
  - [x] 9.1 Create `AGENTS.md` at repo root
    - Sections: purpose/mission, architecture overview (link to `docs/architecture.md`), crate map (link to `docs/crate-map.md`), scenario atlas location, command surface (xtask commands by tier), artifact contracts (JSON schema, golden tests), escalation rules
    - Include examples of good changes: adding a property test, updating a golden artifact, adding a fixture scenario, making a breaking type change with schema version bump, adding a new crate
    - Cross-reference: link to `TESTING.md`, `RELEASE.md`, `MAINTAINERS.md`, Crate Map, Scenario Atlas, Pattern Catalog
    - _Requirements: 4.1, 4.6, 4.7_

  - [x] 9.2 Create `TESTING.md` at repo root
    - Sections: verification matrix by crate tier, how to run each CI tier locally (`cargo xtask ci-fast`, `cargo xtask ci-full`), how to add a new property test, how to add a new fixture scenario, how to update golden artifacts (`cargo insta review`), how to run mutation and fuzz tests
    - _Requirements: 4.2_

  - [x] 9.3 Create `RELEASE.md` at repo root
    - Sections: version bump via workspace `Cargo.toml`, changelog update, `cargo xtask release-check`, tag creation, binary distribution decision
    - _Requirements: 4.3, 9.4_

  - [x] 9.4 Create `MAINTAINERS.md` at repo root
    - Sections: code ownership by crate, review expectations, human review gate for property-test failures, escalation path for breaking changes
    - _Requirements: 4.4_

- [x] 10. Checkpoint — Wave 5 complete
  - Ensure all four teaching layer files exist at repo root and are cross-referenced. Ask the user if questions arise.

- [x] 11. Wave 6 — Diátaxis Site: mdBook setup, tutorials, how-to, explanations, reference
  - [x] 11.1 Create mdBook configuration and directory structure
    - Create `docs/book/book.toml` with title `"faultline"`, language `"en"`, light default theme
    - Create `docs/book/src/SUMMARY.md` with four Diátaxis sections and sidebar links to Pattern Catalog, Scenario Atlas, Crate Map, ADR index
    - _Requirements: 7.1, 7.2, 7.8_

  - [x] 11.2 Create tutorial: `docs/book/src/tutorials/first-run.md`
    - "Your First Faultline Run" — install, run against sample repo, read output artifacts
    - _Requirements: 7.3_

  - [x] 11.3 Create how-to guide: `docs/book/src/howto/add-property-test.md`
    - "Adding a New Property Test" — create proptest, wire to requirement, update Scenario Atlas
    - _Requirements: 7.4_

  - [x] 11.4 Create explanation: `docs/book/src/explanations/localization.md`
    - "How Localization Works" — binary narrowing algorithm, outcome classification, confidence scoring
    - _Requirements: 7.5_

  - [x] 11.5 Create reference pages under `docs/book/src/reference/`
    - `cli-flags.md` — CLI flag reference
    - `artifact-schema.md` — artifact schema reference (link to JSON Schema)
    - `predicate-contract.md` — predicate contract (link to `docs/predicate-contract.md`)
    - `exit-codes.md` — exit code table
    - _Requirements: 7.6_

- [x] 12. Checkpoint — Wave 6 complete
  - Ensure `docs/book/` structure is complete, `SUMMARY.md` links all pages. Ask the user if questions arise.

- [x] 13. Wave 7 — CI & Governance: CI workflows, deny.toml, contract-aware checks
  - [x] 13.1 Create `deny.toml` at repo root
    - Configure `[advisories]`: vulnerability = deny, unmaintained = warn, yanked = deny
    - Configure `[licenses]`: unlicensed = deny, allow list: MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC, Unicode-DFS-2016
    - Configure `[bans]`: multiple-versions = warn, wildcards = deny
    - Configure `[sources]`: unknown-registry = deny, unknown-git = deny
    - _Requirements: 9.1_

  - [x] 13.2 Implement xtask `ci-fast` subcommand
    - Execute `cargo fmt --check`, `cargo clippy --workspace -- -D warnings`, `cargo test --workspace` in sequence
    - Stop on first failure with contract-aware error message (e.g., `"contract broken: code formatting"`)
    - Detect `cargo-nextest` and use it if available, fall back to `cargo test`
    - _Requirements: 5.3, 5.8_

  - [x] 13.3 Implement xtask `ci-full` subcommand
    - Run `ci-fast` steps, then golden test check (`cargo insta test`), then schema check (`check_schema()`)
    - On golden failure: print artifact name and `cargo insta review` instruction
    - On schema drift: print `"schema drift detected"` and remediation link
    - _Requirements: 5.2, 8.1, 8.2, 8.3, 8.5_

  - [x] 13.4 Implement xtask tool-detection helper
    - Create a `ensure_tool(name: &str, install_cmd: &str)` function that checks if a tool binary is on PATH
    - On missing tool: print `"error: {tool} is not installed\n  install: {install_cmd}"` and exit 1
    - Wire into `mutants`, `fuzz`, `docs-check`, `release-check` subcommands
    - _Requirements: 5.7_

  - [x] 13.5 Implement xtask `release-check` subcommand
    - Execute `cargo deny check`, `cargo audit`, `cargo semver-checks` in sequence
    - Report all findings with contract-aware messages
    - _Requirements: 9.2, 9.3_

  - [x] 13.6 Implement xtask `smoke`, `golden`, `mutants`, `fuzz`, `docs-check` subcommands
    - `smoke`: build CLI binary, run against fixture repo
    - `golden`: run `cargo insta test --review`
    - `mutants`: run `cargo mutants -p faultline-localization -- --lib`
    - `fuzz`: run `cargo fuzz run` with `--duration` flag (default 60s)
    - `docs-check`: run `mdbook build docs/book` and check for broken links
    - _Requirements: 5.2, 6.3, 6.6, 6.7, 7.7_

  - [x] 13.7 Create `.github/workflows/ci.yml` (ci-fast, every push)
    - Checkout, install Rust stable with rustfmt + clippy, run `cargo xtask ci-fast`
    - _Requirements: 8.6, 8.8_

  - [x] 13.8 Create `.github/workflows/ci-full.yml` (PRs)
    - Checkout, install Rust stable, install `cargo-insta`, run `cargo xtask ci-full`
    - _Requirements: 8.1, 8.6_

  - [x] 13.9 Create `.github/workflows/ci-extended.yml` (manual/release)
    - Trigger on `workflow_dispatch` and tag push `v*`
    - Install `cargo-mutants`, `cargo-deny`, `cargo-audit`, `cargo-semver-checks`
    - Run `cargo xtask mutants` and `cargo xtask release-check`
    - _Requirements: 8.6_

  - [x] 13.10 Create `.github/workflows/release.yml` (tag push)
    - Trigger on tag push `v*`
    - Install governance tools, run `cargo xtask release-check`
    - _Requirements: 9.5_

  - [x] 13.11 Write property test for tool detection error messages
    - **Property 44: Tool Detection Error Messages**
    - For a set of known tool names, verify the error message contains both the tool name and an install command
    - **Validates: Requirements 5.7**

  - [x] 13.12 Write property test for CI failure message contract identification
    - **Property 46: CI Failure Messages Identify Broken Contract**
    - For each contract check failure type (schema drift, stale golden, missing scenario), verify the error message contains the contract name and a documentation reference
    - **Validates: Requirements 8.7**

- [x] 14. Checkpoint — Wave 7 complete
  - Ensure `cargo xtask ci-fast` runs successfully, `deny.toml` exists, all CI workflow files exist. Ask the user if questions arise.

- [x] 15. Wave 8 — Scaffold Commands: scaffold crate, adr, scenario, doc subcommands
  - [x] 15.1 Implement `cargo xtask scaffold crate <name> --tier <tier>`
    - Validate crate name matches `faultline-[a-z][a-z0-9-]*`
    - Generate `crates/<name>/Cargo.toml` inheriting workspace package metadata
    - Generate `crates/<name>/src/lib.rs` with module doc comment
    - Append crate to workspace `Cargo.toml` members list
    - _Requirements: 10.2_

  - [x] 15.2 Implement `cargo xtask scaffold adr <title>`
    - Scan `docs/adr/` for highest existing numeric prefix
    - Generate new ADR file with next sequential number using `docs/adr/TEMPLATE.md`
    - Pre-fill title from argument
    - _Requirements: 10.3_

  - [x] 15.3 Implement `cargo xtask scaffold scenario <name> --crate <crate>`
    - Generate test file stub in target crate's `tests/` directory
    - Add placeholder entry to `docs/scenarios/scenario_index.md`
    - _Requirements: 10.4_

  - [x] 15.4 Implement `cargo xtask scaffold doc <title> --section <section>`
    - Validate section is one of: `tutorial`, `howto`, `explanation`, `reference`
    - Generate Markdown file in `docs/book/src/{section}/`
    - Add entry to `docs/book/src/SUMMARY.md`
    - _Requirements: 10.5_

  - [x] 15.5 Implement scaffold input validation
    - Reject crate names not matching `faultline-[a-z][a-z0-9-]*`
    - Reject empty ADR titles and scenario names
    - Reject doc sections not in the four Diátaxis categories
    - Print clear error messages for each validation failure
    - _Requirements: 10.6_

  - [x] 15.6 Write property test for scaffold crate generation
    - **Property 47: Scaffold Crate Generation**
    - Generate valid crate names and tiers, verify output: `Cargo.toml` inherits workspace metadata, `src/lib.rs` exists with doc comment, crate name in workspace members
    - **Validates: Requirements 10.2**

  - [x] 15.7 Write property test for scaffold ADR sequential numbering
    - **Property 48: Scaffold ADR Sequential Numbering**
    - Generate random existing ADR counts, verify next number is exactly one greater than highest existing prefix
    - **Validates: Requirements 10.3**

  - [x] 15.8 Write property test for scaffold file generation (scenarios and docs)
    - **Property 49: Scaffold File Generation for Scenarios and Docs**
    - Verify scenario scaffold creates test stub and index entry; doc scaffold creates Markdown file and SUMMARY.md entry
    - **Validates: Requirements 10.4, 10.5**

  - [x] 15.9 Write property test for scaffold input validation
    - **Property 50: Scaffold Input Validation**
    - Generate invalid crate names, empty strings, bad section names — verify rejection with appropriate errors
    - **Validates: Requirements 10.6**

- [x] 16. Checkpoint — Wave 8 complete
  - Ensure all scaffold subcommands work, input validation rejects bad inputs, generated files are well-formed. Ask the user if questions arise.

- [x] 17. Wave 9 — Advanced Verification: mutation testing config, fuzz targets
  - [x] 17.1 Configure mutation testing for `faultline-localization`
    - Create `mutants.toml` or document the xtask invocation targeting `faultline-localization` core narrowing and outcome logic
    - Verify `cargo xtask mutants` invokes `cargo mutants -p faultline-localization -- --lib`
    - _Requirements: 6.3, 6.6_

  - [x] 17.2 Create fuzz target for `AnalysisReport` JSON deserialization
    - Create `fuzz/` directory structure with `Cargo.toml` and `fuzz_targets/fuzz_analysis_report.rs`
    - Fuzz harness: deserialize arbitrary bytes as `AnalysisReport` via `serde_json::from_str`
    - Verify `cargo xtask fuzz --duration 10` invokes the target correctly
    - _Requirements: 6.4, 6.7_

  - [x] 17.3 Implement scenario atlas verification in xtask
    - Add a `check-scenarios` step to `ci-full` that compares test functions in the workspace against `docs/scenarios/scenario_index.md`
    - Report missing entries: `"contract broken: scenario atlas\n  missing entries for: {files}"`
    - Report stale entries: entries referencing tests that no longer exist
    - _Requirements: 2.5, 2.6, 8.4_

  - [x] 17.4 Write property test for scenario atlas consistency
    - **Property 39: Scenario Atlas Consistency**
    - Generate random sets of test names and index entries, verify the verification logic reports exactly the symmetric difference
    - **Validates: Requirements 2.5, 8.4**

- [x] 18. Checkpoint — Wave 9 complete
  - Ensure mutation testing config exists, fuzz target compiles, scenario atlas verification logic works. Ask the user if questions arise.

- [x] 19. Wave 10 — Final Integration and Wiring
  - [x] 19.1 Wire export adapters into xtask and CI
    - Add `faultline-sarif` and `faultline-junit` as dependencies of xtask (optional, for future CLI integration)
    - Ensure golden tests cover SARIF and JUnit output if desired
    - Verify all new crates are in workspace members and compile cleanly
    - _Requirements: 3.6, 3.7_

  - [x] 19.2 Update `docs/scenarios/scenario_index.md` with all new tests
    - Add entries for all property tests P37–P50
    - Add entries for golden tests (analysis.json, index.html, CLI help)
    - Add entries for export adapter tests
    - Ensure scenario atlas verification passes
    - _Requirements: 2.1, 2.5, 2.6_

  - [x] 19.3 Update `docs/scenarios/behavior_map.md` with new cross-references
    - Add rows for all 10 repo-operating-system requirements
    - Cross-reference to new ADRs, scenarios, fixtures, and artifacts
    - _Requirements: 2.4_

  - [x] 19.4 Update `docs/crate-map.md` and `docs/verification-matrix.md`
    - Add `faultline-sarif`, `faultline-junit`, `xtask` to crate map
    - Update verification matrix with new golden tests, property tests, and fuzz targets
    - _Requirements: 4.5, 6.1_

  - [x] 19.5 Verify all teaching layer cross-references
    - Ensure `AGENTS.md` links to all new documents
    - Ensure `TESTING.md` documents new CI tiers and verification techniques
    - Ensure `RELEASE.md` references `cargo xtask release-check`
    - _Requirements: 4.7_

- [x] 20. Final checkpoint — Ensure all tests pass
  - Run `cargo xtask ci-fast` to verify full workspace compiles, all tests pass, formatting and linting are clean. Ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- Each task references specific requirements for traceability
- Checkpoints ensure incremental validation — the workspace must compile after each wave
- Property tests validate universal correctness properties from the design document (P37–P50)
- Unit tests validate specific examples and edge cases
- The implementation language is Rust throughout (stable, edition 2021, workspace resolver v2)
