# Implementation Plan: v0.1 Product Sharpening

## Overview

This plan implements the v0.1 product sharpening pass across 13 waves organized in three tiers: product differentiation features (Waves 1–6), verification depth (Waves 7–10), and repo maturation (Waves 11–13). Each wave builds on the previous, with checkpoints after major milestones. All code is Rust (stable, edition 2024) targeting the existing hexagonal workspace.

## Tasks

- [x] 1. Wave 1 — Type changes and contract freeze
  - [x] 1.1 Add new types to `faultline-types`
    - Add `SuspectEntry` struct with fields: `path`, `priority_score`, `surface_kind`, `change_status`, `is_execution_surface`, `owner_hint`
    - Add `FlakePolicy` struct with `retries: u32`, `stability_threshold: f64`, and `Default` impl (retries=0, stability_threshold=1.0)
    - Add `FlakeSignal` struct with `total_runs`, `pass_count`, `fail_count`, `skip_count`, `indeterminate_count`, `is_stable`
    - Add `ReproductionCapsule` struct with `commit`, `predicate`, `env`, `working_dir`, `timeout_seconds`
    - Add `RunComparison` struct with all fields from design (left/right run_id, outcome_changed, confidence_delta, window_width_delta, probes_reused, suspect_paths_added/removed, ambiguity_reasons_added/removed)
    - Add `pub fn compare_runs(left: &AnalysisReport, right: &AnalysisReport) -> RunComparison` pure function
    - Add `impl ReproductionCapsule { pub fn to_shell_script(&self) -> String }` method
    - _Requirements: 1.1, 1.9, 3.1, 3.2, 4.1, 4.4, 5.1_

  - [x] 1.2 Modify existing types in `faultline-types`
    - Add `#[serde(default)] pub flake_signal: Option<FlakeSignal>` to `ProbeObservation`
    - Add `#[serde(default)] pub flake_policy: FlakePolicy` to `SearchPolicy`
    - Add `#[serde(default)] pub suspect_surface: Vec<SuspectEntry>` to `AnalysisReport`
    - Add `#[serde(default)] pub reproduction_capsules: Vec<ReproductionCapsule>` to `AnalysisReport`
    - Bump `default_schema_version()` from `"0.1.0"` to `"0.2.0"`
    - _Requirements: 1.1, 1.9, 3.1, 3.6, 4.1_

  - [x] 1.3 Add new `HistoryPort` methods in `faultline-ports`
    - Add `fn codeowners_for_paths(&self, paths: &[String]) -> Result<HashMap<String, Option<String>>>` to `HistoryPort` trait
    - Add `fn blame_frequency(&self, paths: &[String]) -> Result<HashMap<String, Option<String>>>` to `HistoryPort` trait
    - _Requirements: 1.3, 1.4_

  - [x] 1.4 Update all construction sites and adapters for compilation
    - Update `faultline-git` `GitAdapter` to implement the two new `HistoryPort` methods (stub returning empty maps initially)
    - Update `faultline-fixtures::arb` — extend `arb_probe_observation()` to include `flake_signal: None`, extend `arb_search_policy()` to include default `flake_policy`, extend `arb_analysis_report()` to include empty `suspect_surface` and `reproduction_capsules`
    - Update `faultline-app` report construction to populate `suspect_surface: vec![]` and `reproduction_capsules: vec![]`
    - Update `faultline-render` golden snapshot expectations for new fields
    - Update `faultline-sarif` and `faultline-junit` if they reference `AnalysisReport` fields directly
    - Regenerate JSON schema via `cargo xtask generate-schema`
    - _Requirements: 1.3, 1.4, 1.9_

- [x] 2. Checkpoint — Wave 1 compilation
  - Run `cargo test` across the entire workspace; ensure all crates compile and existing tests pass with the new fields defaulting to empty/None
  - Update golden snapshots with `cargo insta review` if needed
  - Ensure all tests pass, ask the user if questions arise.

