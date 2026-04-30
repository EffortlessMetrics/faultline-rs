# Implementation Plan: v01-artifact-hardening

## Overview

This plan implements three Priority 0 hardening workstreams for faultline v0.1 across four phases: shared report loading (Phase A), default-safe redaction (Phase B), inspect + bundle commands (Phase C), and documentation alignment (Phase D). Each phase builds on the previous, with clear dependency ordering. All code is Rust, using the existing workspace conventions (`proptest`, `insta`, `clap`, `serde`).

## Tasks

- [ ] 1. Phase A: Shared Loader — Scaffold `faultline-loader` crate and add locator types
  - [ ] 1.1 Add `ArtifactSource` and `LocatedReport` types to `faultline-types`
    - Add `ArtifactSource` enum (`ReportJson`, `AnalysisJson`, `DirectFile`) with `Serialize`, `Deserialize`, `JsonSchema`, `Debug`, `Clone`, `PartialEq`, `Eq` derives
    - Add `LocatedReport` struct with `report: AnalysisReport`, `source: ArtifactSource`, `diagnostics: Vec<String>`
    - _Requirements: 2.1, 8.3_

  - [ ] 1.2 Scaffold `faultline-loader` crate
    - Run `cargo xtask scaffold crate faultline-loader --tier adapter` (or manually create `crates/faultline-loader/` with `Cargo.toml`, `src/lib.rs`, `claude.md`)
    - Add `faultline-loader` to workspace `Cargo.toml` members list
    - Add dependencies: `faultline-types`, `serde_json`
    - _Requirements: 2.1_

  - [ ] 1.3 Implement `locate_and_load_report()` in `faultline-loader`
    - Implement the function with deterministic precedence: if path is a directory, try `report.json` first, then `analysis.json`, error if neither exists; if path is a file, load directly
    - Emit diagnostic to `diagnostics` vec when both `report.json` and `analysis.json` are present
    - Return `FaultlineError::Store` with exit-code-2-appropriate messages on failure
    - _Requirements: 2.1, 2.2, 2.3, 2.6_

  - [ ] 1.4 Replace CLI `load_report_from_dir()` with `faultline-loader`
    - Add `faultline-loader` dependency to `faultline-cli/Cargo.toml`
    - Replace `load_report_from_dir()` in `crates/faultline-cli/src/main.rs` with calls to `faultline_loader::locate_and_load_report()`
    - Print diagnostics to stderr for stdout-streaming commands (`export-markdown`)
    - Update `run_reproduce()` to use the shared loader
    - Update `run_export_markdown()` to use the shared loader
    - _Requirements: 2.4, 2.6_

  - [ ] 1.5 Replace xtask `load_report()` with `faultline-loader`
    - Add `faultline-loader` dependency to `crates/xtask/Cargo.toml`
    - Replace `load_report()` in `crates/xtask/src/main.rs` with calls to `faultline_loader::locate_and_load_report()`
    - Print diagnostics to stderr for all xtask export commands
    - _Requirements: 2.5, 2.6_

  - [ ] 1.6 Update `diff-runs` to use `faultline-loader` for file-based loading
    - Replace `load_report_from_file()` in CLI with `faultline_loader::locate_and_load_report()` (which handles direct file paths)
    - Ensure `--left` and `--right` accept both `report.json` and `analysis.json` file paths
    - _Requirements: 3.1, 3.2, 3.3, 3.4, 3.5_

  - [ ]* 1.7 Write property test for report locator precedence (Property 1)
    - **Property 1: Report Locator Precedence**
    - Use `tempfile` to create directories with various combinations of `report.json` and `analysis.json`
    - Verify deterministic precedence: `report.json` > `analysis.json`, error when neither exists, diagnostics when both present
    - Minimum 100 cases
    - **Validates: Requirements 2.1, 2.2**

  - [ ]* 1.8 Write end-to-end loading tests for all paths (Req 17)
    - Test CLI `reproduce` loads from `report.json`-only directory
    - Test CLI `reproduce` loads from `analysis.json`-only directory
    - Test CLI `export-markdown` loads from `report.json`-only directory
    - Test CLI `export-markdown` loads from `analysis.json`-only directory
    - Test `diff-runs` loads from `report.json` file path and `analysis.json` file path
    - Test xtask `export-markdown`, `export-sarif`, `export-junit` load from `report.json`-only and `analysis.json`-only directories
    - Test all commands error on empty directory with correct error message
    - _Requirements: 17.1, 17.2, 17.3, 17.4, 17.5, 17.6_

