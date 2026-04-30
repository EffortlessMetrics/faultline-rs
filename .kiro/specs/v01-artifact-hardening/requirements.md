# Requirements Document

## Introduction

This specification covers three Priority 0 hardening workstreams for faultline v0.1: canonicalizing the run/artifact model so every command works from either the internal run-store or the rendered artifact directory, adding default-safe redaction of environment variable values and secret-like patterns in all shareable artifacts, and aligning documentation with implementation reality. Together these workstreams ensure faultline artifacts are safe to share, commands are predictable regardless of input layout, and documentation is truthful.

## Glossary

- **Run_Store**: The filesystem-backed persistence layer under `.faultline/runs/{fingerprint}/`, managed by `faultline-store` via the `RunStorePort` trait. Contains `request.json`, `observations.json`, `report.json`, `metadata.json`, and optional `logs/` directory.
- **Shareable_Artifact**: Any file produced by `faultline-render` or export adapters intended for distribution outside the local machine. Core shareables: `analysis.json`, `index.html`, `dossier.md`. Optional shareables: SARIF output, JUnit XML output.
- **Report_JSON**: The `report.json` file written by `faultline-store` into the Run_Store directory. Contains the full `AnalysisReport` serialization for internal persistence and local reproducibility. Always unredacted.
- **Analysis_JSON**: The `analysis.json` file written by `faultline-render` into the user-specified output directory. Contains the `AnalysisReport` serialization intended for sharing. Redacted by default.
- **Redaction_Policy**: A configuration describing which environment variable values and secret-like patterns are masked in Shareable_Artifacts. The default policy masks all env values and scrubs high-confidence secret patterns; explicit opt-in flags override each surface independently.
- **CLI**: The `faultline-cli` crate, the operator-facing entry point providing the `faultline` binary with subcommands.
- **Xtask**: The `xtask` crate providing `cargo xtask` subcommands for CI, export, scaffolding, and repo operations.
- **Artifact_Bundle**: A self-contained directory or archive containing all Shareable_Artifacts from a single analysis run, suitable for attaching to issues or sharing with teammates.
- **Env_Pair**: A key-value tuple `(String, String)` representing an environment variable, stored in `ProbeSpec.env` and `ReproductionCapsule.env`.
- **Report_Locator**: A shared helper module implementing deterministic report file resolution. Used by all CLI subcommands and xtask export commands to locate and load an `AnalysisReport` from either a directory path or a direct file path.
- **Provenance**: Metadata fields within the `AnalysisReport` recording tool version, schema version, redaction state, and artifact source, enabling consumers to understand how the artifact was produced.
- **Adapter_Count**: The number of infrastructure adapter crates in the workspace. Currently six: `faultline-git`, `faultline-probe-exec`, `faultline-store`, `faultline-render`, `faultline-sarif`, `faultline-junit`.
- **Env_Redaction**: (Surface A) Structured masking of values in `ProbeSpec.env`, `ReproductionCapsule.env`, and shell-script `export` lines within Shareable_Artifacts.
- **Output_Scrubbing**: (Surface B) Pattern-based masking of secret-like material found in observation `stdout` and `stderr` fields within Shareable_Artifacts.
- **Secret_Pattern**: A fixed, conservative set of high-confidence regex patterns matching common secret formats (e.g., `ghp_`, `AKIA`, `sk-live_`, `Bearer `, `password=`).

## Requirements

### Requirement 1: Two-Tier Artifact Model Documentation

**User Story:** As a faultline user, I want clear documentation distinguishing `report.json` (internal persistence) from `analysis.json` (shareable artifact), so that I understand which file to use for which purpose.

#### Acceptance Criteria

1. THE CLI SHALL include help text for every subcommand that accepts a report path, explaining whether the subcommand reads from the Run_Store directory or the rendered artifact directory.
2. WHEN the `faultline` binary prints artifact paths after a localization run, THE CLI SHALL label each path with its tier (internal or shareable).
3. THE Architecture_Documentation (`docs/architecture.md`) SHALL describe the two-tier artifact model, defining Report_JSON as internal persistence and Analysis_JSON as the shareable artifact.
4. THE Architecture_Documentation SHALL state the architectural invariant: Run_Store contents remain local and full-fidelity (unredacted), while Shareable_Artifacts are redacted projections produced at render/export time.

