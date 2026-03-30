# Requirements Document

## Introduction

This document captures the requirements for faultline v0.1 — a local-first regression archaeologist for Git repositories. Given a known-good commit, a known-bad commit, and a predicate command the operator already trusts, faultline narrows the regression window and produces portable JSON and HTML artifacts showing where to investigate. The v0.1 scope is deliberately narrow: one local repo, explicit boundaries, exec-style predicates, honest localization outcomes, coarse changed-surface bucketing, cached/resumable runs, and clean scratch worktree lifecycle.


## Glossary

- **Operator**: The human user invoking faultline from the command line.
- **CLI**: The `faultline-cli` binary, the sole operator-facing entry point.
- **Localization_Engine**: The pure-domain regression-window search engine (`faultline-localization`).
- **Probe_Executor**: The adapter that runs the operator's predicate command in a checked-out worktree (`faultline-probe-exec`).
- **Run_Store**: The filesystem-backed persistence layer for observations and run metadata (`faultline-store`).
- **Git_Adapter**: The adapter that shells out to the system `git` binary for history linearization, worktree management, and diff operations (`faultline-git`).
- **Surface_Analyzer**: The domain module that buckets changed paths into coarse subsystem categories (`faultline-surface`).
- **Report_Renderer**: The adapter that writes the canonical JSON artifact and the derived HTML report (`faultline-render`).
- **App_Orchestrator**: The application-layer use-case coordinator that wires ports, drives the localization loop, and produces the final report (`faultline-app`).
- **Predicate**: An operator-supplied shell command or exec-style program whose exit code classifies a revision as pass, fail, skip, or indeterminate.
- **Observation**: A recorded result of running the Predicate against a single commit, including exit code, class, stdout, stderr, and duration.
- **Revision_Sequence**: The linearized, ordered list of commits between the good and bad boundaries (inclusive).
- **Scratch_Worktree**: A disposable Git linked worktree created under `.faultline/scratch/` for isolated predicate execution.
- **Run_Directory**: A persisted directory under `.faultline/runs/` keyed by request fingerprint, holding observations, request metadata, and the final report.
- **Analysis_Report**: The canonical output artifact containing the request, revision sequence, observations, localization outcome, changed paths, and surface summary.
- **Localization_Outcome**: One of three result types: FirstBad (exact boundary), SuspectWindow (ambiguous range with reasons), or Inconclusive (insufficient evidence).
- **Observation_Class**: One of Pass, Fail, Skip, or Indeterminate, derived from predicate exit codes and timeout status.
- **Ambiguity_Reason**: A diagnostic tag explaining why a localization result is not exact (e.g., NonMonotonicEvidence, SkippedRevision, IndeterminateRevision).
- **Subsystem_Bucket**: A coarse grouping of changed file paths by top-level directory and surface kind (source, tests, docs, workflows, scripts, build, lockfiles, benchmarks).
- **Probe_Fingerprint**: A stable hash of the ProbeSpec, used to key cached observations so that different predicates do not share cache entries.

## Requirements

### Requirement 1: History Linearization

**User Story:** As an Operator, I want faultline to linearize the commit history between my known-good and known-bad revisions, so that the Localization_Engine has an ordered sequence of candidates to probe.

#### Acceptance Criteria

1. WHEN the Operator provides `--good` and `--bad` revision specs, THE Git_Adapter SHALL resolve each spec to a full commit SHA using `git rev-parse --verify`.
2. WHEN both revisions are resolved, THE Git_Adapter SHALL verify that the good commit is an ancestor of the bad commit using `git merge-base --is-ancestor`.
3. IF the good commit is not an ancestor of the bad commit, THEN THE Git_Adapter SHALL return an InvalidInput error stating the ancestry requirement.
4. WHEN ancestry is confirmed, THE Git_Adapter SHALL produce a Revision_Sequence containing the good commit, all intermediate commits in topological order, and the bad commit.
5. THE Revision_Sequence SHALL contain at least two commits (the good and bad boundaries).
6. WHEN the `--first-parent` flag is set, THE Git_Adapter SHALL linearize using `--first-parent --ancestry-path`; otherwise THE Git_Adapter SHALL linearize using `--ancestry-path` only.

### Requirement 2: Predicate Execution

