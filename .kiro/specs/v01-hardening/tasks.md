# Implementation Plan: faultline v0.1 Hardening

## Overview

Implements the hardening pass that takes the existing v0.1 scaffold to a releasable state. Changes are delta-only across the existing 12 crates — no new crates, no architectural changes. Waves follow the priority order: freeze contract → prove core → harden adapters → safe resumability → real fixtures → operator surface → dogfood. All code is Rust, tested with `proptest` for correctness properties (P24–P36) and unit tests for fixture scenarios.

## Tasks

- [ ] 1. Wave 0 — Freeze Contract: Type Changes, Code Vocabulary, Port Extension
  - [ ] 1.1 Add `MaxProbesExhausted` and `SignalTermination` variants to `AmbiguityReason` in `faultline-codes`
    - Add `MaxProbesExhausted` and `SignalTermination` to the `AmbiguityReason` enum
    - Add `Display` match arms for both: `"max probes exhausted"` and `"signal termination"`
    - _Requirements: 3.2, 5.1_

  - [ ] 1.2 Remove `edge_refine_threshold` from `SearchPolicy` in `faultline-types`
    - Remove the `edge_refine_threshold` field from `SearchPolicy`
    - Update `Default` impl to only set `max_probes: 64`
    - _Requirements: 3.1_

  - [ ] 1.3 Add `schema_version` to `AnalysisReport` in `faultline-types`
    - Add `pub schema_version: String` field to `AnalysisReport`
    - Default value is `"0.1.0"`
    - _Requirements: 1.4, 1.5, 1.6_

  - [ ] 1.4 Add diagnostic fields to `ProbeObservation` in `faultline-types`
    - Add `pub sequence_index: u64` (temporal probe order)
    - Add `pub signal_number: Option<i32>` (Unix signal that killed process)
    - Add `pub probe_command: String` (effective command for reproducibility)
    - Add `pub working_dir: String` (checkout path for reproducibility)
    - All new fields use `#[serde(default)]` for backward-compatible deserialization
    - _Requirements: 3.3, 5.1, 5.4_

  - [ ] 1.5 Add version metadata to `RunHandle` in `faultline-types`
    - Add `pub schema_version: String` (default `"0.1.0"`)
    - Add `pub tool_version: String` (from workspace Cargo.toml)
    - Use `#[serde(default)]` for backward compatibility
    - _Requirements: 6.4_

  - [ ] 1.6 Add `load_report` to `RunStorePort` in `faultline-ports`
    - Add `fn load_report(&self, run: &RunHandle) -> Result<Option<AnalysisReport>>` to the trait
    - _Requirements: 6.11_

  - [ ] 1.7 Fix all compilation errors from type changes across the workspace
    - Update all `SearchPolicy` construction sites to remove `edge_refine_threshold`
    - Update all `AnalysisReport` construction sites to include `schema_version: "0.1.0".into()`
    - Update all `ProbeObservation` construction sites to include `sequence_index: 0`, `signal_number: None`, `probe_command: String::new()`, `working_dir: String::new()`
    - Update all `RunHandle` construction sites to include `schema_version` and `tool_version`
    - Add `load_report` stub implementations to all `RunStorePort` implementors (`FileRunStore`, mock ports in tests)
    - Update all proptest `Arbitrary` strategies for changed types
    - _Requirements: 1.4, 3.1, 3.3, 5.1, 5.4, 6.4, 6.11_

- [ ] 2. Checkpoint — Wave 0 complete (contract frozen)
  - Ensure `cargo test` passes for the entire workspace. Ask the user if questions arise.