### Requirement 2: Shared Report Locator

**User Story:** As a faultline user, I want every CLI and xtask export command to find the right report file automatically, so that I do not need to remember which file lives where.

#### Acceptance Criteria

1. THE Codebase SHALL implement a shared Report_Locator helper module (not duplicated logic per subcommand) with the following deterministic precedence for directory paths: (a) `report.json` if both `report.json` and `analysis.json` exist, (b) `report.json` if only `report.json` exists, (c) `analysis.json` if only `analysis.json` exists, (d) hard error with exit code 2 listing expected filenames if neither exists.
2. WHEN the Report_Locator resolves a directory containing both `report.json` and `analysis.json`, THE Report_Locator SHALL emit a diagnostic note to stderr indicating both files are present and which was chosen.
3. WHEN a direct JSON file path is passed (not a directory), THE Report_Locator SHALL load the report from that file path directly, accepting both `report.json` and `analysis.json` files.
4. WHEN a CLI subcommand (`reproduce`, `export-markdown`) receives a `--run-dir` path, THE CLI SHALL delegate to the shared Report_Locator for file resolution.
5. WHEN an xtask export command (`export-markdown`, `export-sarif`, `export-junit`) receives a `--run-dir` path, THE Xtask SHALL delegate to the shared Report_Locator for file resolution.
6. WHEN a command streams output to stdout (`export-markdown`, `export-sarif`, `export-junit`), THE Report_Locator SHALL emit all diagnostic messages (including "both files present" notes) to stderr, not stdout, to avoid corrupting the output stream.

### Requirement 3: Diff-Runs File-Based Loading

**User Story:** As a faultline user, I want `diff-runs` to accept both `report.json` and `analysis.json` file paths, so that I can compare runs regardless of which artifact I have.

#### Acceptance Criteria

1. THE `diff-runs` subcommand SHALL accept `--left <file>` and `--right <file>` arguments, where each is a direct path to a JSON file (either `report.json` or `analysis.json`).
2. WHEN a `--left` or `--right` path points to a valid `report.json` file, THE CLI SHALL load and deserialize the `AnalysisReport` from that file.
3. WHEN a `--left` or `--right` path points to a valid `analysis.json` file, THE CLI SHALL load and deserialize the `AnalysisReport` from that file.
4. IF a `--left` or `--right` path does not exist, THEN THE CLI SHALL return an error with exit code 2 and a message indicating the file was not found.
5. IF a `--left` or `--right` path contains invalid JSON or does not deserialize into an `AnalysisReport`, THEN THE CLI SHALL return an error with exit code 2 and a descriptive parse error message.

### Requirement 4: Inspect-Run Command

**User Story:** As a faultline user, I want an `inspect-run` subcommand that explains the layout of a completed run directory, so that I can understand what each file contains without reading source code.

#### Acceptance Criteria

1. WHEN the user invokes `faultline inspect-run --run-dir <path>`, THE CLI SHALL list every file present in the run directory with a one-line description of its purpose.
2. WHEN the run directory contains `report.json`, THE CLI SHALL display the run ID, schema version, outcome type, observation count, and creation timestamp extracted from the report.
3. WHEN the run directory contains `observations.json`, THE CLI SHALL display the number of cached observations.
4. WHEN the run directory contains a `logs/` subdirectory, THE CLI SHALL display the number of log files present.
5. IF the run directory does not exist, THEN THE CLI SHALL return an error with exit code 2 and a message indicating the directory was not found.
6. WHEN the user passes `--json` to `inspect-run`, THE CLI SHALL emit the run layout as a JSON object to stdout instead of human-readable text.

### Requirement 5: Bundle Command with Render-on-Demand

**User Story:** As a faultline user, I want a `faultline bundle` command that packages all shareable artifacts from a run into a single directory or archive, so that I can easily attach them to an issue or share with teammates.

#### Acceptance Criteria