- [ ] 2. Checkpoint — Phase A complete
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 3. Phase B: Redaction Spine — Core types and pure redaction function
  - [ ] 3.1 Add `RedactionPolicy` to `faultline-types`
    - Add `RedactionPolicy` struct with `redact_env: bool` and `scrub_secrets: bool`
    - Implement `default_safe()`, `none()`, `env_exposed()`, `secrets_exposed()`, `name()` methods
    - Add `Serialize`, `Deserialize`, `JsonSchema`, `Debug`, `Clone`, `PartialEq`, `Eq` derives
    - _Requirements: 6.1, 6.7, 7.1, 8.1_

  - [ ] 3.2 Add `SecretScrubber` with `LazyLock` to `faultline-types`
    - Add `regex` dependency to `faultline-types/Cargo.toml`
    - Implement `SECRET_PATTERNS` static using `std::sync::LazyLock` with patterns for: GitHub tokens (`gh[pousr]_`), AWS keys (`AKIA`), Stripe keys (`sk-live_`, `sk-test_`), Bearer tokens, `password=` values
    - Implement `scrub_secrets(input: &str) -> String` that preserves prefix and replaces secret portion with `[REDACTED]`
    - _Requirements: 9.1, 9.2_

  - [ ] 3.3 Add `ArtifactProvenance` struct to `faultline-types`
    - Add `ArtifactProvenance` with fields: `redaction_policy: String`, `env_values_redacted: bool`, `output_scrubbed: bool`, `artifact_source: Option<ArtifactSource>`
    - Add `provenance: Option<ArtifactProvenance>` field to `AnalysisReport` with `#[serde(default)]`
    - _Requirements: 8.1, 8.2, 8.3, 8.5_

  - [ ] 3.4 Implement `redact_report()` pure function in `faultline-types`
    - Implement `redact_report(report: &AnalysisReport, policy: &RedactionPolicy) -> AnalysisReport`
    - Implement `redact_env_pairs()` — replaces all `Env_Pair` values with `[REDACTED]` in `ProbeSpec.env`, `ReproductionCapsule.env`
    - Implement `scrub_command_and_output_surfaces()` — scrubs `ProbeSpec::Shell.script`, `ProbeSpec::Exec.program`, `ProbeSpec::Exec.args`, `ProbeObservation.probe_command`, `ProbeObservation.stdout`, `ProbeObservation.stderr`
    - Set `provenance` field with correct policy name, `env_values_redacted`, `output_scrubbed`
    - _Requirements: 6.1, 6.7, 9.1, 9.2, 9.3_

  - [ ] 3.5 Add `is_valid_shell_identifier()` and `to_shell_script_with_policy()` to `faultline-types`
    - Implement `is_valid_shell_identifier(s: &str) -> bool` — validates `[A-Za-z_][A-Za-z0-9_]*`
    - Implement `ReproductionCapsule::to_shell_script_with_policy(&self, policy: &RedactionPolicy) -> String`
    - Validate env keys, skip invalid ones with comment; redact values when `policy.redact_env`; scrub only command content (not structural parts) when `policy.scrub_secrets`
    - Make existing `to_shell_script()` delegate to `to_shell_script_with_policy(RedactionPolicy::default_safe())`
    - _Requirements: 6.6, 7.2_

  - [ ] 3.6 Add secret pattern fixture corpus to `faultline-fixtures`
    - Add a `secrets` module to `faultline-fixtures` with test corpus: GitHub tokens (`ghp_`, `gho_`, `ghu_`, `ghs_`, `ghr_`), AWS keys (`AKIA...`), Stripe keys (`sk-live_`, `sk-test_`), Bearer tokens, `password=` values
    - Include false-positive near-misses: `ghp_short` (too short), `AKIA` alone (no suffix), `Bearer` alone (no token), `password` alone (no `=value`)
    - Extend `arb_analysis_report()` with `arb_analysis_report_with_sentinels()` that injects UUID-prefixed sentinel env values
    - _Requirements: 9.6_

  - [ ]* 3.7 Write property tests for redaction completeness (Properties 2–6)
    - **Property 2: Env Redaction Completeness in Analysis JSON** — in `faultline-render`
    - **Property 3: Env Redaction Completeness in Markdown** — in `faultline-render`
    - **Property 4: Env Redaction Completeness in SARIF** — in `faultline-sarif`
    - **Property 5: Env Redaction Completeness in JUnit XML** — in `faultline-junit`
    - **Property 6: Env Redaction Completeness in Shell Scripts** — in `faultline-types`
    - Each uses sentinel env values with UUID-like prefixes, minimum 100 cases
    - **Validates: Requirements 6.1, 6.2, 6.4, 6.5, 6.6, 18.1, 18.2, 18.3, 18.4, 18.5**

  - [ ]* 3.8 Write property tests for store fidelity and unsafe flag (Properties 7–8)
    - **Property 7: Run Store Retains Unredacted Values** — in `faultline-store`
    - **Property 8: Unsafe Flag Preserves Raw Env Values** — in `faultline-render`
    - Minimum 100 cases each
    - **Validates: Requirements 6.7, 7.1, 7.2, 18.6**

  - [ ]* 3.9 Write property tests for provenance, round-trip, schema, idempotence, backward compat, secret scrubbing (Properties 9–14)
    - **Property 9: Provenance Correctness** — in `faultline-types`
    - **Property 10: Redaction Round-Trip Structural Validity** — in `faultline-types`
    - **Property 11: Redacted Output Conforms to JSON Schema** — in `faultline-types` or `faultline-render`
    - **Property 12: Redaction Idempotence** — in `faultline-types`
    - **Property 13: Backward-Compatible Deserialization** — in `faultline-types`
    - **Property 14: Secret Pattern Scrubbing in Shareable Artifacts** — in `faultline-types`
    - Minimum 100 cases each
    - **Validates: Requirements 7.3, 7.4, 8.1, 8.2, 8.3, 8.5, 9.1, 9.2, 9.3, 19.1, 19.2, 19.3, 19.5**