- [ ] 3. Wave 1 — Prove Core: Localization Engine Semantics
  - [ ] 3.1 Implement `MaxProbesExhausted` outcome in `LocalizationSession::outcome()` in `faultline-localization`
    - When `observations.len() >= policy.max_probes` and the window has not converged to an adjacent pass-fail pair, include `AmbiguityReason::MaxProbesExhausted` in the outcome's reasons list
    - Modify `outcome()` to check if max probes were exhausted and the result is `Inconclusive` or `SuspectWindow`
    - _Requirements: 3.2_

  - [ ] 3.2 Implement sequence-index tracking in `LocalizationSession` in `faultline-localization`
    - Add an internal `next_sequence_index: u64` counter initialized to 0
    - On each `record()` call, set `observation.sequence_index = self.next_sequence_index` then increment
    - Change `observation_list()` to return observations ordered by `sequence_index` (ascending)
    - _Requirements: 3.3_

  - [ ] 3.3 Remove all references to `edge_refine_threshold` in `faultline-localization`
    - Remove any code paths that reference `policy.edge_refine_threshold`
    - _Requirements: 3.1_

  - [ ] 3.4 Review and document `prop_monotonic_window_narrowing` in `faultline-localization`
    - Review the existing property test for correctness
    - Add a code comment documenting the review outcome (confirmed valid or strengthened)
    - _Requirements: 3.7_

  - [ ]* 3.5 Write property test P25: Max-Probe Exhaustion Produces Explicit Outcome
    - **Property 25: Max-Probe Exhaustion Produces Explicit Outcome**
    - Generate sequences of 5–30 commits, set `max_probes` to 2–5, record that many observations without convergence, verify `outcome()` includes `AmbiguityReason::MaxProbesExhausted`
    - **Validates: Requirement 3.2**

  - [ ]* 3.6 Write property test P26: Observation Sequence Order Preservation
    - **Property 26: Observation Sequence Order Preservation**
    - Generate random observation orderings, record in order, verify `observation_list()` returns by `sequence_index` with monotonically increasing values starting at 0
    - **Validates: Requirement 3.3**

- [ ] 4. Checkpoint — Wave 1 complete (localization semantics proven)
  - Ensure `cargo test -p faultline-localization` passes with all property and unit tests. Ask the user if questions arise.

- [ ] 5. Wave 2 — Harden Adapters: Git, Probe Executor
  - [ ] 5.1 Add environment validation to `GitAdapter::new()` in `faultline-git`
    - Add `verify_git_available()`: run `git --version`, return `FaultlineError::Git("git binary not found on PATH")` on failure
    - Add `verify_git_repo(path)`: run `git rev-parse --git-dir` in the repo root, return `FaultlineError::Git("not a git repository: {path}")` on failure
    - Call both checks at the start of `new()`
    - _Requirements: 4.1, 4.2_

  - [ ] 5.2 Add stale worktree cleanup to `GitAdapter::new()` in `faultline-git`
    - After creating `scratch_root`, scan for existing directories under `.faultline/scratch/`
    - For each stale directory, attempt `git worktree remove --force`, fallback to `fs::remove_dir_all`
    - Log warnings on failure but do not return errors
    - _Requirements: 4.3_

  - [ ] 5.3 Harden `cleanup_checkout` resilience in `faultline-git`
    - When both `git worktree remove --force` and `fs::remove_dir_all` fail, log a warning (eprintln) and return `Ok(())` so the original probe result is not masked
    - _Requirements: 4.4, 4.6_

  - [ ] 5.4 Implement signal-aware classification in `faultline-probe-exec`
    - After `wait_with_output`, extract signal number via `#[cfg(unix)] std::os::unix::process::ExitStatusExt::signal()`
    - On non-Unix, set `signal_number = None`
    - Update `classify` to accept `signal_number: Option<i32>`: if `exit_code` is `None` and `timed_out` is false and `signal_number` is `Some`, return `Indeterminate`
    - Populate `signal_number` on the `ProbeObservation`
    - _Requirements: 5.1, 5.2_

  - [ ] 5.5 Implement output truncation in `faultline-probe-exec`
    - Define `const DEFAULT_TRUNCATION_LIMIT: usize = 64 * 1024`
    - After capturing stdout/stderr, if length exceeds limit, truncate and append `"[truncated]"`
    - Save full output to `{run_dir}/{commit_sha}_stdout.log` and `_stderr.log` when truncated (requires passing run directory context or deferring to store layer)
    - _Requirements: 5.3_

  - [ ] 5.6 Populate `probe_command` and `working_dir` on `ProbeObservation` in `faultline-probe-exec`
    - Set `probe_command` to the effective command string (e.g., `"sh -c 'cargo test'"` or `"cargo test --lib"`)
    - Set `working_dir` to `checkout.path.display().to_string()`
    - _Requirements: 5.4_

  - [ ]* 5.7 Write property test P27: Signal-Aware Exit Code Classification
    - **Property 27: Signal-Aware Exit Code Classification**
    - Generate `(Option<i32>, bool, Option<i32>)` triples for `(exit_code, timed_out, signal_number)`
    - Verify classification: `Indeterminate` when timed_out, `Indeterminate` when no exit_code + signal, `Pass` for exit 0, `Skip` for exit 125, `Fail` for other non-zero, `Indeterminate` when no exit_code and no signal
    - **Validates: Requirements 5.1, 5.2**

  - [ ]* 5.8 Write property test P28: Observation Structural Completeness (Extended)
    - **Property 28: Observation Structural Completeness (Extended)**
    - Generate valid `ProbeSpec` + mock checkout, verify `probe_command` is non-empty and `working_dir` is non-empty in the resulting observation
    - **Validates: Requirements 5.4, extends v01-release-train Property 2**

