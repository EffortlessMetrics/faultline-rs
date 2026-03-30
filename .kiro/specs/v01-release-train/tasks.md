# Implementation Plan: faultline v0.1 Release Train

## Overview

Implements the complete faultline v0.1 release across waves 0–7, following the hexagonal architecture. Each wave builds on the previous, starting with contract freeze (types, codes, ports), progressing through a thin vertical slice, honest localization, changed-surface summary, CLI polish, HTML artifacts, CI governance, and publish readiness. All code is Rust, tested with `proptest` for correctness properties and unit tests for fixture scenarios.

## Tasks

- [x] 1. Wave 0 — Contract Freeze: Types, Codes, Ports, and Fixture Builders
  - [x] 1.1 Finalize `faultline-codes` enums and derive traits
    - Ensure `ObservationClass`, `ProbeKind`, `AmbiguityReason`, `OperatorCode` all derive `Serialize`, `Deserialize`, `Debug`, `Clone`, `PartialEq`, `Eq`
    - Verify `Display` and `FromStr` impls for `ProbeKind` and `Display` for `AmbiguityReason`
    - _Requirements: 2.3, 2.4, 2.5, 2.6, 3.3, 3.4, 3.5_

  - [x] 1.2 Finalize `faultline-types` value objects and report model
    - Ensure all structs (`CommitId`, `RevisionSpec`, `ProbeSpec`, `SearchPolicy`, `AnalysisRequest`, `RevisionSequence`, `ProbeObservation`, `Confidence`, `LocalizationOutcome`, `PathChange`, `SubsystemBucket`, `SurfaceSummary`, `RunHandle`, `CheckedOutRevision`, `AnalysisReport`) derive `Serialize + Deserialize + Debug + Clone + PartialEq + Eq`
    - Verify `stable_hash`, `now_epoch_seconds`, `ProbeSpec::fingerprint`, `AnalysisRequest::fingerprint` utility functions
    - Verify `LocalizationOutcome::boundary_pair` helper
    - _Requirements: 6.2, 6.3, 6.5_

  - [x] 1.3 Write property test P14: JSON Serialization Determinism
    - **Property 14: JSON Serialization Determinism**
    - Add `proptest` as dev-dependency to `faultline-types`
    - Generate random `AnalysisReport` values, serialize to JSON twice, assert byte-identical output
    - **Validates: Requirement 6.3**

  - [x] 1.4 Write property test P15: AnalysisReport JSON Round-Trip
    - **Property 15: AnalysisReport JSON Round-Trip**
    - Generate random `AnalysisReport` values, serialize via `serde_json::to_string_pretty` then deserialize via `serde_json::from_str`, assert equality
    - **Validates: Requirement 6.5**

  - [x] 1.5 Finalize `faultline-ports` trait definitions
    - Verify `HistoryPort`, `CheckoutPort`, `ProbePort`, `RunStorePort` trait signatures match design
    - Ensure all trait methods use `faultline_types::Result<T>` return types
    - _Requirements: 1.1, 1.4, 2.1, 2.8, 4.1, 4.2, 4.5, 4.6_

  - [x] 1.6 Implement `RevisionSequenceBuilder` in `faultline-fixtures`
    - Verify `push` and `build` methods produce valid `RevisionSequence` values
    - Add helper methods for common fixture scenarios: `exact_boundary(n)`, `with_labels(labels)`
    - _Requirements: 12.1_

  - [x] 1.7 Write property test P3: Revision Sequence Boundary Invariant
    - **Property 3: Revision Sequence Boundary Invariant**
    - Add `proptest` as dev-dependency to `faultline-fixtures` or `faultline-localization`
    - Generate sequences via builder, verify first == good, last == bad, length >= 2
    - **Validates: Requirements 1.4, 1.5**

- [x] 2. Checkpoint — Wave 0 complete
  - Ensure `cargo test -p faultline-codes -p faultline-types -p faultline-ports -p faultline-fixtures` passes. Ask the user if questions arise.