- [ ] 4. Phase B continued: Plumb `RedactionPolicy` through ALL render and export call sites
  - [ ] 4.1 Update `ReportRenderer` in `faultline-render` with policy-aware methods
    - Add `render_with_policy(&self, report, policy)` — applies `redact_report()` then writes `analysis.json` + `index.html`
    - Add `render_with_markdown_and_policy(&self, report, policy)` — applies `redact_report()` then writes `analysis.json` + `index.html` + `dossier.md`
    - Make existing `render()` delegate to `render_with_policy(report, RedactionPolicy::default_safe())`
    - Make existing `render_json_only()` delegate to policy-aware version with `RedactionPolicy::default_safe()`
    - Make existing `render_with_markdown()` delegate to `render_with_markdown_and_policy(report, RedactionPolicy::default_safe())`
    - Update standalone `render_markdown()` function to accept `RedactionPolicy` parameter
    - **Call sites updated:**
      - `faultline-render::ReportRenderer::render()`
      - `faultline-render::ReportRenderer::render_json_only()`
      - `faultline-render::ReportRenderer::render_with_markdown()`
      - `faultline-render::render_markdown()`
    - _Requirements: 6.1, 6.2, 6.3_

  - [ ] 4.2 Update `to_sarif()` in `faultline-sarif` to accept `RedactionPolicy`
    - Change signature to `to_sarif(report: &AnalysisReport, policy: &RedactionPolicy) -> Result<String, serde_json::Error>`
    - Apply `redact_report()` internally before SARIF generation
    - **Call site updated:** `faultline-sarif::to_sarif()`
    - _Requirements: 6.4_

  - [ ] 4.3 Update `to_junit_xml()` in `faultline-junit` to accept `RedactionPolicy`
    - Change signature to `to_junit_xml(report: &AnalysisReport, policy: &RedactionPolicy) -> String`
    - Apply `redact_report()` internally before JUnit generation
    - **Call site updated:** `faultline-junit::to_junit_xml()`
    - _Requirements: 6.5_

  - [ ] 4.4 Add `--unsafe-include-env` and `--unsafe-include-output` CLI flags
    - Add both flags to the `Cli` struct in `faultline-cli/src/main.rs` with `#[arg(long, default_value_t = false)]`
    - Construct `RedactionPolicy` from flags: `redact_env: !unsafe_include_env`, `scrub_secrets: !unsafe_include_output`
    - _Requirements: 7.1, 9.4, 9.5_

  - [ ] 4.5 Plumb `RedactionPolicy` through CLI localize flow
    - Pass constructed policy to `renderer.render_with_policy()` / `renderer.render_with_markdown_and_policy()` / `renderer.render_json_only_with_policy()`
    - **Call site updated:** CLI `try_main()` localize flow (constructs renderer)
    - _Requirements: 6.1, 6.2, 6.3, 7.1_

  - [ ] 4.6 Plumb `RedactionPolicy` through CLI `run_reproduce()`
    - Accept policy parameter in `run_reproduce()`
    - Use `capsule.to_shell_script_with_policy(policy)` for `--shell` output
    - Apply redaction to summary output (env values in non-shell mode)
    - **Call site updated:** CLI `run_reproduce()` (prints capsule shell/summary)
    - _Requirements: 6.6, 7.2_

  - [ ] 4.7 Plumb `RedactionPolicy` through CLI `run_export_markdown()`
    - Accept policy parameter in `run_export_markdown()`
    - Apply `redact_report()` before calling `render_markdown()`
    - **Call site updated:** CLI `run_export_markdown()` (prints markdown to stdout)
    - _Requirements: 6.2_

  - [ ] 4.8 Plumb `RedactionPolicy` through xtask export handlers
    - Update xtask `ExportMarkdown` handler to apply `redact_report()` with `RedactionPolicy::default_safe()` before rendering
    - Update xtask `ExportSarif` handler to pass `RedactionPolicy::default_safe()` to `to_sarif()`
    - Update xtask `ExportJunit` handler to pass `RedactionPolicy::default_safe()` to `to_junit_xml()`
    - **Call sites updated:**
      - xtask `ExportMarkdown` handler
      - xtask `ExportSarif` handler
      - xtask `ExportJunit` handler
    - _Requirements: 6.4, 6.5_

  - [ ] 4.9 Bump `schema_version` to `"0.3.0"` and regenerate JSON schema
    - Update `default_schema_version()` in `faultline-types` to return `"0.3.0"`
    - Run `cargo xtask generate-schema` to regenerate `schemas/analysis-report.schema.json`
    - _Requirements: 8.4_

  - [ ] 4.10 Update golden snapshots
    - Run `cargo insta review` to accept updated snapshots for:
      - `faultline-render` `analysis.json` snapshot (new `provenance` field)
      - `faultline-render` `index.html` snapshot (if HTML changes)
      - `faultline-cli` `--help` snapshot (new `--unsafe-include-env`, `--unsafe-include-output` flags, new subcommands)
    - _Requirements: 8.4_