**User Story:** As an Operator, I want faultline to run my predicate command against candidate revisions in isolated worktrees, so that each probe is independent and does not mutate my working copy.

#### Acceptance Criteria

1. WHEN the Localization_Engine selects a commit for probing, THE Git_Adapter SHALL create a disposable linked worktree under `.faultline/scratch/` at that commit using `git worktree add --detach --force`.
2. WHEN a Scratch_Worktree is created, THE Probe_Executor SHALL run the Predicate command with the worktree path as the working directory.
3. WHEN the Predicate exits with code 0, THE Probe_Executor SHALL classify the Observation as Pass.
4. WHEN the Predicate exits with code 125, THE Probe_Executor SHALL classify the Observation as Skip.
5. WHEN the Predicate exits with any non-zero code other than 125, THE Probe_Executor SHALL classify the Observation as Fail.
6. WHEN the Predicate exceeds the `--timeout-seconds` duration, THE Probe_Executor SHALL kill the process and classify the Observation as Indeterminate.
7. THE Probe_Executor SHALL capture stdout, stderr, exit code, timeout status, and wall-clock duration in every Observation.
8. WHEN probing completes (regardless of outcome), THE Git_Adapter SHALL remove the Scratch_Worktree using `git worktree remove --force` and delete any residual directory.

### Requirement 3: Localization Engine

**User Story:** As an Operator, I want faultline to narrow the regression window using binary search with honest handling of ambiguous evidence, so that I get the most precise answer the evidence supports.

#### Acceptance Criteria

1. THE Localization_Engine SHALL use binary narrowing to select the next commit to probe from the unobserved candidates between the current pass and fail boundaries.
2. WHEN all commits between an adjacent pass and fail pair have been observed, THE Localization_Engine SHALL return a FirstBad outcome with the fail commit as first_bad and the pass commit as last_good.
3. WHEN one or more commits between the boundaries are classified as Skip, THE Localization_Engine SHALL return a SuspectWindow outcome with AmbiguityReason SkippedRevision.
4. WHEN one or more commits between the boundaries are classified as Indeterminate, THE Localization_Engine SHALL return a SuspectWindow outcome with AmbiguityReason IndeterminateRevision.
5. WHEN the evidence contains a Fail commit that precedes a Pass commit in the Revision_Sequence, THE Localization_Engine SHALL include AmbiguityReason NonMonotonicEvidence and assign Confidence low.
6. IF no Pass boundary is established, THEN THE Localization_Engine SHALL return an Inconclusive outcome with AmbiguityReason MissingPassBoundary.
7. IF no Fail boundary is established, THEN THE Localization_Engine SHALL return an Inconclusive outcome with AmbiguityReason MissingFailBoundary.
8. THE Localization_Engine SHALL stop probing when the maximum probe count (from SearchPolicy) is reached or when no further unobserved candidates exist.
9. WHEN a FirstBad outcome is produced, THE Localization_Engine SHALL ensure that last_good and first_bad are supported by direct Pass and Fail observations respectively.

### Requirement 4: Run Persistence and Resumability

**User Story:** As an Operator, I want faultline to persist observations and resume interrupted runs, so that I do not re-probe commits that have already been tested with the same predicate.

#### Acceptance Criteria

1. WHEN a localization run begins, THE Run_Store SHALL create a Run_Directory keyed by the Probe_Fingerprint of the AnalysisRequest.
2. WHEN an Observation is recorded, THE Run_Store SHALL persist the Observation to the Run_Directory as part of an `observations.json` file.
3. WHEN a run starts and a Run_Directory with matching fingerprint already exists, THE Run_Store SHALL load previously persisted Observations and mark the RunHandle as resumed.
4. THE App_Orchestrator SHALL feed loaded Observations into the Localization_Engine before selecting new probes, so that cached results are reused.
5. THE Run_Store SHALL persist the AnalysisRequest as `request.json` in the Run_Directory.
6. WHEN the run completes, THE Run_Store SHALL persist the final Analysis_Report as `report.json` in the Run_Directory.
7. THE Probe_Executor SHALL capture stdout and stderr log output in each Observation for post-run inspection.

### Requirement 5: Changed-Surface Summary

**User Story:** As an Operator, I want faultline to summarize the files changed between the localization boundaries, so that I know which subsystems to investigate first.

#### Acceptance Criteria