1. WHEN the user invokes `faultline bundle --source <path> --output <dest>`, THE CLI SHALL load the report using the shared Report_Locator, accepting a run directory, `report.json` file, or `analysis.json` file as the source.
2. WHEN the source directory lacks one or more core Shareable_Artifacts (`analysis.json`, `index.html`, `dossier.md`), THE CLI SHALL generate the missing artifacts into a temporary staging area before bundling.
3. THE CLI SHALL include core Shareable_Artifacts (`analysis.json`, `index.html`, `dossier.md`) in the bundle by default.
4. WHEN the user passes `--include-sarif`, THE CLI SHALL generate and include SARIF output in the bundle. WHEN the user passes `--include-junit`, THE CLI SHALL generate and include JUnit XML output in the bundle. WHEN neither flag is passed, THE CLI SHALL include SARIF or JUnit files only if they already exist in the source directory.
5. WHEN the user passes `--format tar.gz` to the bundle command, THE CLI SHALL produce a gzip-compressed tar archive at the destination path instead of a directory.
6. THE CLI SHALL apply the active Redaction_Policy to all artifacts in the bundle before writing them.
7. IF the source directory contains no loadable report, THEN THE CLI SHALL return an error with exit code 2.
8. WHEN the bundle completes successfully, THE CLI SHALL print the output path and a count of included artifacts.

### Requirement 6: Default Environment Variable Redaction

**User Story:** As a faultline user, I want environment variable values redacted by default in all shareable artifacts, so that I do not accidentally leak secrets when sharing analysis results.

#### Acceptance Criteria

1. THE Renderer SHALL replace every Env_Pair value in Analysis_JSON with the string `"[REDACTED]"` by default, preserving the key name.
2. THE Renderer SHALL replace every Env_Pair value in the Markdown dossier reproduction section with `[REDACTED]` by default.
3. THE Renderer SHALL replace every Env_Pair value in the HTML report with `[REDACTED]` by default.
4. THE SARIF_Adapter SHALL replace every Env_Pair value in SARIF output with `[REDACTED]` by default.
5. THE JUnit_Adapter SHALL replace every Env_Pair value in JUnit XML output with `[REDACTED]` by default.
6. WHEN the `reproduce --shell` subcommand generates a shell script, THE CLI SHALL replace every `export KEY='VALUE'` line with `export KEY='[REDACTED]'` by default.
7. THE Run_Store SHALL retain full unredacted Env_Pair values in Report_JSON for local reproducibility. The `AnalysisReport` in memory and in the Run_Store is always the full-truth, unredacted representation. Redaction is a render/export-time projection, never a mutation of the stored data.

### Requirement 7: Env Redaction Opt-In Override

**User Story:** As a faultline user performing local debugging, I want an explicit flag to include raw environment variable values in shareable artifacts, so that I can share full reproduction details when I know the values are safe.

#### Acceptance Criteria

1. WHEN the user passes `--unsafe-include-env` to the `faultline` localization command, THE Renderer SHALL write unredacted Env_Pair values to all Shareable_Artifacts.
2. WHEN the user passes `--unsafe-include-env` to `reproduce --shell`, THE CLI SHALL emit unredacted `export KEY='VALUE'` lines.
3. WHEN `--unsafe-include-env` is active, THE Renderer SHALL set the `redaction_applied` Provenance field to `false` in Analysis_JSON.
4. WHEN `--unsafe-include-env` is not passed, THE Renderer SHALL set the `redaction_applied` Provenance field to `true` in Analysis_JSON.

### Requirement 8: Provenance Redaction and Source Fields

**User Story:** As a consumer of faultline artifacts, I want provenance metadata indicating whether redaction was applied and which source file was loaded, so that I can determine if environment values are trustworthy or masked and understand the artifact lineage.

#### Acceptance Criteria