- [x] 3. Wave 1 — Thin Vertical Slice: Linearize → Checkout → Probe → Localize → JSON
  - [x] 3.1 Implement `GitAdapter` history linearization (`HistoryPort::linearize`)
    - Resolve revisions via `git rev-parse --verify`
    - Verify ancestry via `git merge-base --is-ancestor`
    - Produce `RevisionSequence` via `git rev-list --reverse --ancestry-path [--first-parent]`
    - Ensure good commit is first, bad commit is last, sequence has >= 2 elements
    - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 1.6_

  - [x] 3.2 Implement `GitAdapter` checkout and cleanup (`CheckoutPort`)
    - `checkout_revision`: create disposable worktree via `git worktree add --detach --force` under `.faultline/scratch/`
    - `cleanup_checkout`: remove via `git worktree remove --force` with fallback to `fs::remove_dir_all`
    - Generate unique worktree paths using `{sha12}-{timestamp_ms}-{counter}`
    - _Requirements: 2.1, 2.8, 9.1, 9.2, 9.3, 9.4_

  - [x] 3.3 Write property test P19: Worktree Path Uniqueness
    - **Property 19: Worktree Path Uniqueness**
    - Generate pairs of `CommitId`, call `unique_worktree_path` twice (even with same commit), assert distinct paths
    - **Validates: Requirement 9.4**

  - [x] 3.4 Implement `ExecProbeAdapter` (`ProbePort::run`)
    - Build command from `ProbeSpec::Exec` or `ProbeSpec::Shell` variants
    - Spawn process with worktree as working directory, capture stdout/stderr
    - Poll with 50ms sleep intervals, enforce timeout, kill on timeout
    - Classify exit codes: 0→Pass, 125→Skip, other→Fail, timeout→Indeterminate
    - Populate all `ProbeObservation` fields
    - _Requirements: 2.2, 2.3, 2.4, 2.5, 2.6, 2.7_

  - [x] 3.5 Write property test P1: Exit Code Classification
    - **Property 1: Exit Code Classification**
    - Add `proptest` as dev-dependency to `faultline-probe-exec`
    - Generate `(Option<i32>, bool)` pairs, verify `classify` returns correct `ObservationClass`
    - **Validates: Requirements 2.3, 2.4, 2.5, 2.6**

  - [x] 3.6 Write property test P2: Observation Structural Completeness
    - **Property 2: Observation Structural Completeness**
    - Generate valid `ProbeSpec` + mock checkout, verify all `ProbeObservation` fields are populated
    - **Validates: Requirements 2.7, 4.7**

  - [x] 3.7 Implement `LocalizationSession` core logic
    - `new`: validate non-empty sequence, build `index_by_commit` map
    - `record`: insert observation by index, reject unknown commits
    - `next_probe`: probe boundaries first, then binary narrowing (median unobserved between pass/fail)
    - `outcome`: compute pass/fail boundaries, classify as FirstBad / SuspectWindow / Inconclusive
    - `has_observation`, `get_observation`, `observation_list`, `sequence`, `max_probes` accessors
    - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5, 3.6, 3.7, 3.8, 3.9_

  - [x] 3.8 Write property test P4: Binary Narrowing Selects Valid Midpoint
    - **Property 4: Binary Narrowing Selects Valid Midpoint**
    - Add `proptest` as dev-dependency to `faultline-localization`
    - Generate sequences of length 3–50, record pass at first and fail at last, verify `next_probe()` returns a commit strictly between boundaries with no existing observation
    - **Validates: Requirement 3.1**

  - [x] 3.9 Write property test P5: Adjacent Pass-Fail Yields FirstBad
    - **Property 5: Adjacent Pass-Fail Yields FirstBad**
    - Generate sequences, record pass at index i and fail at index i+1 with all between observed, verify `outcome()` returns `FirstBad` with correct `last_good` and `first_bad`
    - **Validates: Requirement 3.2**

  - [x] 3.10 Write property test P10: FirstBad Requires Direct Evidence
    - **Property 10: FirstBad Requires Direct Evidence**
    - Generate any session producing `FirstBad`, verify `last_good` has Pass observation and `first_bad` has Fail observation
    - **Validates: Requirements 3.9, 11.1**

  - [x] 3.11 Implement `FileRunStore` (`RunStorePort`)
    - `prepare_run`: create run directory keyed by request fingerprint, set `resumed` flag if exists, write `request.json`
    - `load_observations`: read `observations.json` or return empty vec
    - `save_observation`: upsert by commit ID, sort, rewrite `observations.json`
    - `save_report`: write `report.json`
    - _Requirements: 4.1, 4.2, 4.3, 4.5, 4.6_

  - [x] 3.12 Write property test P11: Run Store Round-Trip
    - **Property 11: Run Store Round-Trip**
    - Add `proptest` as dev-dependency to `faultline-store`
    - Generate random `ProbeObservation`, save then load, verify equivalence
    - Generate random `AnalysisReport`, save then read file and deserialize, verify equivalence
    - Generate random `AnalysisRequest`, prepare run then read `request.json`, verify equivalence
    - Use temp directories for isolation
    - **Validates: Requirements 4.2, 4.5, 4.6**

  - [x] 3.13 Write property test P12: Run Store Resumability
    - **Property 12: Run Store Resumability**
    - Generate request + observations, call `prepare_run` twice, verify second `RunHandle.resumed == true`
    - Verify `load_observations` on second handle returns all observations from first run
    - **Validates: Requirement 4.3**

  - [x] 3.14 Implement `ReportRenderer` JSON output (`analysis.json`)
    - Write `analysis.json` via `serde_json::to_string_pretty`
    - Create output directory if it doesn't exist
    - _Requirements: 6.1, 6.2, 6.3, 6.4_

  - [x] 3.15 Implement `FaultlineApp::localize` orchestration loop
    - Wire `HistoryPort`, `CheckoutPort`, `ProbePort`, `RunStorePort`, `SurfaceAnalyzer`
    - Prepare run, load cached observations, linearize history
    - Create `LocalizationSession`, replay cached observations
    - Ensure boundary validation (good=Pass, bad=Fail) with cached observation reuse
    - Binary narrowing loop: next_probe → checkout → probe → cleanup → save → record → check convergence
    - Compute changed paths and surface summary on boundary pair
    - Build and persist `AnalysisReport`
    - _Requirements: 3.1, 3.8, 4.4, 5.1, 10.1, 10.2, 10.3, 10.4, 10.5_

  - [x] 3.16 Write property test P9: Probe Count Respects Max Probes
    - **Property 9: Probe Count Respects Max Probes**
    - Add `proptest` as dev-dependency to `faultline-app`
    - Use mock port implementations (test doubles)
    - Generate small `max_probes` values (1–10), verify loop terminates within limit
    - **Validates: Requirement 3.8**

  - [x] 3.17 Write property test P20: Boundary Validation Rejects Mismatched Classes
    - **Property 20: Boundary Validation Rejects Mismatched Classes**
    - Use mock ports where good boundary returns Fail or bad boundary returns Pass
    - Verify `localize` returns `InvalidBoundary` error with expected/actual classes
    - **Validates: Requirements 10.1, 10.2, 10.3, 10.4**