- [ ] 6. Checkpoint — Wave 2 complete (adapters hardened)
  - Ensure `cargo test -p faultline-git -p faultline-probe-exec` passes. Ask the user if questions arise.

- [ ] 7. Wave 3 — Safe Resumability: Atomic Writes, Lock File, Store Enhancements
  - [ ] 7.1 Implement `atomic_write` helper in `faultline-store`
    - Write to `{target}.tmp` then `fs::rename` to target
    - Replace all `fs::write` calls in `FileRunStore` with `atomic_write`
    - _Requirements: 6.1_

  - [ ] 7.2 Implement lock file for single-writer guard in `faultline-store`
    - `prepare_run` creates `.lock` file containing `{pid}\n{timestamp}`
    - If `.lock` exists, read PID and check if process is alive (via `/proc/{pid}` on Linux or `kill(pid, 0)` on Unix)
    - Alive → return `FaultlineError::Store("run locked by process {pid}")`
    - Dead → stale lock, remove and re-acquire
    - Release lock on `save_report` or implement `Drop`
    - _Requirements: 6.2, 6.3_

  - [ ] 7.3 Implement `load_report` in `FileRunStore`
    - Read `report.json` from run directory, return `Ok(None)` if file doesn't exist
    - Deserialize and return `Ok(Some(report))`
    - _Requirements: 6.11_

  - [ ] 7.4 Change `save_observation` to preserve sequence-index order in `faultline-store`
    - Sort observations by `sequence_index` (ascending) instead of lexicographic commit hash
    - _Requirements: 6.5_

  - [ ] 7.5 Persist version metadata in `prepare_run` in `faultline-store`
    - Write `metadata.json` containing `{ "schema_version": "0.1.0", "tool_version": "{version}" }` to the run directory
    - Set `schema_version` and `tool_version` on the returned `RunHandle`
    - _Requirements: 6.4_

  - [ ]* 7.6 Write property test P29: Store Observation Sequence Order
    - **Property 29: Store Observation Sequence Order**
    - Generate observations with distinct `sequence_index` values, save via `save_observation`, load via `load_observations`, verify returned in ascending `sequence_index` order
    - **Validates: Requirements 3.3, 6.5**

  - [ ]* 7.7 Write property test P30: Version Metadata Persistence
    - **Property 30: Version Metadata Persistence**
    - Generate random `AnalysisRequest`, call `prepare_run`, read `metadata.json`, verify `schema_version == "0.1.0"` and `tool_version` matches workspace version
    - **Validates: Requirement 6.4**

  - [ ]* 7.8 Write property test P31: Report Load Round-Trip
    - **Property 31: Report Load Round-Trip**
    - Generate random `AnalysisReport`, `save_report` then `load_report`, verify `Some(report)` equals original
    - **Validates: Requirement 6.11**

  - [ ]* 7.9 Write property test P24: Schema Version Round-Trip
    - **Property 24: Schema Version Round-Trip**
    - Generate random `AnalysisReport` with non-empty `schema_version`, save/load via store, verify `schema_version` preserved
    - **Validates: Requirements 1.4, 1.6**

- [ ] 8. Checkpoint — Wave 3 complete (persistence safe)
  - Ensure `cargo test -p faultline-store` passes. Ask the user if questions arise.

