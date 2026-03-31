# Requirements Document

## Introduction

This document captures the requirements for the faultline v0.1 product sharpening pass — the wave of improvements that deepens the product wedge, closes verification gaps, and matures the repo for delegation. Three prior specs are complete: v01-release-train (initial vertical slice), v01-hardening (frozen contract, proven core, hardened adapters, safe persistence, real fixtures, polished operator surface), and repo-operating-system (xtask, schema/golden contracts, export adapters, pattern catalog, scenario atlas, verification matrix, teaching docs, CI workflows, scaffold commands, mutation/fuzz config).

The repo now has 15 crates in a hexagonal architecture, a working CLI with resumability flags, JSON+HTML artifacts, SARIF and JUnit export adapters, a scenario atlas, a verification matrix, and a real command surface via xtask/Justfile. This sharpening pass addresses twelve areas organized into three tiers:

**Product differentiation features** — ranked suspect surface with owner hints, Markdown dossier export, flake-aware probing, reproduction capsule, and run-to-run comparison. These are the features that turn faultline from a bisection tool into a regression archaeologist.

**Verification depth** — deeper mutation coverage across adapter and export crates, more fuzz targets for Git/store/render/CLI surfaces, heavier BDD/scenario coverage for outer flows, and scenario atlas enrichment with metadata fields. These close the gap between the house style's ambition and the current test surface.

**Repo maturation** — finish xtask authority (real smoke, real link checking, explicit generate/check/export commands), clean repo-law residue (workspace metadata, placeholder language, doc/reality mismatches), and deepen the teaching layer (more maintainer playbooks, worked examples, stronger Diátaxis depth).

Key observations about the current state that motivate these requirements:
- `SurfaceSummary` provides coarse path bucketing but no ranking, no owner hints, no rename/delete weighting, and no execution-surface highlighting in the suspect list
- No Markdown export exists — SARIF and JUnit serve machines, HTML serves browsers, but nothing serves PR threads, incident handoffs, or issue comments
- No flake detection exists — ambiguous/boundary commits are probed once and classified; flaky predicates produce misleading outcomes
- No reproduction command is emitted — operators must reconstruct the exact predicate, env, working directory, and target commit manually
- No run-to-run comparison exists — operators cannot answer "did we narrow the problem?" across successive runs
- Mutation testing covers only `faultline-localization` and `faultline-app`; adapter translation layers and export surfaces are unmutated
- Only one fuzz target exists (`fuzz_analysis_report` in `faultline-types`); Git adapter parsing, store corruption paths, renderer/export adapters, and CLI/env parsing are unfuzzed
- The scenario atlas lists tests but lacks metadata: no scenario tier, no requirement IDs, no artifact contract touched, no mutation surface, no criticality, no ownership hints
- `cargo xtask smoke` is a placeholder; `docs-check` does not perform real link checking; no explicit commands exist for `generate-schema`, `check-scenarios`, `export-markdown`, `export-sarif`, `export-junit`
- Workspace `Cargo.toml` still has `authors = ["OpenAI"]`; some docs may reference placeholder language or stale state
- Teaching layer lacks maintainer playbooks for reviewing failing property-test fixes, deciding test technique, bumping schema_version, and handling breaking changes

## Glossary

