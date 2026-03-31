# Pattern Catalog

This catalog defines the 10 named patterns that govern the faultline repository's architecture, testing, documentation, and operational practices. Each pattern is a reusable decision template that contributors and agents should follow when making changes.

---

## 1. Truth Core / Translation Edge

**Definition:** Domain logic lives in pure crates with no I/O; all infrastructure concerns are pushed to adapter crates behind port traits.

**When to use:** Any time you add business logic — it belongs in a domain crate (`faultline-codes`, `faultline-types`, `faultline-localization`, `faultline-surface`). If the logic requires filesystem, network, or process interaction, it belongs in an adapter crate behind a port trait.

**Example:** `faultline-localization` contains the binary narrowing algorithm with zero filesystem or process dependencies. It operates on `RevisionSequence` and `ProbeObservation` values injected by the app layer. The `faultline-git` adapter implements `HistoryPort` and `CheckoutPort` by shelling out to the system `git` binary — the domain never touches Git directly.

**Anti-example:** Putting `git rev-list` parsing inside `faultline-localization` would violate this pattern — Git CLI interaction belongs in `faultline-git`. Similarly, writing files directly from `faultline-surface` would break the boundary.

**Related ADRs:** [ADR-0001: Hexagonal Architecture](../adr/0001-hexagonal-architecture.md)
**Related Scenarios:** `prop_binary_narrowing_selects_valid_midpoint`, `prop_adjacent_pass_fail_yields_first_bad`

---

## 2. Scenario Atlas

**Definition:** Every test in the workspace is cataloged in a flat index with its problem statement, fixture, crate, artifact, invariant, and cross-references, organized by crate tier.

**When to use:** Whenever you add, remove, or rename a test. The scenario index (`docs/scenarios/scenario_index.md`) and behavior map (`docs/scenarios/behavior_map.md`) must stay in sync with the actual test suite. CI enforces this via the scenario atlas consistency check.

**Example:** The property test `prop_exit_code_classification` in `faultline-probe-exec` is listed in the scenario index with its generator (`arb exit_code × timed_out`), the crate it exercises, and the requirement it validates (Req 2.3–2.6).

**Anti-example:** Adding a new property test to `faultline-localization` without creating a corresponding entry in the scenario index. The CI check will catch this and fail with `"contract broken: scenario atlas"`.

**Related ADRs:** [ADR-0003: Honest Localization Outcomes](../adr/0003-honest-localization-outcomes.md)
**Related Scenarios:** See [Scenario Index](../scenarios/scenario_index.md)

---

## 3. Artifact-First Boundary

**Definition:** Every faultline run produces portable, inspectable artifacts (`analysis.json`, `index.html`) whose structure is governed by a JSON Schema and validated by golden tests.

**When to use:** When changing the `AnalysisReport` structure, adding new output fields, or modifying the HTML renderer. The artifact schema must be regenerated (`cargo xtask golden`), golden snapshots must be reviewed (`cargo insta review`), and the schema version must be evaluated for compatibility.

**Example:** `faultline-render` writes `analysis.json` (canonical JSON) and `index.html` (self-contained HTML) from an `AnalysisReport`. Golden tests via `insta` snapshot both artifacts against a canonical fixture. The JSON Schema at `schemas/analysis-report.schema.json` is generated from the Rust types via `schemars`.

**Anti-example:** Adding a new field to `AnalysisReport` without regenerating the JSON Schema or updating golden snapshots. CI will fail with `"schema drift detected"`.

**Related ADRs:** [ADR-0001: Hexagonal Architecture](../adr/0001-hexagonal-architecture.md)
**Related Scenarios:** `golden_analysis_json`, `golden_index_html`, `prop_json_schema_validates_all_valid_reports`

---

## 4. Proof-Carrying Change

**Definition:** Every change to domain logic or artifact contracts must be accompanied by a test that proves the change is correct — property tests for invariants, golden tests for artifacts, BDD tests for integration flows.

**When to use:** When modifying localization logic, outcome classification, exit code mapping, artifact serialization, or any domain invariant. The verification matrix (`docs/verification-matrix.md`) specifies which techniques apply to each crate.

**Example:** Adding a new `AmbiguityReason` variant requires updating the property test `prop_ambiguous_observations_yield_suspect_window` to cover the new reason, and updating golden snapshots if the HTML rendering changes.