- [x] 3. Wave 2 — Ranked suspect surface
  - [x] 3.1 Implement `SurfaceAnalyzer::rank_suspect_surface` in `faultline-surface`
    - Add `pub fn rank_suspect_surface(&self, changes: &[PathChange], owners: &HashMap<String, Option<String>>) -> Vec<SuspectEntry>` to `SurfaceAnalyzer`
    - Implement scoring rules: base 100, execution surface +200, deleted +150, renamed +100, source +50, test +25
    - Tie-breaking: descending score, ascending lexicographic path
    - Reuse existing `surface_kind()` and `is_execution_surface()` functions for classification
    - Return empty vec for empty input
    - _Requirements: 1.1, 1.2, 1.6, 1.7, 1.10_

  - [x] 3.2 Write property test for P43: Suspect surface ranking is sorted and deterministic
    - **Property 43: Suspect surface ranking is sorted and deterministic**
    - Generate arbitrary `Vec<PathChange>` and owners map, call `rank_suspect_surface` twice, assert identical output sorted by descending score then ascending path
    - Place in `faultline-surface` tests, use `ProptestConfig { cases: 100, .. }`
    - **Validates: Requirements 1.1, 1.10**

  - [x] 3.3 Write property test for P44: Execution surfaces, renames, and deletes score higher
    - **Property 44: Execution surfaces, renames, and deletes score higher than ordinary modifications**
    - Generate mixed `PathChange` sets containing at least one execution surface, one rename, one delete, and one ordinary modified source; assert their scores are strictly greater than the ordinary source score
    - Place in `faultline-surface` tests
    - **Validates: Requirements 1.2**

  - [x] 3.4 Write property test for P45: SuspectEntry preserves change_status and surface_kind
    - **Property 45: SuspectEntry preserves change_status and has consistent surface_kind**
    - For any `PathChange`, verify the resulting `SuspectEntry` has matching `change_status`, correct `surface_kind`, and correct `is_execution_surface`
    - Place in `faultline-surface` tests
    - **Validates: Requirements 1.6, 1.7**

  - [x] 3.5 Write property test for P46: SuspectEntry owner_hint matches owners map
    - **Property 46: SuspectEntry owner_hint matches the provided owners map**
    - Generate arbitrary `PathChange` values and owners map, verify each `SuspectEntry.owner_hint` equals the map value (or None if absent)
    - Place in `faultline-surface` tests
    - **Validates: Requirements 1.3, 1.4, 1.5**

  - [x] 3.6 Implement CODEOWNERS parsing in `faultline-git`
    - Implement `GitAdapter::codeowners_for_paths`: read `.github/CODEOWNERS` or `CODEOWNERS` from repo root, parse gitignore-style patterns, match paths, return first matching owner
    - Handle malformed files gracefully (log warning, return empty map)
    - _Requirements: 1.3_

  - [x] 3.7 Write property test for P55: CODEOWNERS parser determinism
    - **Property 55: CODEOWNERS parser determinism**
    - Generate random CODEOWNERS content (lines of pattern + owner pairs) and file paths, assert parsing produces deterministic results
    - Place in `faultline-git` tests
    - **Validates: Requirements 1.3**

  - [x] 3.8 Implement blame frequency in `faultline-git`
    - Implement `GitAdapter::blame_frequency`: run `git log --format='%aN' --since='90 days ago' -- <path>` for each path, return most-frequent author
    - Handle errors gracefully (log warning, return empty map)
    - _Requirements: 1.4_

  - [x] 3.9 Wire suspect surface through `faultline-app`
    - In `FaultlineApp::localize_with_options`, after computing `changed_paths`:
      - Call `self.history.codeowners_for_paths(paths)` — on error, fall back to `blame_frequency`
      - Call `self.surface.rank_suspect_surface(changes, owners)`
      - Populate `report.suspect_surface` with the ranked entries
    - _Requirements: 1.1, 1.3, 1.4, 1.5_

  - [x] 3.10 Render suspect surface in HTML report
    - Update `ReportRenderer::render_html` in `faultline-render` to render `suspect_surface` as a prioritized list with visual distinction for execution surfaces and owner hints inline
    - Update `index.html` golden snapshot
    - _Requirements: 1.8_

- [x] 4. Checkpoint — Wave 2 suspect surface
  - Run `cargo test -p faultline-surface -p faultline-git -p faultline-app -p faultline-render`
  - Verify suspect surface appears in HTML output and JSON artifact
  - Ensure all tests pass, ask the user if questions arise.

