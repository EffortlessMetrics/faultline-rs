# Requirements Document

## Introduction

This document captures the hardening requirements for faultline v0.1 — taking the existing scaffold (all v01-release-train tasks complete) to a trustworthy, releasable state. The v0.1 scaffold has 12 crates following hexagonal architecture, a working localization engine, persistence layer, CLI, and report renderer. This hardening pass addresses eight phases: freezing the public contract, proving the build, correcting localization semantics, hardening Git and probe adapters, making resumability safe, replacing toy fixtures with real Git repo fixtures, polishing the operator surface, and dogfooding for release.

Key observations about the current state that motivate these requirements:
- `edge_refine_threshold` exists in `SearchPolicy` but is unused by the localization engine
- `OperatorCode` exists in `faultline-codes` but the CLI does not map outcomes to exit codes
- `FileRunStore::save_observation` sorts observations lexicographically by commit hash instead of preserving insertion (sequence) order
- `faultline-fixtures` provides only a synthetic `RevisionSequenceBuilder`, not a real Git repo fixture harness
- The `prop_monotonic_window_narrowing` property test previously failed with a counterexample — the fix needs human review
- `AnalysisReport` has no schema version field
- Persistence writes are not atomic (no temp-file-plus-rename pattern)
- No lock file or single-writer guard exists for concurrent run protection
- The CLI has no `--resume`, `--force`, or `--fresh` flags
- The probe adapter does not distinguish timeout from signal termination from ordinary nonzero exits
- Large stdout/stderr from probes is stored in full with no truncation or overflow handling

## Glossary

- **Operator**: The human user invoking faultline from the command line.
- **CLI**: The `faultline-cli` binary, the sole operator-facing entry point.
- **Localization_Engine**: The pure-domain regression-window search engine (`faultline-localization`, specifically `LocalizationSession`).
- **Probe_Executor**: The adapter that runs the operator's predicate command in a checked-out worktree (`faultline-probe-exec`, specifically `ExecProbeAdapter`).
- **Run_Store**: The filesystem-backed persistence layer for observations and run metadata (`faultline-store`, specifically `FileRunStore`).
- **Git_Adapter**: The adapter that shells out to the system `git` binary for history linearization, worktree management, and diff operations (`faultline-git`, specifically `GitAdapter`).
- **Report_Renderer**: The adapter that writes the canonical JSON artifact and the derived HTML report (`faultline-render`, specifically `ReportRenderer`).
- **App_Orchestrator**: The application-layer use-case coordinator (`faultline-app`, specifically `FaultlineApp`).
- **Fixture_Harness**: The test infrastructure crate for constructing real or synthetic Git repositories and predicate outcomes (`faultline-fixtures`).
- **Observation**: A recorded result of running the predicate against a single commit.
- **Scratch_Worktree**: A disposable Git linked worktree created under `.faultline/scratch/`.
- **Run_Directory**: A persisted directory under `.faultline/runs/` keyed by request fingerprint.
- **Analysis_Report**: The canonical output artifact containing request, sequence, observations, outcome, changed paths, and surface summary.
- **SearchPolicy**: Configuration struct controlling localization behavior (`max_probes`, `edge_refine_threshold`).
- **OperatorCode**: An enum in `faultline-codes` mapping outcome semantics to process exit codes.
- **Schema_Version**: A version identifier embedded in persisted artifacts to detect format incompatibilities.
- **Sequence_Index**: A monotonically increasing integer assigned to each observation in the order it was recorded, preserving temporal ordering.

## Requirements

### Requirement 1: Freeze Public Identity and Contract

**User Story:** As a maintainer, I want the product name, binary name, crate names, output directory defaults, HTML title, documentation, and report schema version to be frozen and consistent, so that the v0.1 release has a stable public identity.

#### Acceptance Criteria

1. THE CLI SHALL use the binary name `faultline` in all help text, error messages, and artifact titles.
2. THE workspace Cargo.toml SHALL list all 12 crate members with names prefixed `faultline-`.
3. THE README, BUILDING.md, CLI `--help` output, HTML report title, and default output directory name SHALL all use the name `faultline` consistently.
4. THE Analysis_Report SHALL include a `schema_version` field containing a semantic version string (starting at `"0.1.0"`).
5. WHEN the Analysis_Report structure changes in a backward-incompatible way, THE `schema_version` field SHALL be incremented.
6. THE Run_Store SHALL persist the `schema_version` alongside the report in `report.json`.
7. THE CLI `--help` output SHALL display the tool version from the workspace `Cargo.toml`.