1. THE AnalysisReport SHALL include a `redaction_policy` field of type string, recording the policy name applied during rendering (e.g., `"default"` or `"none"`).
2. THE AnalysisReport SHALL include a `redaction_applied` field of type boolean, set to `true` when env values were redacted and `false` when raw values were preserved.
3. THE AnalysisReport SHALL include an `artifact_source` field of type optional string, recording which file the Report_Locator actually loaded (e.g., `"report_json"` or `"analysis_json"`), set at render/export time.
4. WHEN the schema adds `redaction_policy`, `redaction_applied`, and `artifact_source` fields, THE Schema_Version SHALL be bumped and the JSON Schema regenerated.
5. THE `redaction_policy`, `redaction_applied`, and `artifact_source` fields SHALL have `#[serde(default)]` annotations so that reports produced before this change deserialize without error.

### Requirement 9: Stdout/Stderr Secret Pattern Scrubbing

**User Story:** As a faultline user, I want obvious secret patterns in probe stdout/stderr excerpts redacted in shareable artifacts, so that accidentally printed tokens do not leak.

#### Acceptance Criteria

1. WHEN rendering Shareable_Artifacts, THE Renderer SHALL scan `stdout`, `stderr`, and `probe_command` fields of each observation, as well as `ProbeSpec` shell script content, program name, and argument values, for patterns matching a fixed, conservative set of Secret_Patterns: strings prefixed with `ghp_`, `gho_`, `ghu_`, `ghs_`, `ghr_` (GitHub tokens), `AKIA` (AWS access keys), `sk-live_`, `sk-test_` (Stripe keys), `Bearer ` followed by a token, and `password=` followed by a value.
2. WHEN a matching pattern is found, THE Renderer SHALL replace the secret portion with `[REDACTED]` in the Shareable_Artifact, preserving surrounding context so that the output remains useful for debugging.
3. THE Renderer SHALL not modify stdout/stderr content in Report_JSON stored in the Run_Store. Output_Scrubbing is a render-time projection only.
4. WHEN `--unsafe-include-env` is active, THE Renderer SHALL still apply Output_Scrubbing to stdout/stderr secret patterns unless the user also passes `--unsafe-include-output`.
5. THE `--unsafe-include-output` flag SHALL be independent of `--unsafe-include-env`, allowing a user to include raw env values while still scrubbing output, or vice versa.
6. THE Test_Suite SHALL include a fixture corpus of output samples covering: GitHub token-looking strings, AWS key-looking strings, bearer tokens, `password=` cases, and false-positive near misses (e.g., strings that look similar but should not be redacted).

### Requirement 10: Architecture Documentation Accuracy

**User Story:** As a contributor, I want the architecture documentation to accurately reflect the current codebase, so that I can trust the docs when onboarding.

#### Acceptance Criteria

1. THE `docs/architecture.md` SHALL state the correct Adapter_Count of six infrastructure adapters, listing `faultline-git`, `faultline-probe-exec`, `faultline-store`, `faultline-render`, `faultline-sarif`, and `faultline-junit`.
2. THE `docs/architecture.md` SHALL describe `faultline-fixtures` as a testing utility, not an infrastructure adapter.
3. THE `docs/architecture.md` SHALL document the two-tier artifact model (Report_JSON for internal persistence, Analysis_JSON for shareable output) and the architectural invariant that the Run_Store is always unredacted while Shareable_Artifacts are redacted projections.

### Requirement 11: CLI Flag Documentation

**User Story:** As a faultline user, I want all CLI flags documented in the README and help text, so that I can discover available options without reading source code.

#### Acceptance Criteria

1. THE `README.md` SHALL document the `--env KEY=VALUE` flag with usage examples.
2. THE `README.md` SHALL document the `--retries` flag and its interaction with flake-aware probing.
3. THE `README.md` SHALL document the `--stability-threshold` flag and its valid range (0.0 to 1.0).
4. THE `README.md` SHALL document the `--markdown` flag for Markdown dossier generation.
5. THE `README.md` SHALL document the `--shell` flag and its accepted values (`sh`, `cmd`, `powershell`).
6. THE `README.md` SHALL document the `--unsafe-include-env` flag introduced by this spec.
7. THE `README.md` SHALL document the `--unsafe-include-output` flag introduced by this spec.

### Requirement 12: Xtask Export Command Documentation

**User Story:** As a faultline user, I want xtask export commands documented, so that I know how to produce SARIF, JUnit, and Markdown exports from completed runs.

#### Acceptance Criteria