- [x] 5. Wave 3 — Flake-aware probing
  - [x] 5.1 Implement `FlakeSignal` computation in `faultline-types`
    - Add `pub fn compute_flake_signal(results: &[ObservationClass], stability_threshold: f64) -> FlakeSignal` function
    - `is_stable` = true iff proportion of most-frequent class >= `stability_threshold`
    - Counts must sum to `total_runs`
    - _Requirements: 3.2, 3.3_

  - [x] 5.2 Write property test for P48: FlakeSignal stability classification
    - **Property 48: FlakeSignal stability classification**
    - Generate random `Vec<ObservationClass>` and `stability_threshold` in [0.0, 1.0], verify `is_stable` matches threshold logic and counts sum to `total_runs`
    - Place in `faultline-types` or `faultline-localization` tests
    - **Validates: Requirements 3.2, 3.3**

  - [x] 5.3 Add `FlakePolicy` awareness to `LocalizationSession` in `faultline-localization`
    - Constructor already receives `SearchPolicy` which now contains `flake_policy`
    - In `record()`, check `flake_signal.is_stable` — unstable observations degrade confidence
    - In `outcome()`, reduce confidence score when any observation has `flake_signal.is_stable == false`
    - _Requirements: 3.4, 3.6_

  - [x] 5.4 Write property test for P49: Flaky observations degrade confidence
    - **Property 49: Flaky observations degrade confidence**
    - Metamorphic test: compare confidence of a session with all-stable observations vs same session with one unstable observation; unstable must have strictly lower confidence
    - Place in `faultline-localization` tests
    - **Validates: Requirements 3.4**

  - [x] 5.5 Write property test for P50: Default FlakePolicy produces no FlakeSignal
    - **Property 50: Default FlakePolicy produces no FlakeSignal**
    - With default `FlakePolicy` (retries=0), verify all observations in the report have `flake_signal == None`
    - Place in `faultline-localization` tests
    - **Validates: Requirements 3.6**

  - [x] 5.6 Implement flake retry loop in `faultline-app`
    - In `FaultlineApp::localize_with_options`, when `policy.flake_policy.retries > 0`:
      - Probe each commit up to `1 + retries` times
      - Compute `FlakeSignal` from the set of results using `compute_flake_signal`
      - Classify using majority vote weighted by `stability_threshold`
      - Re-probe ambiguous/boundary commits if initial result is unstable
    - When retries=0 (default), skip retry logic entirely — no `FlakeSignal` attached
    - _Requirements: 3.1, 3.4, 3.5, 3.6_

  - [x] 5.7 Add CLI flags `--retries` and `--stability-threshold` to `faultline-cli`
    - Add `--retries <N>` flag (default 0) mapped to `FlakePolicy.retries`
    - Add `--stability-threshold <F>` flag (default 1.0) mapped to `FlakePolicy.stability_threshold`
    - Validate `stability_threshold` is in [0.0, 1.0], reject with `FaultlineError::InvalidInput` otherwise
    - Update CLI `--help` golden snapshot
    - _Requirements: 3.1, 3.5_

- [x] 6. Checkpoint — Wave 3 flake-aware probing
  - Run `cargo test -p faultline-types -p faultline-localization -p faultline-app -p faultline-cli`
  - Ensure all tests pass, ask the user if questions arise.

- [x] 7. Wave 4 — Reproduction capsule
  - [x] 7.1 Implement `ReproductionCapsule::to_shell_script` in `faultline-types`
    - Generate a POSIX shell script containing: `cd <working_dir>`, `git checkout <commit>`, env var exports, the predicate command, and `timeout <seconds>`
    - Escape shell-special characters in predicate arguments and environment values
    - Handle both `ProbeSpec::Exec` and `ProbeSpec::Shell` variants
    - _Requirements: 4.4_

  - [x] 7.2 Write property test for P52: Shell script generation contains required fields
    - **Property 52: Shell script generation contains required fields**
    - Generate random `ReproductionCapsule`, verify `to_shell_script()` output contains commit SHA, predicate command, timeout value, and each env key-value pair
    - Place in `faultline-types` tests
    - **Validates: Requirements 4.4**

  - [x] 7.3 Generate reproduction capsules in `faultline-app`
    - After building the report, generate one `ReproductionCapsule` per observation from `request.probe`, observation commit, request env, working dir, and timeout
    - Populate `report.reproduction_capsules`
    - _Requirements: 4.1, 4.2_

  - [x] 7.4 Write property test for P51: ReproductionCapsule structural correspondence
    - **Property 51: ReproductionCapsule structural correspondence**
    - For any `AnalysisReport`, verify capsule count equals observation count, each capsule has a matching observation commit, capsule predicate equals `request.probe`, capsule timeout equals probe spec timeout
    - Place in `faultline-app` or `faultline-types` tests
    - **Validates: Requirements 4.1, 4.2**

  - [x] 7.5 Add CLI `reproduce` subcommand to `faultline-cli`
    - `faultline reproduce --run-dir <path> [--commit <sha>] [--shell]`
    - Read report from run-dir, extract capsule for given commit (or boundary commits by default)
    - `--shell` emits shell script to stdout instead of executing
    - Update CLI `--help` golden snapshot
    - _Requirements: 4.3, 4.5_