1. WHEN a Localization_Outcome with a boundary pair is produced, THE Git_Adapter SHALL compute the changed paths between the lower and upper boundary commits using `git diff --name-status`.
2. THE Surface_Analyzer SHALL group changed paths into Subsystem_Buckets by top-level directory.
3. THE Surface_Analyzer SHALL assign a surface kind to each path using coarse rules: source, tests, benchmarks, scripts, workflows, docs, build-script, lockfile, migrations, or other.
4. THE Surface_Analyzer SHALL identify execution surfaces (workflow files, build scripts, shell scripts) separately.
5. THE Analysis_Report SHALL include the full list of PathChange entries and the SurfaceSummary with all Subsystem_Buckets.

### Requirement 6: JSON Artifact

**User Story:** As an Operator, I want faultline to produce a canonical JSON artifact for every completed run, so that I can integrate the results into other tools and workflows.

#### Acceptance Criteria

1. THE Report_Renderer SHALL write an `analysis.json` file to the output directory for every completed run.
2. THE `analysis.json` SHALL contain the run_id, created_at timestamp, AnalysisRequest, Revision_Sequence, all Observations, the Localization_Outcome, changed paths, and SurfaceSummary.
3. THE `analysis.json` SHALL be deterministic: identical inputs and observations SHALL produce byte-identical JSON output.
4. THE `analysis.json` SHALL be valid JSON parseable by any standard JSON parser.
5. FOR ALL valid Analysis_Report values, serializing to JSON then deserializing back SHALL produce an equivalent Analysis_Report (round-trip property).

### Requirement 7: HTML Report

**User Story:** As an Operator, I want faultline to produce a human-readable HTML report derived from the JSON artifact, so that I can share results with teammates who prefer a visual format.

#### Acceptance Criteria

1. THE Report_Renderer SHALL write an `index.html` file to the output directory for every completed run.
2. THE `index.html` SHALL display the run ID, localization outcome summary, observation timeline, changed-surface buckets, and changed paths.
3. THE `index.html` SHALL be a self-contained static HTML file with no external resource dependencies.
4. THE `index.html` SHALL be derived from the same Analysis_Report data as the `analysis.json`, so that both artifacts tell the same story.
5. THE `index.html` SHALL use proper HTML escaping for all dynamic content to prevent rendering issues with special characters in commit SHAs, paths, or output.

### Requirement 8: CLI Interface

**User Story:** As an Operator, I want a stable command-line interface with clear flags, help text, and exit codes, so that I can invoke faultline reliably from scripts and CI pipelines.

#### Acceptance Criteria

1. THE CLI SHALL accept `--good <rev>`, `--bad <rev>`, `--repo <path>`, `--timeout-seconds <n>`, and `--output-dir <path>` flags.
2. THE CLI SHALL accept either `--cmd <shell_script>` for shell-mode predicates or `--program <binary>` with `--arg <value>` for exec-mode predicates, but not both simultaneously.
3. IF neither `--cmd` nor `--program` is provided, THEN THE CLI SHALL exit with an error message stating that one is required.
4. IF both `--cmd` and `--program` are provided, THEN THE CLI SHALL exit with an error message stating that only one is allowed.
5. THE CLI SHALL accept `--kind <probe_kind>` to classify the predicate type (build, test, lint, perf-threshold, custom) with a default of custom.
6. THE CLI SHALL accept `--first-parent` to select first-parent history linearization, defaulting to ancestry-path mode.
7. THE CLI SHALL accept `--max-probes <n>` to cap the number of probe executions, defaulting to 64.
8. WHEN the run completes successfully, THE CLI SHALL print the run ID, observation count, output directory path, and outcome summary to stdout.
9. WHEN the run completes successfully, THE CLI SHALL exit with code 0.
10. WHEN the run fails due to an error, THE CLI SHALL print a diagnostic message to stderr and exit with code 2.
11. THE CLI SHALL produce stable `--help` output describing all flags and their defaults.

### Requirement 9: Scratch Worktree Cleanup

**User Story:** As an Operator, I want faultline to clean up all disposable worktrees after each probe, so that my repository does not accumulate stale checkout directories.

#### Acceptance Criteria