1. THE `README.md` SHALL document `cargo xtask export-markdown --run-dir <path>` with usage examples.
2. THE `README.md` SHALL document `cargo xtask export-sarif --run-dir <path>` with usage examples.
3. THE `README.md` SHALL document `cargo xtask export-junit --run-dir <path>` with usage examples.
4. THE `README.md` SHALL explain that after this spec is implemented, all xtask export commands use the shared Report_Locator and accept directories containing either `report.json` or `analysis.json`.

### Requirement 13: Report Loading Path Documentation

**User Story:** As a contributor, I want the report loading path differences between CLI and xtask commands documented, so that I understand the unified loading strategy.

#### Acceptance Criteria

1. THE `docs/architecture.md` SHALL include a table mapping each CLI subcommand and xtask export command to the report file resolution strategy, noting that all commands delegate to the shared Report_Locator.
2. THE `docs/architecture.md` SHALL explain the historical reason for the divergence (CLI loaded `report.json`, xtask loaded `analysis.json`) and how the shared Report_Locator resolves it with deterministic precedence.

### Requirement 14: Reproduction Capsule Documentation

**User Story:** As a faultline user, I want the reproduction capsule structure and shell script generation documented, so that I understand what `reproduce --shell` outputs and how to use it.

#### Acceptance Criteria

1. THE `README.md` SHALL document the `ReproductionCapsule` structure, explaining that each capsule captures the commit, predicate, environment, working directory, and timeout needed to reproduce a probe.
2. THE `README.md` SHALL document the `reproduce` subcommand with examples of `--shell` output and `--commit` targeting.
3. THE `README.md` SHALL note that environment variable values in shell script output are redacted by default and require `--unsafe-include-env` to include raw values.

### Requirement 15: Flake-Aware Probing Documentation

**User Story:** As a faultline user, I want flake-aware probing behavior documented, so that I understand how `--retries` and `--stability-threshold` affect localization outcomes.

#### Acceptance Criteria

1. THE `README.md` SHALL document the `FlakeSignal` structure and its fields (`total_runs`, `pass_count`, `fail_count`, `skip_count`, `indeterminate_count`, `is_stable`).
2. THE `README.md` SHALL explain the majority-vote logic used to classify observations when retries are enabled.
3. THE `README.md` SHALL explain how `--stability-threshold` determines whether an observation is marked stable or unstable.
4. THE `README.md` SHALL document how unstable observations affect confidence scoring in the localization outcome.

### Requirement 16: CI Tier Documentation Alignment

**User Story:** As a contributor, I want CI tier descriptions to match actual workflow reality, so that I can trust the documented CI behavior.

#### Acceptance Criteria

1. THE `TESTING.md` SHALL accurately describe the `ci-fast` tier as running `cargo fmt --check`, `cargo clippy --workspace -- -D warnings`, and `cargo test --workspace`.
2. THE `TESTING.md` SHALL accurately describe the `ci-full` tier as running ci-fast plus golden snapshot verification and JSON Schema drift detection.
3. THE `TESTING.md` SHALL accurately describe the `ci-extended` tier as running mutation testing and supply-chain checks (`cargo-deny`, `cargo-audit`, `cargo-semver-checks`).
4. THE `TESTING.md` SHALL explicitly state that faultline supports fuzz targets (via `cargo xtask fuzz`), but the current `ci-extended` workflow (`.github/workflows/ci-extended.yml`) does not execute fuzz automatically; fuzz is available as a manual tool.
5. THE `docs/verification-matrix.md` SHALL be consistent with the CI tier descriptions in `TESTING.md`.
6. THE `docs/patterns/catalog.md` SHALL be consistent with the CI tier descriptions in `TESTING.md`.
7. THE `.github/workflows/ci.yml`, `.github/workflows/ci-full.yml`, and `.github/workflows/ci-extended.yml` SHALL be verified against the documented tier descriptions, and any discrepancies SHALL be resolved before the pull request is merged.

### Requirement 17: End-to-End Artifact Layout Tests

**User Story:** As a contributor, I want end-to-end tests verifying that every export command works from both the Run_Store directory and the rendered artifact directory, so that regressions in report loading are caught automatically.