- [x] 8. Wave 5 — Run-to-run comparison
  - [x] 8.1 Implement `compare_runs` pure function in `faultline-types`
    - Compare two `AnalysisReport` values: detect outcome changes, compute confidence delta, window width delta, count reused probes (matching commit+class pairs), diff suspect paths and ambiguity reasons
    - Function must never panic — always returns a `RunComparison`
    - _Requirements: 5.1, 5.2, 5.3_

  - [x] 8.2 Write property test for P53: compare_runs is total
    - **Property 53: compare_runs is total**
    - Generate two arbitrary `AnalysisReport` values, call `compare_runs`, assert it returns without panicking
    - Place in `faultline-types` tests
    - **Validates: Requirements 5.1**

  - [x] 8.3 Write property test for P54: Self-comparison yields zero diff
    - **Property 54: Self-comparison yields zero diff**
    - Generate one `AnalysisReport`, call `compare_runs(report, report.clone())`, assert `outcome_changed == false`, `confidence_delta == 0`, `window_width_delta == 0`, `probes_reused == observations.len()`, and all added/removed vecs are empty
    - Place in `faultline-types` tests
    - **Validates: Requirements 5.3**

  - [x] 8.4 Add CLI `diff-runs` subcommand to `faultline-cli`
    - `faultline diff-runs --left <path> --right <path> [--json]`
    - Load both reports, call `compare_runs`, render human-readable summary (or JSON with `--json`)
    - Update CLI `--help` golden snapshot
    - _Requirements: 5.4, 5.5_

- [x] 9. Checkpoint — Waves 4–5 capsule and comparison
  - Run `cargo test -p faultline-types -p faultline-app -p faultline-cli`
  - Ensure all tests pass, ask the user if questions arise.

- [x] 10. Wave 6 — Markdown dossier export
  - [x] 10.1 Implement `render_markdown` in `faultline-render`
    - Add `pub fn render_markdown(report: &AnalysisReport) -> String` as a new `markdown` module in `faultline-render`
    - Sections: outcome summary (one-line), boundary info (good/bad/window width), ranked suspect surface (top 10 with scores and owners), observation timeline (table: commit | class | duration | flake?), reproduction command (shell one-liner for boundary commit), artifact links
    - Never fails — returns placeholder text for missing/empty fields
    - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.5, 2.6_

  - [x] 10.2 Write property test for P47: Markdown dossier contains all required sections
    - **Property 47: Markdown dossier contains all required sections**
    - Generate arbitrary `AnalysisReport` with at least one observation and non-Inconclusive outcome; verify output contains: outcome variant name + boundary commit IDs, good/bad revision specs, at least one suspect path (if non-empty), observation table rows with commit ID and class, reproduction command (if capsules non-empty)
    - Place in `faultline-render` tests
    - **Validates: Requirements 2.1, 2.2, 2.3, 2.4, 2.5**

  - [x] 10.3 Add CLI `--markdown` flag and `export-markdown` subcommand
    - `--markdown` flag on the main command: write Markdown dossier alongside HTML/JSON
    - `faultline export-markdown --run-dir <path>`: read existing report, emit Markdown to stdout or file
    - Update CLI `--help` golden snapshot
    - _Requirements: 2.7, 2.8_

  - [x] 10.4 Update SARIF and JUnit adapters for suspect_surface
    - Update `faultline-sarif` to include `suspect_surface` entries as SARIF result locations
    - Update `faultline-junit` to include `suspect_surface` in `system-out` or properties
    - _Requirements: 1.8, 1.9_