- **Suspect_Surface**: A ranked list of changed paths between the regression boundary commits, ordered by investigation priority, with execution-surface flags, rename/delete weighting, and optional owner hints.
- **Owner_Hint**: A suggested code owner for a changed path, derived from a CODEOWNERS file or git-blame frequency heuristics.
- **Markdown_Dossier**: A human-operational Markdown export of a faultline run, suitable for pasting into PRs, issues, incident threads, or handoff packets.
- **Flake_Signal**: An explicit indicator that a commit's predicate result was unstable across multiple probe executions, recorded in the observation and surfaced in the report.
- **Stability_Threshold**: A fraction (0.0–1.0) representing the minimum proportion of consistent probe results required to classify a commit as stable (non-flaky).
- **Reproduction_Capsule**: A self-contained script or structured record that captures the exact predicate, environment, working directory, target commit, and timeout needed to reproduce a single faultline probe.
- **Run_Comparison**: A structured diff between two faultline runs, comparing outcomes, confidence, window width, probe reuse, suspect path changes, and ambiguity reason changes.
- **Localization_Engine**: The pure-domain regression-window search engine (`faultline-localization`, specifically `LocalizationSession`).
- **App_Orchestrator**: The application-layer use-case coordinator (`faultline-app`, specifically `FaultlineApp`).
- **CLI**: The `faultline-cli` binary, the sole operator-facing entry point.
- **Report_Renderer**: The adapter that writes the canonical JSON artifact and the derived HTML report (`faultline-render`).
- **Run_Store**: The filesystem-backed persistence layer for observations and run metadata (`faultline-store`).
- **Git_Adapter**: The adapter that shells out to the system `git` binary for history, worktree, and diff operations (`faultline-git`).
- **Probe_Executor**: The adapter that runs the operator's predicate command (`faultline-probe-exec`).
- **Analysis_Report**: The canonical output artifact containing request, sequence, observations, outcome, changed paths, surface summary, and (new) ranked suspect surface.
- **Scenario_Atlas**: The first-class discoverable index of BDD and property-test scenarios at `docs/scenarios/scenario_index.md`.
- **Verification_Matrix**: The per-crate mapping of verification techniques at `docs/verification-matrix.md`.
- **Xtask**: The Rust-native `cargo xtask` binary at `crates/xtask/`.
- **Mutation_Surface**: The set of crates and code regions targeted by `cargo-mutants` for mutation testing.
- **Fuzz_Target**: A `cargo-fuzz` harness that exercises a parsing, deserialization, or boundary path with random inputs.
- **Teaching_Layer**: Agent-facing and contributor-facing documentation (AGENTS.md, TESTING.md, RELEASE.md, MAINTAINERS.md, handbook, Diátaxis site).

## Requirements

### Requirement 1: Ranked Suspect Surface with Owner Hints

**User Story:** As an operator investigating a regression, I want the report to show changed files ranked by investigation priority with execution-surface flags, rename/delete weighting, and owner hints, so that I know which files to read first instead of scanning a flat list.

#### Acceptance Criteria

1. THE Analysis_Report SHALL include a `suspect_surface` field containing a ranked list of changed paths between the boundary commits, ordered by descending investigation priority score.
2. WHEN ranking suspect paths, THE Suspect_Surface SHALL assign higher priority scores to: execution surfaces (workflow files, build scripts, shell scripts), renamed files, and deleted files, relative to ordinary modified source files.
3. WHEN a `CODEOWNERS` file exists in the repository root, THE Git_Adapter SHALL parse the CODEOWNERS file and THE Suspect_Surface SHALL include an `owner_hint` field for each path that matches a CODEOWNERS pattern.
4. WHEN no `CODEOWNERS` file exists, THE Git_Adapter SHALL derive owner hints from git-blame frequency heuristics (most-frequent committer to each changed file in the last 90 days), and THE Suspect_Surface SHALL include the derived `owner_hint` field.
5. IF both CODEOWNERS and git-blame data are unavailable for a path, THEN THE Suspect_Surface SHALL set the `owner_hint` field to `null` for that path.
6. THE Suspect_Surface SHALL include a `surface_kind` field for each path (source, tests, scripts, workflows, build-script, docs, lockfile, other) reusing the classification from `faultline-surface`.
7. THE Suspect_Surface SHALL include a `change_status` field for each path (added, modified, deleted, renamed) from the existing `PathChange` data.
8. THE HTML report SHALL render the Suspect_Surface as a prioritized list with visual distinction for execution surfaces and owner hints displayed inline.
9. THE JSON artifact SHALL include the `suspect_surface` array with fields: `path`, `priority_score`, `surface_kind`, `change_status`, `is_execution_surface`, and `owner_hint`.
10. THE Suspect_Surface ranking algorithm SHALL be deterministic: identical inputs produce identical rankings.

### Requirement 2: Markdown Dossier Export

**User Story:** As an operator, I want a Markdown export of a faultline run that I can paste into a PR, issue, incident thread, or handoff packet, so that I can share regression findings without requiring recipients to open HTML files or parse JSON.

#### Acc