### Requirement 2: Build Proof and Project Hygiene

**User Story:** As a maintainer, I want the workspace to build cleanly, pass all tests, and satisfy formatting and lint checks on a fresh clone, so that contributors can trust the build and CI catches regressions.

#### Acceptance Criteria

1. WHEN `cargo build` is run on a fresh clone, THE workspace SHALL compile without errors on stable Rust.
2. WHEN `cargo test` is run, THE workspace SHALL pass all unit tests, property tests, and integration tests.
3. WHEN `cargo fmt --check` is run, THE workspace SHALL report no formatting violations.
4. WHEN `cargo clippy` is run, THE workspace SHALL report no warnings at the default lint level.
5. THE repository SHALL contain a CI configuration for at least Linux that runs `cargo build`, `cargo test`, `cargo fmt --check`, and `cargo clippy`.
6. THE BUILDING.md SHALL accurately describe the build prerequisites, build commands, and test commands that match the actual workspace configuration.
7. THE repository SHALL contain a smoke test that runs the CLI binary against a fixture repository and verifies that `analysis.json` and `index.html` are produced in the output directory.

### Requirement 3: Localization Engine Semantic Correctness

**User Story:** As a developer, I want the localization engine to have fully defined, testable semantics for edge refinement, max-probe exhaustion, observation ordering, non-monotonic evidence, and confidence scoring, so that the engine's behavior can be trusted without reading the implementation.

#### Acceptance Criteria

1. WHEN `SearchPolicy.edge_refine_threshold` is set, THE Localization_Engine SHALL use the threshold to switch from binary narrowing to linear edge refinement when the candidate window size falls below the threshold; IF the threshold is not used by the engine, THEN THE `edge_refine_threshold` field SHALL be removed from `SearchPolicy`.
2. WHEN the maximum probe count is reached before convergence, THE Localization_Engine SHALL return an explicit outcome with `AmbiguityReason::MaxProbesExhausted` instead of silently stopping.
3. THE Localization_Engine SHALL preserve observations in sequence order using a Sequence_Index, so that the observation list reflects the temporal order of probing rather than lexicographic commit hash order.
4. WHEN non-monotonic evidence is detected (a Fail observation at a lower sequence index than a Pass observation), THE Localization_Engine SHALL degrade the outcome to `SuspectWindow` with `AmbiguityReason::NonMonotonicEvidence` and `Confidence::low()`, and the degradation rules SHALL be deterministic and documented.
5. THE Localization_Engine SHALL derive confidence scores from explicit, deterministic rules based on evidence quality (number of observations, presence of ambiguity reasons, window size), or THE confidence scoring SHALL use the existing fixed buckets (`high`/`medium`/`low`) with rules that are documented and tested.
6. THE Localization_Engine SHALL include property tests that verify: (a) the window never expands between successive probes, (b) FirstBad outcomes have direct Pass and Fail evidence at the boundary, (c) observation order independence holds for the same observation set, (d) max-probe exhaustion produces an explicit outcome.
7. THE `prop_monotonic_window_narrowing` property test SHALL be reviewed and either confirmed as a valid fix or the property SHALL be strengthened, with the review outcome documented.

### Requirement 4: Harden the Git Adapter

**User Story:** As an operator, I want the Git adapter to validate its environment, handle stale state, and clean up reliably, so that repeated runs do not leave broken scratch directories or produce confusing errors.

#### Acceptance Criteria

1. WHEN the Git_Adapter is constructed, THE Git_Adapter SHALL verify that the target path is a Git repository by running `git rev-parse --git-dir` and SHALL return a clear error if the path is not a Git repository.
2. WHEN the Git_Adapter is constructed, THE Git_Adapter SHALL verify that the `git` binary is available on the system PATH and SHALL return a clear error if `git` is not found.
3. WHEN a Scratch_Worktree directory exists from a previous interrupted run, THE Git_Adapter SHALL detect and remove stale worktree directories under `.faultline/scratch/` before creating new worktrees.
4. IF probe execution fails hard (panic, signal, or adapter error), THEN THE Git_Adapter SHALL still attempt worktree cleanup via `git worktree remove --force` with fallback to directory deletion.
5. THE Git_Adapter SHALL include tests for: ancestry-path linearization, first-parent linearization, rename handling in `changed_paths`, deleted file handling in `changed_paths`, and merge-heavy history linearization.
6. WHEN `git worktree remove --force` fails and the fallback directory deletion also fails, THE Git_Adapter SHALL log a warning and continue rather than returning an error that masks the original probe result.