- [ ] 5. Checkpoint — Phase B complete
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 6. Phase C: Inspect + Bundle commands
  - [ ] 6.1 Add `inspect-run` subcommand to `faultline-cli`
    - Add `InspectRun { run_dir: PathBuf, json: bool }` variant to `Commands` enum
    - Implement `run_inspect_run()`: walk run directory, identify known files (`report.json`, `analysis.json`, `observations.json`, `request.json`, `metadata.json`, `logs/`), print one-line description for each
    - When `report.json` is present, extract and display: run ID, schema version, outcome type, observation count, creation timestamp
    - When `observations.json` is present, display cached observation count
    - When `logs/` exists, display log file count
    - Error with exit code 2 if run directory does not exist
    - _Requirements: 4.1, 4.2, 4.3, 4.4, 4.5_

  - [ ] 6.2 Implement `inspect-run --json` structured output
    - Define `InspectRunOutput`, `FileEntry`, `ReportSummary` structs with `Serialize`
    - Include `report_parse_error: Option<String>` field for structured error handling
    - When `report.json` is unparseable: exit 0, set `report_summary: None`, set `report_parse_error: Some(details)`, output well-formed JSON
    - In text mode, unparseable `report.json` emits warning to stderr and continues with partial output (exit 0)
    - _Requirements: 4.6_

  - [ ] 6.3 Add `bundle` subcommand to `faultline-cli`
    - Add `Bundle { source: PathBuf, output: PathBuf, include_sarif: bool, include_junit: bool, format: BundleFormat }` variant to `Commands` enum
    - Define `BundleFormat` enum (`Dir`, `TarGz`) with `clap::ValueEnum`
    - Add `flate2` and `tar` dependencies to `faultline-cli/Cargo.toml`
    - _Requirements: 5.1, 5.5_

  - [ ] 6.4 Implement `bundle` command logic
    - Load report via `faultline_loader::locate_and_load_report()` (accepts run directory, `report.json`, or `analysis.json`)
    - Create temporary staging directory
    - Apply `RedactionPolicy` (default_safe or from CLI flags)
    - Generate ALL core artifacts fresh from loaded report: `analysis.json`, `index.html`, `dossier.md`
    - If `--include-sarif` passed, generate SARIF fresh into staging
    - If `--include-junit` passed, generate JUnit XML fresh into staging
    - For `BundleFormat::Dir`: copy staging to output directory
    - For `BundleFormat::TarGz`: create gzip-compressed tar archive at output path using `flate2` + `tar`
    - Print output path and artifact count
    - Error with exit code 2 if no loadable report in source
    - _Requirements: 5.1, 5.2, 5.3, 5.4, 5.5, 5.6, 5.7, 5.8_

  - [ ]* 6.5 Write tests for `inspect-run` and `bundle`
    - Test `inspect-run` lists files with descriptions
    - Test `inspect-run` extracts report metadata
    - Test `inspect-run --json` emits valid JSON
    - Test `inspect-run --json` with unparseable report has `report_parse_error` field
    - Test `inspect-run` errors on missing directory
    - Test `bundle` generates all core artifacts fresh
    - Test `bundle --include-sarif` adds SARIF
    - Test `bundle` without `--include-sarif` excludes SARIF
    - Test `bundle --format tar-gz` creates archive
    - Test `bundle` errors on empty source
    - Test `BundleFormat::Dir` and `TarGz` parse from clap
    - _Requirements: 4.1, 4.2, 4.5, 4.6, 5.2, 5.3, 5.4, 5.5, 5.7_