1. WHEN a probe completes (pass, fail, skip, timeout, or error), THE Git_Adapter SHALL attempt to remove the Scratch_Worktree via `git worktree remove --force`.
2. IF `git worktree remove` fails, THEN THE Git_Adapter SHALL fall back to deleting the worktree directory directly.
3. THE Git_Adapter SHALL create all Scratch_Worktrees under the `.faultline/scratch/` directory within the repository root.
4. THE Git_Adapter SHALL generate unique worktree directory names using the commit SHA prefix, a timestamp, and an atomic counter to prevent collisions.

### Requirement 10: Boundary Validation

**User Story:** As an Operator, I want faultline to verify that my declared good revision actually passes and my declared bad revision actually fails, so that I get an early error instead of a misleading result.

#### Acceptance Criteria

1. WHEN a localization run begins, THE App_Orchestrator SHALL probe the good boundary commit and verify that the Observation_Class is Pass.
2. WHEN a localization run begins, THE App_Orchestrator SHALL probe the bad boundary commit and verify that the Observation_Class is Fail.
3. IF the good boundary does not evaluate as Pass, THEN THE App_Orchestrator SHALL return an InvalidBoundary error stating the expected and actual class.
4. IF the bad boundary does not evaluate as Fail, THEN THE App_Orchestrator SHALL return an InvalidBoundary error stating the expected and actual class.
5. WHEN cached Observations exist for the boundary commits, THE App_Orchestrator SHALL use the cached results instead of re-probing.

### Requirement 11: Correctness Properties

**User Story:** As a developer, I want the localization engine to satisfy formal correctness properties, so that I can trust the results are logically sound.

#### Acceptance Criteria

1. WHEN a FirstBad outcome is produced, THE Localization_Engine SHALL guarantee that the last_good commit has a direct Pass observation and the first_bad commit has a direct Fail observation.
2. THE Localization_Engine SHALL guarantee that the narrowing process only reduces or maintains the candidate window size; the window SHALL NOT expand between successive probe steps.
3. WHEN a SuspectWindow outcome is produced, THE Localization_Engine SHALL guarantee that the confidence score does not exceed the confidence that would be assigned to a FirstBad outcome with equivalent boundary evidence.
4. THE Localization_Engine SHALL produce the same Localization_Outcome regardless of the order in which Observations are recorded, given the same set of Observations and Revision_Sequence.
5. FOR ALL valid Analysis_Report values, the `analysis.json` and `index.html` artifacts SHALL represent the same Localization_Outcome, observation count, and boundary commits.

### Requirement 12: Fixture-Driven Testing

**User Story:** As a developer, I want a fixture framework for constructing synthetic Git histories and predicate outcomes, so that I can write deterministic tests without depending on real repositories.

#### Acceptance Criteria

1. THE faultline-fixtures crate SHALL provide a RevisionSequenceBuilder that constructs Revision_Sequence values from a list of commit identifiers.
2. THE test suite SHALL include a fixture for the exact-first-bad scenario: a linear history where a single pass-to-fail transition exists.
3. THE test suite SHALL include a fixture for the skipped-midpoint scenario: a linear history where the midpoint commit is classified as Skip.
4. THE test suite SHALL include a fixture for the timeout-island scenario: a linear history where one or more commits are classified as Indeterminate.
5. THE test suite SHALL include a fixture for the non-monotonic-predicate scenario: a history where a Fail observation precedes a Pass observation.
6. THE test suite SHALL include a fixture for the all-revisions-untestable scenario: a history where every intermediate commit is classified as Skip or Indeterminate.
7. THE test suite SHALL include a fixture for the cached-resume scenario: a run that loads previously persisted Observations and completes without re-probing cached commits.

### Requirement 13: Package Metadata and Release Readiness

**User Story:** As a maintainer, I want the workspace to have complete package metadata, documentation, and release artifacts, so that the v0.1 release is professional and auditable.

#### Acceptance Criteria

1. THE workspace Cargo.toml SHALL specify version, edition, license, and authors for all crates.
2. THE README SHALL contain a working quickstart example that an Operator can copy-paste and run.
3. THE repository SHALL contain an ARCHITECTURE.md describing the hexagonal layout and crate boundaries.
4. THE repository SHALL contain ADR documents for key design decisions (hexagonal architecture, Git CLI adapter, honest localization outcomes).
5. THE CLI `--help` output SHALL be stable and describe all accepted flags with their defaults and types.