### Requirement 5: Harden the Probe Executor

**User Story:** As an operator, I want the probe executor to clearly distinguish termination causes, handle large output, and include diagnostic metadata in reports, so that probe results are reproducible and diagnosable.

#### Acceptance Criteria

1. WHEN the predicate process is killed by a signal (e.g., SIGKILL, SIGTERM) without a timeout, THE Probe_Executor SHALL classify the observation as `Indeterminate` and record the signal number in the observation.
2. WHEN the predicate process exceeds the timeout, THE Probe_Executor SHALL classify the observation as `Indeterminate` and set `timed_out` to true, distinguishing timeout termination from signal termination.
3. WHEN the predicate produces stdout or stderr exceeding a configurable truncation limit (default 64 KiB), THE Probe_Executor SHALL truncate the captured output in the observation and save the full output to a separate log file in the Run_Directory.
4. THE Probe_Executor SHALL include the effective probe command string and working directory path in each observation for diagnostic reproducibility.
5. THE CLI SHALL accept `--shell <shell_kind>` to select the shell used for `--cmd` predicates, supporting at least `sh`, `cmd`, and `powershell`.
6. THE CLI SHALL accept `--env <KEY=VALUE>` (repeatable) to inject environment variables into the predicate execution environment.
7. WHEN the Probe_Executor runs a predicate, THE Probe_Executor SHALL pass any CLI-specified environment variables to the child process in addition to the inherited environment.

### Requirement 6: Safe Resumability and Persistence

**User Story:** As an operator, I want interrupted runs to resume safely, concurrent invocations to fail cleanly, and reruns with the same request to do the obvious thing, so that persistence is trustworthy.

#### Acceptance Criteria

1. WHEN the Run_Store writes `observations.json`, `request.json`, or `report.json`, THE Run_Store SHALL write to a temporary file in the same directory and then atomically rename the temporary file to the target path.
2. WHEN a localization run begins, THE Run_Store SHALL create a lock file (e.g., `.lock`) in the Run_Directory; IF the lock file already exists and is held by another process, THEN THE Run_Store SHALL return an error indicating that another process is using the same run.
3. WHEN a localization run completes or fails, THE Run_Store SHALL release the lock file.
4. THE Run_Store SHALL persist the tool version and Schema_Version in the Run_Directory metadata.
5. THE Run_Store SHALL preserve observations in Sequence_Index order rather than sorting by commit hash string.
6. THE CLI SHALL accept `--resume` to continue an interrupted run using cached observations (current default behavior made explicit).
7. THE CLI SHALL accept `--force` to discard any existing cached observations and start a fresh run.
8. THE CLI SHALL accept `--fresh` to delete the entire Run_Directory for the matching fingerprint and start from scratch.
9. WHEN `--resume` is specified and no prior run exists, THE CLI SHALL proceed as a fresh run without error.
10. WHEN neither `--resume`, `--force`, nor `--fresh` is specified, THE CLI SHALL default to resume behavior (reuse cached observations if they exist).
11. THE Run_Store SHALL provide a `load_report` method that loads a previously persisted `Analysis_Report` from the Run_Directory, so that repeated export (e.g., re-rendering HTML) can be performed without re-running localization.

### Requirement 7: Real Git Repository Fixture Harness

**User Story:** As a developer, I want a fixture harness that creates real Git repositories with real commits, so that adapter-level tests exercise actual Git behavior instead of only synthetic data structures.

#### Acceptance Criteria