**Anti-example:** Changing the confidence scoring formula in `faultline-localization` without updating or adding a property test that validates the new behavior. The mutation testing budget for `faultline-localization` is specifically designed to catch this.

**Related ADRs:** [ADR-0003: Honest Localization Outcomes](../adr/0003-honest-localization-outcomes.md)
**Related Scenarios:** `prop_suspect_window_confidence_cap`, `prop_first_bad_requires_direct_evidence`

---

## 5. Replayable Run Memory

**Definition:** Every faultline run persists its request, observations, and report to disk so that runs can be resumed, replayed, and audited after the fact.

**When to use:** When modifying the run store, observation persistence, or the localization loop's caching behavior. The store uses atomic writes and lock files to prevent corruption.

**Example:** `faultline-store` persists observations to `{run_dir}/observations.json` using atomic writes (write to `.tmp`, then rename). When a run is resumed, cached observations are loaded and the localization loop skips already-probed commits. The `integration_cached_resume_skips_cached_commits` test validates this behavior.

**Anti-example:** Writing observations directly to the final path without atomic rename — a crash mid-write would leave a corrupted file that breaks resume.

**Related ADRs:** [ADR-0002: Git CLI and Disposable Checkouts](../adr/0002-git-cli-and-disposable-checkouts.md)
**Related Scenarios:** `prepare_run_creates_directory_and_request_json`, `save_and_load_single_observation`, `integration_cached_resume_skips_cached_commits`

---

## 6. Operator Dossier

**Definition:** The CLI output and HTML report provide the operator with a complete dossier: outcome, confidence, ambiguity reasons, observation timeline, changed surface, and execution surfaces — everything needed to begin investigating without re-running.

**When to use:** When modifying CLI output formatting, HTML rendering, or adding new diagnostic information to the report. The operator should never need to re-run faultline to understand what happened.

**Example:** The HTML report includes outcome-specific CSS classes (`outcome-firstbad`, `outcome-suspect`, `outcome-inconclusive`), ambiguity reason badges, a temporal observation timeline sorted by `sequence_index`, and a separate execution surfaces section highlighting CI workflows and build scripts.

**Anti-example:** Rendering the observation timeline in commit-hash alphabetical order instead of probe sequence order — the operator needs to see the temporal flow of the investigation.

**Related ADRs:** [ADR-0003: Honest Localization Outcomes](../adr/0003-honest-localization-outcomes.md)
**Related Scenarios:** `prop_html_contains_required_data`, `prop_html_temporal_observation_order`, `prop_operator_code_exit_code_mapping`

---

## 7. Human Review Gate After Failing Property

**Definition:** When a property test fails, the failure is reported with the counterexample and the contributor must review and accept the change — automated systems do not silently suppress property test failures.

**When to use:** In CI configuration and the human review process. Property test failures in CI produce actionable messages identifying which contract was broken and linking to remediation documentation. Golden test failures require explicit `cargo insta review` acceptance.

**Example:** When `prop_binary_narrowing_selects_valid_midpoint` fails, CI reports the counterexample (the sequence size and observation pattern that triggered the failure) and links to `TESTING.md` for remediation steps. A maintainer must review the counterexample and either fix the code or update the property.

**Anti-example:** Configuring CI to ignore property test failures or automatically accepting golden snapshot changes without human review.

**Related ADRs:** [ADR-0003: Honest Localization Outcomes](../adr/0003-honest-localization-outcomes.md)
**Related Scenarios:** `prop_schema_drift_detection`, all `proptest!` blocks in the workspace

---

## 8. Mutation-On-Diff

**Definition:** Mutation testing is applied to the domain crates most critical to correctness — `faultline-localization` (narrowing + outcome logic) and `faultline-app` (orchestration loop) — to ensure that the test suite catches meaningful code changes.

**When to use:** When modifying core narrowing logic, outcome classification, or the orchestration loop. Run `cargo xtask mutants` to verify that surviving mutants are intentional (equivalent mutants) rather than gaps in test coverage.

**Example:** `cargo xtask mutants` invokes `cargo-mutants` targeting `faultline-localization`'s core narrowing and outcome logic. Surviving mutants are reviewed to determine if they represent equivalent mutations or missing test coverage.

**Anti-example:** Running mutation testing on adapter crates like `faultline-git` where the test suite depends on external state (real Git repos) — mutation testing is most valuable on pure domain logic.