- [ ] 9. Wave 4 — Real Fixtures: GitRepoBuilder and Fixture Scenarios
  - [ ] 9.1 Implement `GitRepoBuilder` in `faultline-fixtures`
    - Add `GitRepoBuilder` struct with `TempDir`, commit list
    - Add `FixtureCommit` struct with message and `Vec<FileOp>`
    - Add `FileOp` enum: `Write { path, content }`, `Delete { path }`, `Rename { from, to }`
    - Implement `new()` → init bare repo with initial config
    - Implement `commit(message, ops)` → apply file ops, `git add .`, `git commit`
    - Implement `merge(message, branch)` → create merge commit
    - Implement `build()` → return `FixtureRepo { dir, commits: Vec<CommitId> }`
    - Add `tempfile` as dev-dependency to `faultline-fixtures`
    - _Requirements: 7.1, 7.2, 7.3_

  - [ ] 9.2 Add fixture scenario: exact-first-bad-commit
    - Create a linear 5-commit repo via `GitRepoBuilder` where commit 3 introduces a failing change
    - Test end-to-end with `GitAdapter::linearize` + `LocalizationSession`
    - Verify `FirstBad` outcome with correct boundary pair
    - _Requirements: 7.4_

  - [ ] 9.3 Add fixture scenario: first-parent-merge-history
    - Create a repo with merge commits via `GitRepoBuilder`
    - Verify `--first-parent` produces a different linearization than ancestry-path
    - _Requirements: 7.8_

  - [ ] 9.4 Add fixture scenario: rename-and-delete
    - Create a repo where files are renamed and deleted between boundary commits
    - Verify `GitAdapter::changed_paths` returns correct `PathChange` entries
    - _Requirements: 7.9_

  - [ ] 9.5 Add fixture scenario: invalid-boundaries
    - Create a repo where the good commit is not an ancestor of the bad commit
    - Verify `GitAdapter::linearize` returns an error
    - _Requirements: 7.10_

  - [ ] 9.6 Add fixture scenarios for localization edge cases (skipped, timed-out, non-monotonic)
    - Skipped-midpoint: midpoint classified as Skip → `SuspectWindow`
    - Timed-out-midpoint: midpoint classified as Indeterminate → `SuspectWindow`
    - Non-monotonic: Fail before Pass → low confidence
    - These use `RevisionSequenceBuilder` + `LocalizationSession` (no real Git needed)
    - _Requirements: 7.5, 7.6, 7.7_

  - [ ] 9.7 Add fixture scenario: interrupted-run-and-resume
    - Pre-populate a `FileRunStore` run directory with partial observations
    - Resume via `FaultlineApp::localize`, verify no re-probing of cached commits
    - _Requirements: 7.11_

  - [ ] 9.8 Add snapshot tests for `analysis.json` and HTML report
    - Create a canonical fixture, render via `ReportRenderer`, compare JSON output against golden file
    - Compare HTML output against golden file (or verify key structural elements)
    - _Requirements: 7.12, 7.13_

- [ ] 10. Checkpoint — Wave 4 complete (real fixtures in place)
  - Ensure `cargo test -p faultline-fixtures -p faultline-git -p faultline-app` passes. Ask the user if questions arise.