#### Acceptance Criteria

1. FOR ALL CLI subcommands that use the Report_Locator with `--run-dir` (`reproduce`, `export-markdown`), THE Test_Suite SHALL include a test that loads from a directory containing only `report.json` and verifies successful execution.
2. FOR ALL CLI subcommands that use the Report_Locator with `--run-dir`, THE Test_Suite SHALL include a test that loads from a directory containing only `analysis.json` and verifies successful execution.
3. FOR the `diff-runs` subcommand, THE Test_Suite SHALL include a test that loads from a `report.json` file path and a test that loads from an `analysis.json` file path, verifying successful execution for each.
4. FOR ALL xtask export commands (`export-markdown`, `export-sarif`, `export-junit`), THE Test_Suite SHALL include a test that loads from a directory containing only `report.json` and verifies successful execution.
5. FOR ALL xtask export commands, THE Test_Suite SHALL include a test that loads from a directory containing only `analysis.json` and verifies successful execution.
6. FOR ALL commands that load reports, THE Test_Suite SHALL include a test that attempts to load from an empty directory (or nonexistent file for `diff-runs`) and verifies the correct error message is returned.

### Requirement 18: Redaction Completeness Tests

**User Story:** As a contributor, I want property-based tests verifying that no shareable artifact contains raw environment variable values when redaction is active, so that secret leakage is caught automatically.

#### Acceptance Criteria

1. FOR ALL valid AnalysisReport instances with non-empty Env_Pairs, THE Test_Suite SHALL include a property test verifying that Analysis_JSON produced by the Renderer contains no raw Env_Pair values when redaction is active. The test SHALL generate sentinel env values with a unique prefix (e.g., UUID-like) to avoid false positives from values that coincidentally appear in other fields.
2. FOR ALL valid AnalysisReport instances with non-empty Env_Pairs, THE Test_Suite SHALL include a property test verifying that Markdown dossier output contains no raw Env_Pair values when redaction is active, using sentinel env values.
3. FOR ALL valid AnalysisReport instances with non-empty Env_Pairs, THE Test_Suite SHALL include a property test verifying that SARIF output contains no raw Env_Pair values when redaction is active, using sentinel env values.
4. FOR ALL valid AnalysisReport instances with non-empty Env_Pairs, THE Test_Suite SHALL include a property test verifying that JUnit XML output contains no raw Env_Pair values when redaction is active, using sentinel env values.
5. FOR ALL valid AnalysisReport instances with non-empty Env_Pairs, THE Test_Suite SHALL include a property test verifying that shell script output from `ReproductionCapsule::to_shell_script` contains no raw Env_Pair values when redaction is active, using sentinel env values.
6. FOR ALL valid AnalysisReport instances with non-empty Env_Pairs and `--unsafe-include-env` active, THE Test_Suite SHALL include a property test verifying that all raw Env_Pair values are present in the output.

### Requirement 19: Redaction Round-Trip and Schema Compatibility

**User Story:** As a contributor, I want round-trip property tests ensuring that redacted artifacts still deserialize correctly and that schema evolution is backward-compatible, so that redaction does not break downstream consumers.

#### Acceptance Criteria

1. FOR ALL valid AnalysisReport instances, THE Test_Suite SHALL include a property test verifying that serializing with redaction and then deserializing produces a valid AnalysisReport (the `env` values will be `[REDACTED]` but the structure remains intact).
2. FOR ALL valid AnalysisReport instances, THE Test_Suite SHALL include a property test verifying that the redacted Analysis_JSON conforms to the JSON Schema at `schemas/analysis-report.schema.json`.
3. THE Test_Suite SHALL include a test verifying that old unredacted reports (without `redaction_policy`, `redaction_applied`, or `artifact_source` fields) deserialize successfully into the current `AnalysisReport` structure via `#[serde(default)]`.
4. THE Test_Suite SHALL include a test verifying that new redacted reports (with provenance fields populated) deserialize into the same `AnalysisReport` structure.
5. THE Test_Suite SHALL include a property test verifying that rendering a redacted report again is idempotent: applying redaction to an already-redacted report produces identical output.