- [x] 11. Checkpoint — Tier 1 complete
  - Run `cargo test` across the entire workspace
  - Verify all Tier 1 product differentiation features work end-to-end
  - Regenerate JSON schema: `cargo xtask generate-schema`
  - Review and accept all golden snapshots: `cargo insta review`
  - Ensure all tests pass, ask the user if questions arise.

- [x] 12. Wave 7 — Mutation coverage expansion
  - [x] 12.1 Extend `mutants.toml` for all adapter and export crates
    - Add `examine_re` entries for: `faultline_git::`, `faultline_probe_exec::`, `faultline_store::`, `faultline_render::`, `faultline_sarif::`, `faultline_junit::`, `faultline_surface::`
    - _Requirements: 6.1, 6.2_

  - [x] 12.2 Add `--crate` flag to `cargo xtask mutants`
    - Update `crates/xtask/src/lib.rs` (or relevant module) to accept `--crate <name>` for targeted mutation runs
    - _Requirements: 6.3_

- [x] 13. Wave 8 — Fuzz targets
  - [x] 13.1 Add fuzz target `fuzz_git_diff_parse`
    - Create `fuzz/fuzz_targets/fuzz_git_diff_parse.rs` — fuzz Git adapter diff output parsing with arbitrary byte strings
    - Register in `fuzz/Cargo.toml`
    - _Requirements: 7.1_

  - [x] 13.2 Add fuzz target `fuzz_store_json`
    - Create `fuzz/fuzz_targets/fuzz_store_json.rs` — fuzz store JSON deserialization with arbitrary byte strings as `observations.json`
    - Register in `fuzz/Cargo.toml`
    - _Requirements: 7.1_

  - [x] 13.3 Add fuzz target `fuzz_html_escape`
    - Create `fuzz/fuzz_targets/fuzz_html_escape.rs` — fuzz renderer HTML escaping with adversarial strings
    - Register in `fuzz/Cargo.toml`
    - _Requirements: 7.1_

  - [x] 13.4 Add fuzz target `fuzz_cli_args`
    - Create `fuzz/fuzz_targets/fuzz_cli_args.rs` — fuzz CLI argument parsing via clap with arbitrary string vectors
    - Register in `fuzz/Cargo.toml`
    - _Requirements: 7.1_

  - [x] 13.5 Add fuzz target `fuzz_sarif_export`
    - Create `fuzz/fuzz_targets/fuzz_sarif_export.rs` — fuzz SARIF serialization with arbitrary `AnalysisReport` JSON
    - Register in `fuzz/Cargo.toml`
    - _Requirements: 7.1_

  - [x] 13.6 Add fuzz target `fuzz_junit_export`
    - Create `fuzz/fuzz_targets/fuzz_junit_export.rs` — fuzz JUnit serialization with arbitrary `AnalysisReport` JSON
    - Register in `fuzz/Cargo.toml`
    - _Requirements: 7.1_

- [x] 14. Checkpoint — Waves 7–8 verification depth
  - Run `cargo test` to ensure fuzz targets compile
  - Verify `mutants.toml` covers all target crates
  - Ensure all tests pass, ask the user if questions arise.

- [x] 15. Wave 9 — BDD/scenario coverage
  - [x] 15.1 Add report generation end-to-end scenario
    - Write an integration test exercising app → render → verify artifacts (JSON + HTML + Markdown) exist and contain expected fields
    - Use `faultline-fixtures` builders for test data
    - _Requirements: 8.1_

  - [x] 15.2 Add resume/rerender scenario
    - Write a test that loads cached observations from store, re-renders the report, and verifies artifact consistency
    - _Requirements: 8.1_

  - [x] 15.3 Add schema evolution scenario
    - Write a test that deserializes an old-version (0.1.0) report JSON into the new `AnalysisReport` struct, verifying forward compatibility via `#[serde(default)]`
    - _Requirements: 8.1_

  - [x] 15.4 Add export surfaces scenario
    - Write a test that generates SARIF + JUnit from the same `AnalysisReport` and verifies both contain consistent suspect surface data
    - _Requirements: 8.1_

  - [x] 15.5 Add CI contract failure scenario
    - Write a test that simulates schema drift (modified `AnalysisReport` without schema regeneration) and verifies xtask detects the mismatch
    - _Requirements: 8.1_