- [ ] 11. Wave 5 — Operator Surface: CLI Polish, Exit Codes, HTML Enhancements
  - [ ] 11.1 Implement `OperatorCode`-based exit codes in `faultline-cli`
    - Add `outcome_to_operator_code` mapping: `FirstBad` → `Success`, `SuspectWindow` → `SuspectWindow`, `Inconclusive` → `Inconclusive`
    - Add `exit_code_for_operator_code`: `Success` → 0, `SuspectWindow` → 1, `ExecutionError` → 2, `Inconclusive` → 3, `InvalidInput` → 4
    - Replace the current `std::process::exit(2)` catch-all with mapped exit codes
    - _Requirements: 8.1_

  - [ ] 11.2 Add new CLI flags: `--resume`, `--force`, `--fresh`, `--no-render`, `--shell`, `--env`
    - `--resume`: explicit resume (default behavior)
    - `--force`: discard cached observations
    - `--fresh`: delete entire run directory
    - `--no-render`: skip HTML generation
    - `--shell <shell_kind>`: select shell for `--cmd` (sh, cmd, powershell)
    - `--env <KEY=VALUE>`: repeatable env var injection
    - Enforce mutual exclusion: `--resume` + `--force`, `--resume` + `--fresh`, `--force` + `--fresh` → error
    - Parse `--env` values, reject missing `=` separator
    - Parse `--shell`, reject unknown shell kinds
    - _Requirements: 5.5, 5.6, 6.6, 6.7, 6.8, 6.9, 6.10, 8.4_

  - [ ] 11.3 Enhance CLI terminal summary output
    - Print: run ID, observation count, output directory, artifact paths (`analysis.json`, `index.html` if rendered), history mode, outcome type, boundary commits, confidence, ambiguity reasons
    - On `InvalidBoundary` error, print which boundary failed, expected class, observed class
    - _Requirements: 8.2, 8.5, 8.6, 8.7_

  - [ ] 11.4 Wire `--env` and `--shell` into `ProbeSpec` construction in `faultline-cli`
    - Pass `--env` pairs into `ProbeSpec::Exec.env` or as environment for `ProbeSpec::Shell`
    - Map `--shell` value to `ShellKind` variant
    - _Requirements: 5.5, 5.6, 5.7_

  - [ ] 11.5 Enhance HTML report with visual outcome distinction in `faultline-render`
    - Add CSS classes: `outcome-firstbad` (green border), `outcome-suspect` (amber border), `outcome-inconclusive` (red border)
    - Wrap outcome summary in a `<div>` with the appropriate class
    - _Requirements: 8.8_

  - [ ] 11.6 Add ambiguity reason badges to HTML report in `faultline-render`
    - Render each `AmbiguityReason` as a `<span class="badge badge-{reason}">` next to the outcome summary
    - _Requirements: 8.9_

  - [ ] 11.7 Implement temporal observation timeline in HTML report in `faultline-render`
    - Order observation table rows by `sequence_index` (ascending) instead of commit order
    - Add color-coded row backgrounds: Pass=green, Fail=red, Skip=gray, Indeterminate=yellow
    - _Requirements: 8.10_

  - [ ] 11.8 Add execution surface separation in HTML report in `faultline-render`
    - Render `execution_surfaces` in a separate highlighted section from general changed paths
    - _Requirements: 8.11_

  - [ ] 11.9 Add log file links in HTML report in `faultline-render`
    - When per-probe log files exist (from output truncation), render relative `<a>` links to them
    - _Requirements: 8.12_

  - [ ]* 11.10 Write property test P32: OperatorCode Exit Code Mapping
    - **Property 32: OperatorCode Exit Code Mapping**
    - Generate all `LocalizationOutcome` variants, verify: `FirstBad` → 0, `SuspectWindow` → 1, `Inconclusive` → 3, and all exit codes are distinct
    - **Validates: Requirement 8.1**

  - [ ]* 11.11 Write property test P33: HTML Outcome Visual Distinction and Ambiguity Badges
    - **Property 33: HTML Outcome Visual Distinction and Ambiguity Badges**
    - Generate reports with each outcome type, verify distinct CSS classes present; for `SuspectWindow`/`Inconclusive` with reasons, verify each reason appears as a badge element
    - **Validates: Requirements 8.8, 8.9**

  - [ ]* 11.12 Write property test P34: HTML Temporal Observation Order
    - **Property 34: HTML Temporal Observation Order**
    - Generate reports with multiple observations having distinct `sequence_index` values, verify HTML `<tr>` rows appear in ascending `sequence_index` order
    - **Validates: Requirement 8.10**

  - [ ]* 11.13 Write property test P35: HTML Execution Surface Separation
    - **Property 35: HTML Execution Surface Separation**
    - Generate reports with non-empty `execution_surfaces`, verify a separate HTML section/container exists for execution surfaces
    - **Validates: Requirement 8.11**

  - [ ]* 11.14 Write property test P36: CLI Help Flag Completeness
    - **Property 36: CLI Help Flag Completeness**
    - Verify `--help` output contains all flag names: `--resume`, `--force`, `--fresh`, `--no-render`, `--shell`, `--env`, plus all pre-existing flags
    - **Validates: Requirement 9.6**

  - [ ] 11.15 Add CLI unit tests for new flag validation
    - Test: `--force` + `--resume` rejected
    - Test: `--fresh` + `--resume` rejected
    - Test: `--env KEY=VALUE` parsed correctly
    - Test: `--env` missing `=` rejected
    - Test: `--shell unknown` rejected
    - Test: `--no-render` skips HTML (verify only `analysis.json` produced)
    - Test: exit code 0 for `FirstBad`, 1 for `SuspectWindow`, 3 for `Inconclusive`
    - _Requirements: 6.6, 6.7, 6.8, 8.1, 8.4_

- [ ] 12. Checkpoint — Wave 5 complete (operator surface polished)
  - Ensure `cargo test -p faultline-cli -p faultline-render` passes. Ask the user if questions arise.