1. THE Fixture_Harness SHALL provide a builder API that creates a temporary Git repository with a configurable sequence of commits, each with specified file contents.
2. THE Fixture_Harness SHALL support creating commits that add, modify, delete, and rename files.
3. THE Fixture_Harness SHALL support creating merge commits for testing first-parent and merge-heavy history scenarios.
4. THE test suite SHALL include a fixture scenario for exact-first-bad-commit: a linear history where a single pass-to-fail transition exists, tested end-to-end with the Git_Adapter.
5. THE test suite SHALL include a fixture scenario for skipped-midpoint: a linear history where the midpoint commit is classified as Skip, tested with the Localization_Engine.
6. THE test suite SHALL include a fixture scenario for timed-out-midpoint: a linear history where one or more commits are classified as Indeterminate.
7. THE test suite SHALL include a fixture scenario for non-monotonic-evidence: a history where a Fail observation precedes a Pass observation.
8. THE test suite SHALL include a fixture scenario for first-parent-merge-history: a repository with merge commits where `--first-parent` produces a different linearization than ancestry-path.
9. THE test suite SHALL include a fixture scenario for rename-and-delete: a repository where files are renamed and deleted between the boundary commits, verifying `changed_paths` correctness.
10. THE test suite SHALL include a fixture scenario for invalid-boundaries: a repository where the good commit is not an ancestor of the bad commit.
11. THE test suite SHALL include a fixture scenario for interrupted-run-and-resume: a run that is interrupted (simulated by pre-populating the Run_Directory with partial observations) and then resumed.
12. THE test suite SHALL include snapshot tests for `analysis.json` output against canonical fixture scenarios.
13. THE test suite SHALL include snapshot or golden tests for the HTML report against canonical fixture scenarios.

### Requirement 8: Operator Surface Polish

**User Story:** As an operator, I want clear exit codes, a concise terminal summary, all necessary CLI flags, and an HTML report that is immediately useful for triage, so that I can run the tool once and know what to do next.

#### Acceptance Criteria

1. THE CLI SHALL map `OperatorCode` values to process exit codes: `Success` → 0, `SuspectWindow` → 1, `Inconclusive` → 3, `InvalidInput` → 4, `ExecutionError` → 2.
2. WHEN the run completes, THE CLI SHALL print a concise terminal summary to stdout containing: run ID, observation count, output directory path, outcome type, boundary commits (if any), confidence score, and ambiguity reasons (if any).
3. THE CLI SHALL accept `--max-probes <n>` to cap probe executions (already exists, default 64).
4. THE CLI SHALL accept `--no-render` to skip HTML report generation while still producing `analysis.json`.
5. WHEN the run completes, THE CLI SHALL print the exact file paths of produced artifacts (`analysis.json`, `index.html` if rendered).
6. WHEN the run completes, THE CLI SHALL print the resolved history mode (ancestry-path or first-parent) in the summary.
7. WHEN boundary validation fails, THE CLI SHALL print a clear message stating which boundary (good or bad) failed, what class was expected, and what class was observed.
8. THE HTML report SHALL visually distinguish between an exact boundary (FirstBad) and a suspect window (SuspectWindow) using different styling or layout.
9. THE HTML report SHALL render ambiguity reasons as visible badges or tags next to the outcome summary.
10. THE HTML report SHALL include an observation timeline that shows probes in temporal order with pass/fail/skip/indeterminate color coding.
11. THE HTML report SHALL highlight execution surfaces (workflow files, build scripts, shell scripts) separately from other changed paths.
12. WHEN per-probe log files are stored (from Requirement 5.3), THE HTML report SHALL render links to the log files relative to the output directory.
13. THE HTML report SHALL be readable enough to paste into a ticket or postmortem thread without additional formatting.

### Requirement 9: Dogfood and Release Readiness

**User Story:** As a maintainer, I want to run faultline against real regressions, save the artifacts as examples, tune defaults, and prepare packaging, so that the v0.1 release is trustworthy and documented from real usage.

#### Acceptance Criteria

1. THE repository SHALL contain at least one example artifact set (analysis.json and index.html) generated from a real regression in the faultline repository or a known test repository.
2. THE README SHALL include a quickstart example that references real commit SHAs from the example artifact, so that operators can see what real output looks like.
3. THE SearchPolicy defaults SHALL be reviewed and documented: max_probes default (currently 64), timeout behavior, and output truncation limits.
4. THE README SHALL document the packaging decision: source-first only, tagged releases, or prebuilt binaries.
5. WHEN a tagged release is created, THE repository SHALL contain a release workflow (GitHub Actions or equivalent) that builds the binary and attaches it to the release, or THE README SHALL document that releases are source-only.
6. THE CLI `--help` output SHALL be stable, complete, and describe all flags added in this hardening pass (including `--shell`, `--env`, `--resume`, `--force`, `--fresh`, `--no-render`).