**Related ADRs:** [ADR-0001: Hexagonal Architecture](../adr/0001-hexagonal-architecture.md)
**Related Scenarios:** `prop_monotonic_window_narrowing`, `prop_probe_count_respects_max_probes`

---

## 9. Golden Artifact Contract

**Definition:** Key artifacts (JSON report, HTML report, CLI help text) are snapshot-tested via `insta` so that any change to their structure or content requires explicit human review and acceptance.

**When to use:** When modifying `AnalysisReport` serialization, HTML rendering, or CLI flag definitions. Run `cargo insta test` to detect changes, then `cargo insta review` to accept or reject them.

**Example:** The `golden_analysis_json` test in `faultline-render` snapshots the JSON output of a canonical fixture report. The `golden_cli_help` test in `faultline-cli` snapshots the `--help` text. When these change, CI fails with instructions to run `cargo insta review`.

**Anti-example:** Deleting or regenerating golden snapshots without reviewing the diff — this defeats the purpose of the contract.

**Related ADRs:** [ADR-0001: Hexagonal Architecture](../adr/0001-hexagonal-architecture.md)
**Related Scenarios:** `golden_analysis_json`, `golden_index_html`, `golden_cli_help`

---

## 10. Delegation-Safe Crate Seam

**Definition:** Crate boundaries are drawn at natural seams where a delegated agent or new contributor can work on one crate without understanding the internals of others — each crate has a clear responsibility, a defined tier, and documented dependencies.

**When to use:** When adding a new crate or restructuring existing ones. Use `cargo xtask scaffold crate` to generate the boilerplate. The crate map (`docs/crate-map.md`) documents every crate's tier, dependencies, and responsibility.

**Example:** `faultline-sarif` and `faultline-junit` are adapter crates that depend only on `faultline-types`. A contributor can implement SARIF export without understanding the localization algorithm — they only need the `AnalysisReport` type definition.

**Anti-example:** Creating a "utils" crate that every other crate depends on — this creates a coupling hub that defeats the purpose of crate seams. Each crate should have a focused, single responsibility.

**Related ADRs:** [ADR-0001: Hexagonal Architecture](../adr/0001-hexagonal-architecture.md)
**Related Scenarios:** `prop_sarif_export_structural_validity`, `prop_junit_xml_export_structural_validity`

---

## Glossary

| Term | Definition |
|------|-----------|
| **Adapter** | A crate that implements a port trait, bridging domain logic to infrastructure (Git, filesystem, processes). |
| **Artifact** | A portable output file produced by a faultline run (`analysis.json`, `index.html`). |
| **BDD scenario** | A behavior-driven test that exercises an integration flow with realistic fixtures. |
| **Boundary pair** | The adjacent last-good and first-bad commits that define the regression boundary. |
| **Confidence** | A score (high/medium/low) indicating how much certainty the evidence supports for a localization outcome. |
| **Domain crate** | A pure crate with no I/O dependencies that contains business logic. |
| **FirstBad** | A localization outcome where the exact first-bad commit is identified with high confidence. |
| **Golden test** | A snapshot test (via `insta`) that captures the expected output of an artifact for regression detection. |
| **Inconclusive** | A localization outcome where insufficient evidence prevents narrowing the regression window. |
| **Localization** | The process of narrowing a regression window by binary search over a revision sequence. |
| **Mutation testing** | A verification technique that introduces small code changes (mutants) to verify the test suite detects them. |
| **Non-monotonic evidence** | When a Fail observation appears before a Pass observation in the revision sequence, violating the expected pass→fail transition. |
| **Operator** | The human or automated system that invokes faultline with a predicate command. |
| **Port** | A trait defining an outbound boundary between domain logic and infrastructure. |
| **Predicate** | The operator-supplied command that faultline runs at each candidate revision to determine pass/fail/skip/indeterminate. |
| **Property test** | A test using `proptest` that verifies an invariant holds across many randomly generated inputs. |
| **Revision sequence** | An ordered list of commits between the good and bad boundaries. |
| **Scenario Atlas** | The collection of documents (`scenario_index.md`, `behavior_map.md`) that catalog all tests in the workspace. |
| **SuspectWindow** | A localization outcome where the regression is narrowed to a range but not an exact commit, with ambiguity reasons. |
| **Verification matrix** | A per-crate mapping of applicable verification techniques (property, BDD, golden, fuzz, mutation, smoke). |