- [ ] 13. Wave 6 — App Orchestrator Updates and Integration
  - [ ] 13.1 Update `FaultlineApp::localize` to set `schema_version` on `AnalysisReport`
    - Set `schema_version: "0.1.0".into()` when constructing the report
    - _Requirements: 1.4_

  - [ ] 13.2 Update `FaultlineApp::localize` to handle `MaxProbesExhausted`
    - When the narrowing loop ends because `probe_count >= max_probes` and the session has not converged, the session's `outcome()` now includes `MaxProbesExhausted` — verify this is wired correctly
    - _Requirements: 3.2_

  - [ ] 13.3 Wire `--force` and `--fresh` behavior through `FaultlineApp`
    - `--force`: clear cached observations before starting the loop (or pass a flag to `prepare_run`)
    - `--fresh`: delete the run directory before `prepare_run`
    - `--no-render`: skip `ReportRenderer::render` call
    - _Requirements: 6.7, 6.8, 8.4_

  - [ ] 13.4 Update integration tests in `faultline-app` for new type fields
    - Update all mock port implementations to include new fields (`schema_version`, `tool_version`, `sequence_index`, etc.)
    - Update all `AnalysisReport` assertions to check `schema_version`
    - _Requirements: 1.4, 3.3_

- [ ] 14. Checkpoint — Wave 6 complete (app orchestrator updated)
  - Ensure `cargo test -p faultline-app` passes. Ask the user if questions arise.

- [ ] 15. Wave 7 — Dogfood and Release Readiness
  - [ ] 15.1 Add CI configuration (`.github/workflows/ci.yml`)
    - GitHub Actions workflow for Linux: `cargo build`, `cargo test`, `cargo fmt --check`, `cargo clippy -- -D warnings`
    - _Requirements: 2.5_

  - [ ] 15.2 Add workspace-level smoke test
    - Create `tests/smoke.rs` that builds a real Git repo via `GitRepoBuilder`, runs the CLI binary via `std::process::Command`, verifies exit code 0, verifies `analysis.json` and `index.html` exist, verifies `analysis.json` contains `schema_version`
    - _Requirements: 2.7_

  - [ ] 15.3 Verify and fix `cargo fmt --check` and `cargo clippy` compliance
    - Run `cargo fmt --check` and fix any violations
    - Run `cargo clippy` and fix any warnings
    - _Requirements: 2.3, 2.4_

  - [ ] 15.4 Update BUILDING.md with accurate build prerequisites and commands
    - Document Rust stable requirement, `git` on PATH, build/test/fmt/clippy commands
    - _Requirements: 2.6_

  - [ ] 15.5 Verify naming consistency across all artifacts
    - Ensure CLI binary name, help text, error messages, HTML title, default output directory all use `faultline`
    - Ensure README, BUILDING.md, and `--help` are consistent
    - _Requirements: 1.1, 1.2, 1.3_

  - [ ] 15.6 Add CLI `--help` version display from workspace Cargo.toml
    - Add `#[command(version)]` to the `Cli` struct so `--help` and `--version` show the workspace version
    - _Requirements: 1.7_

  - [ ] 15.7 Update README with quickstart referencing real artifacts and document packaging
    - Include example artifact references or commit SHAs
    - Document `SearchPolicy` defaults (max_probes=64, timeout, truncation)
    - Document packaging decision (source-first or tagged releases)
    - _Requirements: 9.1, 9.2, 9.3, 9.4_

  - [ ] 15.8 Add release workflow or document source-only releases
    - Either add `.github/workflows/release.yml` that builds and attaches binaries on tag, or document in README that releases are source-only
    - _Requirements: 9.5_

- [ ] 16. Final checkpoint — All tests pass, release ready
  - Ensure `cargo test` passes for the entire workspace, `cargo build --release` succeeds, `cargo fmt --check` reports no violations, `cargo clippy` reports no warnings, and `cargo run -p faultline-cli -- --help` produces expected output with all new flags. Ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- Each task references specific requirements for traceability
- Checkpoints ensure incremental validation at each wave boundary
- Property tests validate universal correctness properties (P24–P36) from the design document
- Unit tests validate specific fixture scenarios from Requirement 7
- All property tests use `proptest` with minimum 100 iterations
- Wave 0 is the most disruptive (type changes ripple across the workspace) — the checkpoint ensures everything compiles before proceeding
- No new crates are added; all changes are internal to existing crates