- [-] 16. Wave 10 — Scenario atlas enrichment
  - [-] 16.1 Add metadata columns to `docs/scenarios/scenario_index.md`
    - Add columns: `scenario_tier` (domain | adapter | app | integration), `requirement_ids`, `artifact_contract`, `mutation_surface`, `criticality` (P0 | P1 | P2), `ownership_hint`, `human_review_required` (yes | no)
    - Populate metadata for all existing scenarios
    - _Requirements: 9.1, 9.2, 9.3_

  - [~] 16.2 Add scenario entries for all new tests from this spec
    - Add entries for all property tests P43–P55, fuzz targets, BDD scenarios, and unit tests created in this spec
    - Include full metadata columns for each entry
    - _Requirements: 9.1, 9.2_

- [ ] 17. Checkpoint — Tier 2 complete
  - Run `cargo test` across the entire workspace
  - Verify scenario atlas has metadata columns populated
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 18. Wave 11 — Xtask authority
  - [~] 18.1 Implement real smoke test in `cargo xtask smoke`
    - Replace placeholder with a real test: use `GitRepoBuilder` (or equivalent) to create a fixture repo with a known regression, run faultline CLI against it, verify exit code and artifact existence
    - _Requirements: 10.1_

  - [~] 18.2 Implement real link checking in `cargo xtask docs-check`
    - Integrate `lychee` or `markdown-link-check` for real link validation across all Markdown docs
    - _Requirements: 10.2_

  - [~] 18.3 Add explicit xtask commands
    - Add `cargo xtask generate-schema` — regenerate `schemas/analysis-report.schema.json` from Rust types
    - Add `cargo xtask check-scenarios` — verify scenario atlas entries match actual tests
    - Add `cargo xtask export-markdown` — run Markdown export from a report directory
    - Add `cargo xtask export-sarif` — run SARIF export from a report directory
    - Add `cargo xtask export-junit` — run JUnit export from a report directory
    - _Requirements: 10.3_

- [ ] 19. Wave 12 — Repo-law cleanup
  - [~] 19.1 Fix workspace metadata and placeholder language
    - Update `Cargo.toml` workspace `authors` field to correct value
    - Remove or replace placeholder language in docs
    - Align `docs/crate-map.md` with current crate set and new capabilities
    - Align `docs/verification-matrix.md` with expanded mutation/fuzz coverage
    - Fix any stale references in documentation
    - _Requirements: 11.1, 11.2, 11.3_

- [ ] 20. Wave 13 — Teaching layer depth
  - [~] 20.1 Create maintainer playbooks
    - Write playbook: reviewing failing property tests (in `docs/handbook/` or `MAINTAINERS.md`)
    - Write playbook: deciding test technique (property vs unit vs golden vs fuzz)
    - Write playbook: bumping `schema_version`
    - Write playbook: handling breaking changes
    - _Requirements: 12.1_

  - [~] 20.2 Add worked examples and Diátaxis depth
    - Add worked examples to `docs/handbook/` demonstrating key workflows
    - Strengthen Diátaxis depth: ensure tutorials, how-to guides, reference, and explanation sections are balanced
    - _Requirements: 12.2, 12.3_

- [ ] 21. Final checkpoint — All tiers complete
  - Run `cargo test` across the entire workspace
  - Run `cargo xtask ci-full` to verify all contracts pass
  - Regenerate JSON schema: `cargo xtask generate-schema`
  - Review and accept all golden snapshots: `cargo insta review`
  - Verify scenario atlas is complete with metadata for all new tests
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- Each task references specific requirements for traceability
- Checkpoints ensure incremental validation after each wave
- Property tests (P43–P55) validate universal correctness properties from the design
- All new `AnalysisReport` fields use `#[serde(default)]` for backward compatibility
- Golden snapshots must be updated after type changes ripple through the workspace
- New arbitrary generators in `faultline-fixtures::arb` are needed for property tests on new types