- [x] 4. Checkpoint — Wave 1 complete (thin vertical slice works end-to-end)
  - Ensure `cargo test` passes for all crates. Ask the user if questions arise.

- [ ] 5. Wave 2 — Honest Localization: Skip, Timeout, SuspectWindow, Property Tests
  - [x] 5.1 Add unit test fixtures for localization edge cases
    - Timeout island scenario: one or more `Indeterminate` commits between boundaries → `SuspectWindow`
    - Non-monotonic predicate scenario: `Fail` before `Pass` in sequence → `SuspectWindow` with `NonMonotonicEvidence`, confidence low
    - All-revisions-untestable scenario: every intermediate commit is `Skip` or `Indeterminate`
    - _Requirements: 12.3, 12.4, 12.5, 12.6_

  - [x] 5.2 Write property test P6: Ambiguous Observations Yield SuspectWindow
    - **Property 6: Ambiguous Observations Yield SuspectWindow**
    - Generate sessions with Skip or Indeterminate between pass/fail boundaries
    - Verify `outcome()` returns `SuspectWindow` with `SkippedRevision` and/or `IndeterminateRevision` in reasons
    - **Validates: Requirements 3.3, 3.4**

  - [x] 5.3 Write property test P7: Non-Monotonic Evidence Yields Low Confidence
    - **Property 7: Non-Monotonic Evidence Yields Low Confidence**
    - Generate observation sets where Fail index < Pass index
    - Verify `outcome()` includes `NonMonotonicEvidence` and confidence == `Confidence::low().score`
    - **Validates: Requirement 3.5**

  - [x] 5.4 Write property test P8: Missing Boundary Yields Inconclusive
    - **Property 8: Missing Boundary Yields Inconclusive**
    - Generate sessions with only Pass (no Fail) or only Fail (no Pass)
    - Verify `outcome()` returns `Inconclusive` with `MissingPassBoundary` or `MissingFailBoundary`
    - **Validates: Requirements 3.6, 3.7**

  - [x] 5.5 Write property test P21: Monotonic Window Narrowing
    - **Property 21: Monotonic Window Narrowing**
    - Generate observation sequences, record one at a time, verify candidate window size never increases
    - **Validates: Requirement 11.2**

  - [ ] 5.6 Write property test P22: SuspectWindow Confidence Cap
    - **Property 22: SuspectWindow Confidence Cap**
    - Generate sessions producing `SuspectWindow`, verify confidence score < 95 (`Confidence::high().score`)
    - **Validates: Requirement 11.3**

  - [-] 5.7 Write property test P23: Observation Order Independence
    - **Property 23: Observation Order Independence**
    - Generate observation sets and `RevisionSequence`, record in multiple permutation orders, verify same `LocalizationOutcome`
    - **Validates: Requirement 11.4**