- [ ] 7. Checkpoint — Phase C complete
  - Ensure all tests pass, ask the user if questions arise.

- [ ] 8. Phase D: Documentation reality sweep
  - [ ] 8.1 Update `docs/architecture.md`
    - Fix adapter count to six (list all six: `faultline-git`, `faultline-probe-exec`, `faultline-store`, `faultline-render`, `faultline-sarif`, `faultline-junit`)
    - Describe `faultline-fixtures` as a testing utility, not an infrastructure adapter
    - Document the two-tier artifact model (Report_JSON for internal persistence, Analysis_JSON for shareable output)
    - Add the architectural invariant: Run_Store is always unredacted, Shareable_Artifacts are redacted projections
    - Add report loading table mapping each CLI subcommand and xtask export command to the shared Report_Locator
    - Document `faultline-loader` as a shared infrastructure helper crate
    - _Requirements: 1.3, 1.4, 10.1, 10.2, 10.3, 13.1, 13.2_

  - [ ] 8.2 Update `README.md`
    - Document `--env KEY=VALUE` flag with usage examples
    - Document `--retries` flag and interaction with flake-aware probing
    - Document `--stability-threshold` flag and valid range (0.0 to 1.0)
    - Document `--markdown` flag for Markdown dossier generation
    - Document `--shell` flag and accepted values (`sh`, `cmd`, `powershell`)
    - Document `--unsafe-include-env` flag
    - Document `--unsafe-include-output` flag
    - Document `cargo xtask export-markdown --run-dir <path>` with usage examples
    - Document `cargo xtask export-sarif --run-dir <path>` with usage examples
    - Document `cargo xtask export-junit --run-dir <path>` with usage examples
    - Explain that all xtask export commands use the shared Report_Locator
    - Document `ReproductionCapsule` structure and `reproduce` subcommand with `--shell` examples
    - Note that env values in shell script output are redacted by default
    - Document `FlakeSignal` structure, majority-vote logic, `--stability-threshold` behavior, and effect on confidence scoring
    - _Requirements: 11.1, 11.2, 11.3, 11.4, 11.5, 11.6, 11.7, 12.1, 12.2, 12.3, 12.4, 14.1, 14.2, 14.3, 15.1, 15.2, 15.3, 15.4_

  - [ ] 8.3 Update `TESTING.md`
    - Align CI tier descriptions: `ci-fast` runs fmt + clippy + test, `ci-full` runs ci-fast + golden + schema-check, `ci-extended` runs mutation + supply-chain
    - Explicitly state that fuzz is available as a manual tool via `cargo xtask fuzz` but is NOT part of `ci-extended` workflow
    - _Requirements: 16.1, 16.2, 16.3, 16.4_

  - [ ] 8.4 Update `docs/verification-matrix.md` and `docs/patterns/catalog.md`
    - Ensure `docs/verification-matrix.md` is consistent with TESTING.md CI tier descriptions
    - Ensure `docs/patterns/catalog.md` is consistent with TESTING.md CI tier descriptions
    - _Requirements: 16.5, 16.6_

  - [ ] 8.5 Verify `.github/workflows/` match documented tiers
    - Check `.github/workflows/ci.yml`, `.github/workflows/ci-full.yml`, `.github/workflows/ci-extended.yml` against documented tier descriptions
    - Resolve any discrepancies between workflow files and documentation
    - _Requirements: 16.7_

- [ ] 9. Final checkpoint — All phases complete
  - Ensure all tests pass, ask the user if questions arise.

## Notes

- Tasks marked with `*` are optional and can be skipped for faster MVP
- Each task references specific requirements for traceability
- Checkpoints ensure incremental validation after each phase
- Property tests validate universal correctness properties from the design document (Properties 1–14)
- Unit tests validate specific examples and edge cases
- The explicit call-site enumeration in task 4 ensures no render/export path is left unredacted
- Phase ordering (A→B→C→D) respects dependency chains: loader before redaction, redaction before bundle, all code before docs