- [ ] 6. Checkpoint — Wave 2 complete (honest localization fully tested)
  - Ensure `cargo test -p faultline-localization` passes with all property and unit tests. Ask the user if questions arise.

- [ ] 7. Wave 3 — Changed-Surface Summary
  - [ ] 7.1 Implement `GitAdapter::changed_paths` (`HistoryPort::changed_paths`)
    - Compute changed paths via `git diff --name-status` between two commits
    - Parse tab-separated output into `PathChange` entries with correct `ChangeStatus`
    - Handle rename entries (use destination path)
    - _Requirements: 5.1_

  - [ ] 7.2 Implement `SurfaceAnalyzer::summarize`
    - Group paths into `SubsystemBucket` by top-level directory (`bucket_name`)
    - Assign surface kinds via `surface_kind`: source, tests, benchmarks, scripts, workflows, docs, build-script, lockfile, migrations, other
    - Identify execution surfaces via `is_execution_surface`: workflow files, build scripts, shell scripts
    - Produce `SurfaceSummary` with `total_changes`, `buckets`, `execution_surfaces`
    - _Requirements: 5.2, 5.3, 5.4, 5.5_

  - [ ] 7.3 Write property test P13: Surface Analysis Invariants
    - **Property 13: Surface Analysis Invariants**
    - Add `proptest` as dev-dependency to `faultline-surface`
    - Generate random `PathChange` vectors, verify: (a) `total_changes` == input length, (b) every path in exactly one bucket, (c) bucket names match top-level dirs, (d) valid surface kinds, (e) `execution_surfaces` subset of input
    - **Validates: Requirements 5.2, 5.3, 5.4**

- [ ] 8. Checkpoint — Wave 3 complete
  - Ensure `cargo test -p faultline-surface -p faultline-git` passes. Ask the user if questions arise.

- [ ] 9. Wave 4 — Operator UX: CLI Polish, Cleanup, Exit Codes
  - [ ] 9.1 Polish `faultline-cli` argument parsing and validation
    - Enforce mutual exclusion of `--cmd` / `--program` with clear error messages
    - Validate that at least one of `--cmd` or `--program` is provided
    - Parse `--kind` into `ProbeKind` with helpful error on invalid values
    - Wire `--first-parent` flag to `HistoryMode`
    - Wire `--max-probes` to `SearchPolicy`
    - _Requirements: 8.1, 8.2, 8.3, 8.4, 8.5, 8.6, 8.7_

  - [ ] 9.2 Implement CLI exit codes and output formatting
    - Exit 0 on successful run, print run ID, observation count, output dir, outcome summary to stdout
    - Exit 2 on any error, print diagnostic to stderr
    - _Requirements: 8.8, 8.9, 8.10_

  - [ ] 9.3 Add CLI unit tests for argument validation
    - Test: rejects both `--cmd` and `--program` simultaneously
    - Test: rejects neither `--cmd` nor `--program`
    - Test: `--help` output is stable and describes all flags
    - _Requirements: 8.3, 8.4, 8.11_

- [ ] 10. Checkpoint — Wave 4 complete
  - Ensure `cargo test -p faultline-cli` passes. Ask the user if questions arise.

- [ ] 11. Wave 5 — HTML Report and Artifact Polish
  - [ ] 11.1 Implement `ReportRenderer::render_html`
    - Generate self-contained HTML with inline CSS, no external dependencies
    - Display: run ID, outcome summary, probe fingerprint, history mode, observation timeline table, changed-surface buckets, changed paths list
    - HTML-escape all dynamic content via `escape_html` function
    - _Requirements: 7.1, 7.2, 7.3, 7.5_

  - [ ] 11.2 Write property test P16: HTML Contains Required Data Consistent with JSON
    - **Property 16: HTML Contains Required Data Consistent with JSON**
    - Add `proptest` as dev-dependency to `faultline-render`
    - Generate random `AnalysisReport`, render HTML, verify it contains run_id, outcome type, boundary SHAs, one `<tr>` per observation
    - **Validates: Requirements 7.2, 7.4, 11.5**

  - [ ] 11.3 Write property test P17: HTML Escaping Correctness
    - **Property 17: HTML Escaping Correctness**
    - Generate strings with `<`, `>`, `&`, `"`, `'`, verify `escape_html` replaces each with HTML entity
    - **Validates: Requirement 7.5**

  - [ ] 11.4 Write property test P18: HTML Is Self-Contained
    - **Property 18: HTML Is Self-Contained**
    - Generate random `AnalysisReport`, render HTML, verify no `<link>`, `<script>`, or `<img>` tags with `http://` or `https://` URLs
    - **Validates: Requirement 7.3**

- [ ] 12. Checkpoint — Wave 5 complete
  - Ensure `cargo test -p faultline-render` passes. Ask the user if questions arise.

- [ ] 13. Wave 6 — Integration Tests and Fixture Scenarios
  - [ ] 13.1 Add integration test: cached-resume scenario
    - Use mock ports to simulate a run that loads previously persisted observations
    - Verify no re-probing of cached commits, verify final outcome matches expected
    - _Requirements: 4.3, 4.4, 12.7_

  - [ ] 13.2 Add integration test: boundary validation with mock ports
    - Test: good boundary evaluates as Fail → `InvalidBoundary` error
    - Test: bad boundary evaluates as Pass → `InvalidBoundary` error
    - Test: cached boundary observations are reused (no re-probe)
    - _Requirements: 10.1, 10.2, 10.3, 10.4, 10.5_

  - [ ] 13.3 Add integration test: full localization loop with mock ports
    - Wire `FaultlineApp` with mock `HistoryPort`, `CheckoutPort`, `ProbePort`, `RunStorePort`
    - Verify end-to-end: linearize → boundary validation → binary narrowing → outcome → report
    - _Requirements: 3.1, 3.2, 3.8, 3.9_

- [ ] 14. Checkpoint — Wave 6 complete
  - Ensure `cargo test` passes for the entire workspace. Ask the user if questions arise.

- [ ] 15. Wave 7 — Package Metadata and Release Readiness
  - [ ] 15.1 Complete workspace `Cargo.toml` metadata
    - Ensure `version`, `edition`, `license`, `authors` are set for all crates via `[workspace.package]`
    - Verify each crate's `Cargo.toml` inherits workspace metadata
    - _Requirements: 13.1_

  - [ ] 15.2 Verify documentation and ADR completeness
    - Confirm `README.md` contains a working quickstart example (copy-paste runnable)
    - Confirm `docs/architecture.md` describes hexagonal layout and crate boundaries
    - Confirm ADR documents exist: `0001-hexagonal-architecture.md`, `0002-git-cli-and-disposable-checkouts.md`, `0003-honest-localization-outcomes.md`
    - _Requirements: 13.2, 13.3, 13.4_

  - [ ] 15.3 Verify CLI `--help` stability
    - Ensure `cargo run -p faultline-cli -- --help` produces stable output describing all flags with defaults and types
    - _Requirements: 13.5, 8.11_

- [ ] 16. Final checkpoint — All tests pass, release ready
  - Ensure `cargo test` passes for the entire workspace, `cargo build --release` succeeds, and `cargo run -p faultline-cli -- --help` produces expected output. Ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- Each task references specific requirements for traceability
- Checkpoints ensure incremental validation at each wave boundary
- Property tests validate universal correctness properties (P1–P23) from the design document
- Unit tests validate specific fixture scenarios from Requirement 12
- All property tests use `proptest` with minimum 100 iterations
- Mock port implementations (test doubles) are needed for `faultline-app` integration tests
- No tests require a real Git repository — all use synthetic fixtures or mock ports
