# Design Document: v01-artifact-hardening

## Overview

This design covers three Priority 0 hardening workstreams for faultline v0.1:

1. **Canonical artifact model** — A shared report-loading module replaces duplicated report-loading logic across CLI and xtask, establishing deterministic precedence (`report.json` > `analysis.json`) and enabling new `inspect-run` and `bundle` commands.

2. **Default-safe redaction** — Environment variable values and secret-like patterns are masked in all shareable artifacts at render/export time. The run store remains unredacted. Two independent opt-in flags (`--unsafe-include-env`, `--unsafe-include-output`) override each redaction surface independently.

3. **Documentation reality alignment** — Architecture docs, README, and TESTING.md are updated to match the actual codebase (adapter count, two-tier model, CI tiers, CLI flags, export commands).

### Design Principles

- **Redaction is a projection, not a mutation.** The `AnalysisReport` in memory and in the run store is always the full-truth, unredacted representation. Redaction happens at serialization boundaries (renderer, export adapters, shell script generation).
- **Single source of truth for report loading.** One shared loader crate, used by all consumers.
- **Pure types stay pure.** `faultline-types` contains only data types and pure functions — no filesystem I/O. Report loading I/O lives in a dedicated `faultline-loader` crate.
- **Backward compatibility.** New provenance fields use `#[serde(default)]`. Old reports deserialize without error.
- **Deterministic bundles.** Bundle contents are always generated fresh from the loaded report under the chosen policy. No opportunistic copying from source directories.
- **Phased delivery.** Four phases (A→D) with clear dependency ordering.

## Architecture

### High-Level Component Interaction

```mermaid
graph TD
    CLI[faultline-cli] --> LOADER[faultline-loader]
    XTASK[xtask] --> LOADER
    LOADER --> TYPES[faultline-types]
    LOADER --> FS[Filesystem]
    
    CLI --> APP[faultline-app]
    APP --> STORE[faultline-store]
    STORE -->|save_report: unredacted| FS
    
    APP --> RENDER[faultline-render]
    RENDER -->|RedactionPolicy| FS
    
    CLI --> SARIF[faultline-sarif]
    CLI --> JUNIT[faultline-junit]
    SARIF -->|RedactionPolicy| FS
    JUNIT -->|RedactionPolicy| FS
    
    CLI -->|reproduce --shell| CAPSULE[ReproductionCapsule]
    CAPSULE -->|RedactionPolicy| STDOUT[stdout]
```

### Crate Placement Decisions

| New Component | Crate | Rationale |
|---|---|---|
| `ArtifactSource`, `ArtifactProvenance`, `LocatedReport` (types only) | `faultline-types` | Pure data types. No I/O. Part of the canonical data model. |
| `locate_and_load_report()` (I/O function) | `faultline-loader` (new crate) | Performs filesystem I/O (read files, check existence, deserialize). Cannot live in `faultline-types` — that crate must stay free of infrastructure behavior per faultline's repo law. Infrastructure helper crate (not a port adapter). Both CLI and xtask depend on this crate. |
| `RedactionPolicy` | `faultline-types` | Part of the data model (stored in provenance fields). Used by renderer, export adapters, and CLI. |
| `SecretScrubber` | `faultline-types` | Multiple crates need it (render, sarif, junit, CLI). Pure regex-based string transformation — no I/O. |
| `ArtifactProvenance` | `faultline-types` on `AnalysisReport` | Natural extension of the report model. Replaces the three flat provenance fields with a structured type. |
| `inspect-run` subcommand | `faultline-cli` | CLI-only command. |
| `bundle` subcommand | `faultline-cli` | CLI-only command, invokes renderer internally. |

### Redaction Data Flow

```mermaid
sequenceDiagram
    participant App as faultline-app
    participant Store as faultline-store
    participant Render as faultline-render
    participant SARIF as faultline-sarif
    participant JUnit as faultline-junit

    App->>Store: save_report(report) [UNREDACTED]
    Note over Store: report.json — full truth, never redacted
    
    App->>Render: render(report, RedactionPolicy::default_safe())
    Note over Render: Apply env redaction + secret scrubbing
    Render-->>Render: analysis.json [REDACTED]
    Render-->>Render: index.html [REDACTED]
    Render-->>Render: dossier.md [REDACTED]
    
    App->>SARIF: to_sarif(report, RedactionPolicy::default_safe())
    Note over SARIF: Apply env redaction + secret scrubbing
    
    App->>JUnit: to_junit_xml(report, RedactionPolicy::default_safe())
    Note over JUnit: Apply env redaction + secret scrubbing
```

### Implementation Phasing

| Phase | Scope | Dependencies |
|---|---|---|
| **A: Shared loader** | `faultline-loader` crate with `locate_and_load_report`, CLI adoption, xtask adoption, corrected `diff-runs`, end-to-end loading tests | None |
| **B: Redaction spine** | `RedactionPolicy`, `SecretScrubber`, `ArtifactProvenance`, render-time redacted projection, export adapter redaction, `ReproductionCapsule` redaction, provenance fields, schema bump, golden updates | Phase A |
| **C: Inspect + bundle** | `inspect-run` command, `bundle` staging/render-on-demand, tar.gz support, artifact counts | Phase A, Phase B |
| **D: Docs reality sweep** | Update architecture.md, README.md, TESTING.md, verification-matrix.md, patterns/catalog.md | Phase A, B, C (docs describe final state) |

## Components and Interfaces

### 1. Report Locator Types (faultline-types) and Loader (faultline-loader)

Pure data types live in `faultline-types`. The I/O function lives in the new `faultline-loader` crate.

#### Types in `faultline-types`

```rust
/// Source of the loaded report, recorded in provenance.
/// Does NOT store filesystem paths — these are semantic tags only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum ArtifactSource {
    ReportJson,
    AnalysisJson,
    DirectFile, // No path stored — just indicates it was loaded from a direct file
}

/// Result of report location resolution.
pub struct LocatedReport {
    pub report: AnalysisReport,
    pub source: ArtifactSource,
    /// Diagnostic messages (e.g., "both files present, chose report.json")
    pub diagnostics: Vec<String>,
}
```

**Design decision: `ArtifactSource` does not store arbitrary file paths.** The previous design had `DirectFile(String)` which embedded a literal filesystem path into the serialized report. This is nondeterministic, leaks local filesystem details, makes golden artifacts noisy, and is not part of the report's semantic truth. If the actual path is needed for diagnostics, it belongs in command output or `inspect-run`, not inside the serialized `AnalysisReport`.

#### I/O in `faultline-loader` (new crate)

```rust
// crates/faultline-loader/src/lib.rs

use faultline_types::{AnalysisReport, ArtifactSource, LocatedReport, Result};
use std::path::Path;

/// Resolve and load an AnalysisReport from a path.
///
/// If `path` is a directory:
///   1. report.json if present (preferred)
///   2. analysis.json if present (fallback)
///   3. Error if neither exists
///
/// If `path` is a file:
///   Load directly from that file path.
pub fn locate_and_load_report(path: &Path) -> Result<LocatedReport> { ... }
```

**Design decision: separate `faultline-loader` crate.** The previous design placed `locate_and_load_report()` (which does file I/O) in `faultline-types`. This violates faultline's repo law: pure types/domain crates must stay free of infrastructure behavior. The new `faultline-loader` crate:
- Depends on `faultline-types` (for `AnalysisReport`, `ArtifactSource`, `LocatedReport`)
- Contains the filesystem I/O logic (read files, check existence, deserialize)
- Is depended on by `faultline-cli` and `xtask`
- Is NOT depended on by any domain or adapter crate

This keeps the dependency direction clean: `faultline-loader` is a thin infrastructure helper crate — not a port adapter like `faultline-git` or `faultline-store`, but a shared report/artifact loading utility at the same dependency tier. It does not implement any port trait; it provides a convenience function used by entry-point crates (CLI, xtask) to resolve and load reports from the filesystem.

### 2. RedactionPolicy (faultline-types)

```rust
/// Controls what gets redacted in shareable artifacts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RedactionPolicy {
    /// Mask environment variable values with "[REDACTED]".
    pub redact_env: bool,
    /// Scrub secret-like patterns from stdout/stderr and command surfaces.
    pub scrub_secrets: bool,
}

impl RedactionPolicy {
    /// Default policy: redact env values, scrub secrets.
    pub fn default_safe() -> Self {
        Self { redact_env: true, scrub_secrets: true }
    }

    /// No redaction (--unsafe-include-env + --unsafe-include-output).
    pub fn none() -> Self {
        Self { redact_env: false, scrub_secrets: false }
    }

    /// Env values exposed, secrets still scrubbed (--unsafe-include-env only).
    pub fn env_exposed() -> Self {
        Self { redact_env: false, scrub_secrets: true }
    }

    /// Env values redacted, secrets exposed (--unsafe-include-output only).
    pub fn secrets_exposed() -> Self {
        Self { redact_env: true, scrub_secrets: false }
    }

    /// The policy name string stored in provenance.
    pub fn name(&self) -> &'static str {
        match (self.redact_env, self.scrub_secrets) {
            (true, true) => "default",
            (false, false) => "none",
            (false, true) => "env-exposed",
            (true, false) => "secrets-exposed",
        }
    }
}
```

### 3. SecretScrubber (faultline-types)

```rust
use regex::Regex;
use std::sync::LazyLock;

/// Conservative set of high-confidence secret patterns.
static SECRET_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| vec![
    // GitHub tokens: ghp_, gho_, ghu_, ghs_, ghr_ followed by alphanumeric
    Regex::new(r"(gh[pousr]_)[A-Za-z0-9]{36,}").unwrap(),
    // AWS access keys: AKIA followed by 16 uppercase alphanumeric
    Regex::new(r"(AKIA)[A-Z0-9]{16}").unwrap(),
    // Stripe keys: sk-live_ or sk-test_ followed by alphanumeric
    Regex::new(r"(sk-(?:live|test)_)[A-Za-z0-9]{24,}").unwrap(),
    // Bearer tokens: "Bearer " followed by non-whitespace
    Regex::new(r"(Bearer )\S+").unwrap(),
    // password= followed by non-whitespace value
    Regex::new(r"(password=)\S+").unwrap(),
]);

/// Scrub secret-like patterns from a string, replacing matched
/// portions with [REDACTED] while preserving the prefix for context.
pub fn scrub_secrets(input: &str) -> String { ... }
```

**Design decision: `std::sync::LazyLock` instead of `once_cell`.** Rust stable now has `std::sync::LazyLock`. No need for the `once_cell` dependency.

**Design decision:** The regex set is intentionally conservative (high-confidence patterns only) to minimize false positives. The prefix (e.g., `ghp_`, `Bearer `) is preserved in the output so the user can see *what kind* of secret was redacted. The replacement format is `{prefix}[REDACTED]`.

### 4. ArtifactProvenance (faultline-types)

```rust
/// Structured provenance metadata for shareable artifacts.
/// Replaces the previous flat fields (redaction_policy, redaction_applied, artifact_source)
/// with two explicit booleans covering the two independent redaction surfaces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ArtifactProvenance {
    /// Policy name: "default", "none", "env-exposed", "secrets-exposed"
    pub redaction_policy: String,
    /// Whether env variable values were masked with [REDACTED]
    pub env_values_redacted: bool,
    /// Whether stdout/stderr secret patterns were scrubbed
    pub output_scrubbed: bool,
    /// Which file the report was loaded from
    pub artifact_source: Option<ArtifactSource>,
}
```

**Design decision: two explicit provenance booleans instead of one.** The previous design had a single `redaction_applied: Option<bool>` field but TWO independent redaction surfaces (env values and output scrubbing). The old logic set it from `policy.redact_env` only, which was wrong for cases like:
- `--unsafe-include-env` + default output scrubbing → env not redacted, output scrubbed
- env redacted + `--unsafe-include-output` → env redacted, output not scrubbed

The new `ArtifactProvenance` struct makes both surfaces explicit and independently queryable.

### 5. Redacted Projection Functions (faultline-types)

```rust
/// Apply redaction policy to an AnalysisReport, producing a new report
/// suitable for serialization into shareable artifacts.
///
/// This is a pure function — it does not modify the input report.
pub fn redact_report(report: &AnalysisReport, policy: &RedactionPolicy) -> AnalysisReport {
    let mut redacted = report.clone();
    
    if policy.redact_env {
        redact_env_pairs(&mut redacted);
    }
    if policy.scrub_secrets {
        scrub_command_and_output_surfaces(&mut redacted);
    }
    
    // Set provenance fields
    redacted.provenance = Some(ArtifactProvenance {
        redaction_policy: policy.name().to_string(),
        env_values_redacted: policy.redact_env,
        output_scrubbed: policy.scrub_secrets,
        artifact_source: None, // set by caller if known
    });
    
    redacted
}

/// Replace all env values with "[REDACTED]" in probe specs,
/// reproduction capsules, and any other Env_Pair locations.
fn redact_env_pairs(report: &mut AnalysisReport) { ... }

/// Scrub secret patterns from all free-text command surfaces
/// and observation stdout/stderr fields.
///
/// Surfaces scrubbed:
/// - ProbeSpec::Shell.script
/// - ProbeSpec::Exec.program
/// - ProbeSpec::Exec.args (each element)
/// - ProbeObservation.probe_command
/// - ProbeObservation.stdout
/// - ProbeObservation.stderr
///
/// NOT scrubbed (too noisy / not a realistic leak vector):
/// - ProbeObservation.working_dir
/// - AnalysisRequest.repo_root
fn scrub_command_and_output_surfaces(report: &mut AnalysisReport) { ... }
```

**Design decision: scrub all serialized free-text command surfaces, not just stdout/stderr.** The report model contains multiple free-text fields that can carry secrets: shell scripts, CLI args (e.g., `--token=...`), authorization headers passed as args, inline `TOKEN=... cmd` style predicates, and recorded probe commands. Since `analysis.json` serializes the full request and observation model, all of these surfaces must be scrubbed when `scrub_secrets` is active. `working_dir` and `repo_root` are excluded because path scrubbing tends to produce noisy false positives.

### 6. Updated Renderer Interface (faultline-render)

```rust
impl ReportRenderer {
    /// Render with redaction policy applied.
    pub fn render_with_policy(
        &self,
        report: &AnalysisReport,
        policy: &RedactionPolicy,
    ) -> Result<()> {
        let redacted = redact_report(report, policy);
        // Write analysis.json, index.html from redacted report
        ...
    }

    /// Render JSON + HTML + Markdown with redaction.
    pub fn render_with_markdown_and_policy(
        &self,
        report: &AnalysisReport,
        policy: &RedactionPolicy,
    ) -> Result<()> {
        let redacted = redact_report(report, policy);
        // Write analysis.json, index.html, dossier.md from redacted report
        ...
    }
}
```

**Design decision:** The existing `render()` and `render_with_markdown()` methods are preserved for backward compatibility but internally delegate to the policy-aware versions with `RedactionPolicy::default_safe()`. This ensures all existing call sites get redaction by default without code changes.

### 7. Updated Export Adapter Interfaces

```rust
// faultline-sarif
pub fn to_sarif(
    report: &AnalysisReport,
    policy: &RedactionPolicy,
) -> Result<String, serde_json::Error> {
    let redacted = redact_report(report, policy);
    // Existing SARIF generation on redacted report
    ...
}

// faultline-junit
pub fn to_junit_xml(
    report: &AnalysisReport,
    policy: &RedactionPolicy,
) -> String {
    let redacted = redact_report(report, policy);
    // Existing JUnit generation on redacted report
    ...
}
```

### 8. Updated ReproductionCapsule Interface

```rust
impl ReproductionCapsule {
    /// Generate a shell script with redaction applied.
    pub fn to_shell_script_with_policy(&self, policy: &RedactionPolicy) -> String {
        let mut script = String::from("#!/bin/sh\nset -e\n");
        // ... cd, git checkout ...
        
        for (key, value) in &self.env {
            if !is_valid_shell_identifier(key) {
                // Skip with a comment in the script
                script.push_str(&format!("# skipped invalid env key: {}\n", key));
                continue;
            }
            let display_value = if policy.redact_env {
                "[REDACTED]".to_string()
            } else {
                shell_escape(value)
            };
            script.push_str(&format!("export {}='{}'\n", key, display_value));
        }
        
        // Apply secret scrubbing to the command/script portion ONLY,
        // not the entire assembled script (structural parts are safe)
        match &self.predicate {
            ProbeSpec::Shell { script: shell_script, .. } => {
                let cmd_content = if policy.scrub_secrets {
                    scrub_secrets(shell_script)
                } else {
                    shell_script.clone()
                };
                script.push_str(&format!(
                    "timeout {} sh -c '{}'\n",
                    self.timeout_seconds,
                    shell_escape(&cmd_content),
                ));
            }
            ProbeSpec::Exec { program, args, env: probe_env, .. } => {
                // Export probe-level env vars (same validation)
                for (key, value) in probe_env {
                    if !is_valid_shell_identifier(key) {
                        script.push_str(&format!("# skipped invalid env key: {}\n", key));
                        continue;
                    }
                    let display_value = if policy.redact_env {
                        "[REDACTED]".to_string()
                    } else {
                        shell_escape(value)
                    };
                    script.push_str(&format!("export {}='{}'\n", key, display_value));
                }
                let mut cmd_parts = vec![format!("'{}'", shell_escape(program))];
                for arg in args {
                    cmd_parts.push(format!("'{}'", shell_escape(arg)));
                }
                script.push_str(&format!(
                    "timeout {} {}\n",
                    self.timeout_seconds,
                    cmd_parts.join(" "),
                ));
            }
        }
        
        script
    }
}

/// Validate that a string is a valid POSIX shell identifier: [A-Za-z_][A-Za-z0-9_]*
fn is_valid_shell_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}
```

**Design decision: env key validation, not escaping.** The previous design used `shell_escape(key)` for env variable names in `export` lines. Shell variable names are NOT arbitrary escaped strings — they must be valid identifiers (`[A-Za-z_][A-Za-z0-9_]*`). Invalid keys are skipped with a comment in the generated script rather than producing broken shell syntax.

**Design decision: narrow secret scrubbing scope.** Instead of running `scrub_secrets()` over the entire assembled shell script (which could accidentally match structural parts like the shebang, `cd`, `git checkout`, or `export` lines), scrubbing is applied only to the interpolated free-text surfaces: the `script` field content (for Shell variant) and stdout/stderr content if it appears in comments. Structural parts are never scrubbed.

### 9. Provenance Fields on AnalysisReport (faultline-types)

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AnalysisReport {
    // ... existing fields ...
    
    /// Structured provenance metadata. None for reports produced before this change.
    #[serde(default)]
    pub provenance: Option<ArtifactProvenance>,
}
```

**Design decision:** Provenance is a single `Option<ArtifactProvenance>` with `#[serde(default)]` so that reports produced before this change deserialize without error. The `schema_version` will be bumped to `"0.3.0"` to reflect the schema addition.

> **Schema version semantics:** Schema version is the artifact-contract version, independent of the workspace crate version. It is bumped when the `AnalysisReport` structure changes in a way that affects consumers. The current bump from "0.2.0" to "0.3.0" reflects the addition of the `provenance` field.

### 10. inspect-run Command (faultline-cli)

```rust
#[derive(Debug, Subcommand)]
enum Commands {
    // ... existing commands ...
    
    /// Explain the layout of a completed run directory
    InspectRun {
        /// Path to the run directory
        #[arg(long)]
        run_dir: PathBuf,
        
        /// Emit JSON output instead of human-readable text
        #[arg(long, default_value_t = false)]
        json: bool,
    },
}
```

The command walks the run directory, identifies known files (`report.json`, `analysis.json`, `observations.json`, `request.json`, `metadata.json`, `logs/`), and prints a one-line description for each. When `report.json` is present, it extracts and displays run ID, schema version, outcome type, observation count, and creation timestamp.

#### inspect-run --json structured error handling

For `--json` mode, the output is always a well-formed JSON object, even when `report.json` is unparseable:

```rust
#[derive(Serialize)]
struct InspectRunOutput {
    discovered_files: Vec<FileEntry>,
    report_summary: Option<ReportSummary>,
    report_parse_error: Option<String>,
    observation_count: Option<usize>,
    log_file_count: Option<usize>,
}
```

When `report.json` is unparseable in `--json` mode:
- Still exit 0
- Set `report_summary: None`
- Set `report_parse_error: Some("failed to parse report.json: {details}")`
- The JSON output is always well-formed and complete

In human-readable text mode, an unparseable `report.json` emits a warning to stderr and continues with partial output (exit 0).

### 11. bundle Command (faultline-cli)

```rust
#[derive(Debug, Clone, clap::ValueEnum)]
enum BundleFormat {
    Dir,
    TarGz,
}

#[derive(Debug, Subcommand)]
enum Commands {
    // ... existing commands ...
    
    /// Package shareable artifacts from a run into a directory or archive
    Bundle {
        /// Path to a run directory, report.json, or analysis.json
        #[arg(long)]
        source: PathBuf,
        
        /// Output destination (directory or .tar.gz path)
        #[arg(long)]
        output: PathBuf,
        
        /// Include SARIF output in the bundle
        #[arg(long, default_value_t = false)]
        include_sarif: bool,
        
        /// Include JUnit XML output in the bundle
        #[arg(long, default_value_t = false)]
        include_junit: bool,
        
        /// Output format
        #[arg(long, default_value_t = BundleFormat::Dir, value_enum)]
        format: BundleFormat,
    },
}
```

**Design decision: `BundleFormat` is a typed enum, not a raw string.** Using `clap::ValueEnum` eliminates one unnecessary validation path — clap rejects invalid values at parse time.

**Bundle workflow:**
1. Load report via `faultline-loader::locate_and_load_report` (accepts a run directory, `report.json`, or `analysis.json`)
2. Create a temporary staging directory
3. Apply `RedactionPolicy::default_safe()` (or policy from flags)
4. Generate ALL core artifacts fresh from the loaded report: `analysis.json`, `index.html`, `dossier.md`
5. If `--include-sarif` is explicitly passed, generate SARIF fresh into staging
6. If `--include-junit` is explicitly passed, generate JUnit XML fresh into staging
7. Copy staging to output directory, or create tar.gz archive
8. Print output path and artifact count

**Design decision: no opportunistic copying.** The previous design said "Optionally generate SARIF/JUnit if flags are set or files already exist in source." The "or files already exist in source" part creates hidden variability: same command, same report, different output depending on what old files happen to be lying around. Bundle contents are now ALWAYS generated fresh from the loaded report under the chosen policy. SARIF and JUnit are included only when their respective flags are explicitly passed.

### 12. CLI Flag Additions

```rust
struct Cli {
    // ... existing fields ...
    
    /// Include raw environment variable values in shareable artifacts
    /// (UNSAFE: may leak secrets)
    #[arg(long, default_value_t = false)]
    unsafe_include_env: bool,
    
    /// Include raw stdout/stderr output without secret scrubbing
    /// (UNSAFE: may leak tokens)
    #[arg(long, default_value_t = false)]
    unsafe_include_output: bool,
}
```

The CLI constructs a `RedactionPolicy` from these flags:
```rust
let policy = RedactionPolicy {
    redact_env: !cli.unsafe_include_env,
    scrub_secrets: !cli.unsafe_include_output,
};
```

## Data Models

### Updated AnalysisReport Schema (v0.3.0)

New field added to `AnalysisReport`:

| Field | Type | Default | Description |
|---|---|---|---|
| `provenance` | `Option<ArtifactProvenance>` | `None` | Structured provenance metadata (policy, redaction state, source) |

### ArtifactProvenance

| Field | Type | Description |
|---|---|---|
| `redaction_policy` | `String` | Policy name: `"default"`, `"none"`, `"env-exposed"`, `"secrets-exposed"` |
| `env_values_redacted` | `bool` | `true` if env values were masked with `[REDACTED]` |
| `output_scrubbed` | `bool` | `true` if secret patterns were scrubbed from stdout/stderr and command surfaces |
| `artifact_source` | `Option<ArtifactSource>` | Which file the report was loaded from |

### RedactionPolicy

| Field | Type | Description |
|---|---|---|
| `redact_env` | `bool` | Whether to mask env values with `[REDACTED]` |
| `scrub_secrets` | `bool` | Whether to scrub secret patterns from stdout/stderr and command surfaces (shell scripts, program args, probe commands) |

### ArtifactSource

| Variant | Description |
|---|---|
| `ReportJson` | Loaded from `report.json` in a directory |
| `AnalysisJson` | Loaded from `analysis.json` in a directory |
| `DirectFile` | Loaded from a direct file path (no path stored — just a semantic tag) |

### BundleFormat

| Variant | Description |
|---|---|
| `Dir` | Output as a directory of files |
| `TarGz` | Output as a gzip-compressed tar archive |

### InspectRunOutput (--json mode)

| Field | Type | Description |
|---|---|---|
| `discovered_files` | `Vec<FileEntry>` | Files found in the run directory with descriptions |
| `report_summary` | `Option<ReportSummary>` | Extracted report metadata (None if unparseable) |
| `report_parse_error` | `Option<String>` | Parse error details (None if report parsed OK) |
| `observation_count` | `Option<usize>` | Number of cached observations |
| `log_file_count` | `Option<usize>` | Number of log files in logs/ |

### Secret Pattern Set

| Pattern | Matches | Example |
|---|---|---|
| `gh[pousr]_[A-Za-z0-9]{36,}` | GitHub tokens | `ghp_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx` |
| `AKIA[A-Z0-9]{16}` | AWS access key IDs | `AKIAIOSFODNN7EXAMPLE` |
| `sk-(?:live\|test)_[A-Za-z0-9]{24,}` | Stripe API keys | `sk-live_xxxxxxxxxxxxxxxxxxxxxxxx` |
| `Bearer \S+` | Bearer tokens | `Bearer eyJhbGciOiJIUzI1NiJ9...` |
| `password=\S+` | Password parameters | `password=hunter2` |

### Run Directory Layout (for inspect-run)

| File | Tier | Description |
|---|---|---|
| `request.json` | Internal | Original analysis request parameters |
| `observations.json` | Internal | Cached probe observations |
| `report.json` | Internal | Full unredacted AnalysisReport |
| `metadata.json` | Internal | Schema version and tool version |
| `.lock` | Internal | Process lock file |
| `logs/` | Internal | Full probe stdout/stderr logs |
| `analysis.json` | Shareable | Redacted AnalysisReport for sharing |
| `index.html` | Shareable | Human-readable HTML report |
| `dossier.md` | Shareable | Markdown dossier |


## Correctness Properties

*A property is a characteristic or behavior that should hold true across all valid executions of a system — essentially, a formal statement about what the system should do. Properties serve as the bridge between human-readable specifications and machine-verifiable correctness guarantees.*

### Property 1: Report Locator Precedence

*For any* directory containing any combination of `report.json` and `analysis.json` files, the `locate_and_load_report` function SHALL return the file matching the deterministic precedence: `report.json` when present, `analysis.json` as fallback, and an error when neither exists. When both files are present, the diagnostics list SHALL be non-empty.

**Validates: Requirements 2.1, 2.2**

### Property 2: Env Redaction Completeness in Analysis JSON

*For any* valid `AnalysisReport` with non-empty `Env_Pair` values containing unique sentinel prefixes, rendering to `analysis.json` with `RedactionPolicy::default_safe()` SHALL produce output that contains zero occurrences of any raw sentinel env value and at least one occurrence of `[REDACTED]`.

**Validates: Requirements 6.1, 18.1**

### Property 3: Env Redaction Completeness in Markdown

*For any* valid `AnalysisReport` with non-empty `Env_Pair` values containing unique sentinel prefixes, rendering to Markdown dossier with `RedactionPolicy::default_safe()` SHALL produce output that contains zero occurrences of any raw sentinel env value.

**Validates: Requirements 6.2, 18.2**

### Property 4: Env Redaction Completeness in SARIF

*For any* valid `AnalysisReport` with non-empty `Env_Pair` values containing unique sentinel prefixes, exporting to SARIF with `RedactionPolicy::default_safe()` SHALL produce output that contains zero occurrences of any raw sentinel env value.

**Validates: Requirements 6.4, 18.3**

### Property 5: Env Redaction Completeness in JUnit XML

*For any* valid `AnalysisReport` with non-empty `Env_Pair` values containing unique sentinel prefixes, exporting to JUnit XML with `RedactionPolicy::default_safe()` SHALL produce output that contains zero occurrences of any raw sentinel env value.

**Validates: Requirements 6.5, 18.4**

### Property 6: Env Redaction Completeness in Shell Scripts

*For any* valid `ReproductionCapsule` with non-empty `Env_Pair` values containing unique sentinel prefixes, generating a shell script with `RedactionPolicy::default_safe()` SHALL produce output that contains zero occurrences of any raw sentinel env value and at least one occurrence of `[REDACTED]`.

**Validates: Requirements 6.6, 18.5**

### Property 7: Run Store Retains Unredacted Values

*For any* valid `AnalysisReport` with non-empty `Env_Pair` values, saving to the run store via `save_report()` and then loading via `load_report()` SHALL preserve all original env values exactly. The store NEVER applies redaction.

**Validates: Requirements 6.7**

### Property 8: Unsafe Flag Preserves Raw Env Values

*For any* valid `AnalysisReport` with non-empty `Env_Pair` values containing unique sentinel prefixes, rendering with `RedactionPolicy { redact_env: false, .. }` SHALL produce output that contains every raw sentinel env value.

**Validates: Requirements 7.1, 7.2, 18.6**

### Property 9: Provenance Correctness

*For any* valid `AnalysisReport` and any `RedactionPolicy`, applying `redact_report()` SHALL produce a report whose `provenance` field is `Some` with `env_values_redacted` equal to `policy.redact_env`, `output_scrubbed` equal to `policy.scrub_secrets`, and `redaction_policy` equal to `policy.name()`.

**Validates: Requirements 7.3, 7.4, 8.1, 8.2, 8.3**

### Property 10: Redaction Round-Trip Structural Validity

*For any* valid `AnalysisReport`, applying `redact_report()` and then serializing to JSON and deserializing back SHALL produce a valid `AnalysisReport` with intact structure (env values will be `[REDACTED]` but all fields are present and correctly typed).

**Validates: Requirements 19.1**

### Property 11: Redacted Output Conforms to JSON Schema

*For any* valid `AnalysisReport`, the redacted `analysis.json` output SHALL validate against the JSON Schema at `schemas/analysis-report.schema.json`.

**Validates: Requirements 19.2**

### Property 12: Redaction Idempotence

*For any* valid `AnalysisReport`, applying `redact_report()` twice with the same policy SHALL produce identical output to applying it once. Formally: `redact(redact(report, policy), policy) == redact(report, policy)`.

**Validates: Requirements 19.5**

### Property 13: Backward-Compatible Deserialization

*For any* valid `AnalysisReport` JSON produced before the provenance field was added (i.e., missing `provenance`), deserialization into the current `AnalysisReport` struct SHALL succeed with the `provenance` field defaulting to `None`.

**Validates: Requirements 8.5, 19.3**

### Property 14: Secret Pattern Scrubbing in Shareable Artifacts

*For any* valid `AnalysisReport` with observation `stdout`, `stderr`, `probe_command` fields, or `ProbeSpec` shell script / program / args fields containing injected secret-pattern strings (GitHub tokens, AWS keys, Stripe keys, Bearer tokens, password= values), rendering with `RedactionPolicy::default_safe()` SHALL produce shareable artifact output that contains zero occurrences of the injected secret strings while preserving the pattern prefix for context.

**Validates: Requirements 9.1, 9.2, 9.3**

## Error Handling

### ReportLocator Errors (faultline-loader)

| Condition | Error | Exit Code |
|---|---|---|
| Directory exists but contains neither `report.json` nor `analysis.json` | `FaultlineError::Store("no report found in {path}: expected report.json or analysis.json")` | 2 |
| Direct file path does not exist | `FaultlineError::Store("report file not found: {path}")` | 2 |
| File exists but contains invalid JSON | `FaultlineError::Serde("failed to parse {path}: {details}")` | 2 |
| File exists but does not deserialize into `AnalysisReport` | `FaultlineError::Serde("failed to deserialize {path}: {details}")` | 2 |

### Bundle Command Errors

| Condition | Error | Exit Code |
|---|---|---|
| No loadable report in source directory | `FaultlineError::Store("no report found")` | 2 |
| Output directory cannot be created | `FaultlineError::Io("failed to create {path}")` | 2 |
| tar.gz creation fails | `FaultlineError::Io("failed to create archive")` | 2 |

### Inspect-Run Errors

| Condition | Behavior (text mode) | Behavior (--json mode) | Exit Code |
|---|---|---|---|
| Run directory does not exist | Error message to stderr | Error message to stderr | 2 |
| report.json exists but is unparseable | Warning to stderr, continue with partial output | `report_summary: None`, `report_parse_error: Some(...)`, well-formed JSON output | 0 |

### Redaction Errors

Redaction is infallible by design. The `redact_report()` function operates on cloned data and cannot fail. Secret pattern scrubbing uses pre-compiled regexes that cannot fail at runtime. If a regex somehow fails to compile at startup (programming error), the process panics during `LazyLock` initialization — this is caught by unit tests.

### Diagnostic Messages

All diagnostic messages from the report loader go to stderr when the command streams structured output to stdout (export-markdown, export-sarif, export-junit). This prevents corruption of the output stream.

## Testing Strategy

### Property-Based Testing

This feature is well-suited for property-based testing. The core redaction logic is a pure function (`redact_report`) with clear input/output behavior, and the universal property "no raw secret values in shareable output" should hold across all valid inputs.

**Library:** `proptest` (already used throughout the codebase)
**Minimum iterations:** 100 per property test
**Tag format:** `Feature: v01-artifact-hardening, Property {N}: {title}`

#### Generator Extensions

The existing `arb_analysis_report()` generator in `faultline-fixtures` needs extension to produce reports with:
- Non-empty env pairs containing unique sentinel prefixes (UUID-like) to avoid false positives
- Observation stdout/stderr containing injected secret patterns
- Reproduction capsules with env pairs

New generator: `arb_analysis_report_with_sentinels()` that wraps `arb_analysis_report()` and injects sentinel env values with a known prefix (e.g., `SENTINEL_<uuid>_`) so tests can search for the prefix to detect leaks.

#### Property Test Placement

| Property | Crate | Rationale |
|---|---|---|
| 1 (Locator precedence) | `faultline-loader` | Tests the loader module's I/O logic |
| 2 (JSON redaction) | `faultline-render` | Tests render output |
| 3 (Markdown redaction) | `faultline-render` | Tests markdown output |
| 4 (SARIF redaction) | `faultline-sarif` | Tests SARIF export |
| 5 (JUnit redaction) | `faultline-junit` | Tests JUnit export |
| 6 (Shell redaction) | `faultline-types` | Tests capsule shell generation |
| 7 (Store unredacted) | `faultline-store` | Tests store persistence |
| 8 (Unsafe flag) | `faultline-render` | Tests render with policy override |
| 9 (Provenance correctness) | `faultline-types` | Tests redact_report pure function |
| 10 (Round-trip) | `faultline-types` | Tests serialization |
| 11 (Schema conformance) | `faultline-types` or `faultline-render` | Tests against JSON schema |
| 12 (Idempotence) | `faultline-types` | Tests redact_report pure function |
| 13 (Backward compat) | `faultline-types` | Tests deserialization |
| 14 (Secret scrubbing) | `faultline-types` | Tests SecretScrubber |

### Unit Tests (Example-Based)

| Test | Crate | Validates |
|---|---|---|
| Locator returns report.json when both present | `faultline-loader` | Req 2.1 |
| Locator returns analysis.json when only it exists | `faultline-loader` | Req 2.1 |
| Locator emits diagnostic when both present | `faultline-loader` | Req 2.2 |
| Locator loads direct file path | `faultline-loader` | Req 2.3 |
| Locator errors on empty directory | `faultline-loader` | Req 2.1 |
| diff-runs accepts report.json path | `faultline-cli` | Req 3.2 |
| diff-runs accepts analysis.json path | `faultline-cli` | Req 3.3 |
| diff-runs errors on missing file | `faultline-cli` | Req 3.4 |
| diff-runs errors on invalid JSON | `faultline-cli` | Req 3.5 |
| inspect-run lists files with descriptions | `faultline-cli` | Req 4.1 |
| inspect-run extracts report metadata | `faultline-cli` | Req 4.2 |
| inspect-run --json emits valid JSON | `faultline-cli` | Req 4.6 |
| inspect-run --json with unparseable report has structured error | `faultline-cli` | Req 4.6 |
| inspect-run errors on missing directory | `faultline-cli` | Req 4.5 |
| bundle generates all artifacts fresh | `faultline-cli` | Req 5.2 |
| bundle includes core artifacts | `faultline-cli` | Req 5.3 |
| bundle --include-sarif adds SARIF | `faultline-cli` | Req 5.4 |
| bundle without --include-sarif excludes SARIF | `faultline-cli` | Req 5.4 |
| bundle --format tar-gz creates archive | `faultline-cli` | Req 5.5 |
| bundle errors on empty source | `faultline-cli` | Req 5.7 |
| provenance env_values_redacted=true when redacting | `faultline-types` | Req 7.4 |
| provenance env_values_redacted=false with unsafe flag | `faultline-types` | Req 7.3 |
| provenance output_scrubbed=true by default | `faultline-types` | Req 8.2 |
| provenance output_scrubbed=false with --unsafe-include-output | `faultline-types` | Req 8.2 |
| provenance fields present in output | `faultline-types` | Req 8.1-8.3 |
| old reports without provenance deserialize | `faultline-types` | Req 19.3 |
| new reports with provenance deserialize | `faultline-types` | Req 19.4 |
| --unsafe-include-env still scrubs secrets | `faultline-render` | Req 9.4 |
| --unsafe-include-output is independent | `faultline-cli` | Req 9.5 |
| shell script skips invalid env key names | `faultline-types` | Req 6.6 |
| is_valid_shell_identifier accepts valid names | `faultline-types` | Req 6.6 |
| is_valid_shell_identifier rejects invalid names | `faultline-types` | Req 6.6 |
| BundleFormat::Dir and TarGz parse from clap | `faultline-cli` | Req 5.5 |

### Integration Tests (End-to-End Artifact Layout)

| Test | Validates |
|---|---|
| CLI reproduce loads from report.json-only directory | Req 17.1 |
| CLI reproduce loads from analysis.json-only directory | Req 17.2 |
| CLI export-markdown loads from report.json-only directory | Req 17.1 |
| CLI export-markdown loads from analysis.json-only directory | Req 17.2 |
| diff-runs loads report.json file path | Req 17.3 |
| diff-runs loads analysis.json file path | Req 17.3 |
| xtask export-markdown loads from report.json-only directory | Req 17.4 |
| xtask export-markdown loads from analysis.json-only directory | Req 17.5 |
| xtask export-sarif loads from report.json-only directory | Req 17.4 |
| xtask export-sarif loads from analysis.json-only directory | Req 17.5 |
| xtask export-junit loads from report.json-only directory | Req 17.4 |
| xtask export-junit loads from analysis.json-only directory | Req 17.5 |
| All commands error on empty directory | Req 17.6 |

### Secret Pattern Fixture Corpus (Req 9.6)

A fixture module in `faultline-fixtures` containing:

| Category | Examples |
|---|---|
| GitHub tokens | `ghp_ABCDEFghijklmnop1234567890abcdef1234`, `gho_...`, `ghu_...`, `ghs_...`, `ghr_...` |
| AWS access keys | `AKIAIOSFODNN7EXAMPLE` |
| Stripe keys | `sk-live_abcdefghijklmnopqrstuvwx`, `sk-test_...` |
| Bearer tokens | `Bearer eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U` |
| Password params | `password=hunter2`, `password=s3cr3t!@#` |
| False positives | `ghp_short` (too short), `AKIA` (no suffix), `Bearer` (no token), `password` (no =value) |

### Documentation Tests

| Test | Validates |
|---|---|
| docs/architecture.md contains "six" adapters | Req 10.1 |
| docs/architecture.md describes fixtures as testing utility | Req 10.2 |
| docs/architecture.md describes two-tier model | Req 10.3 |
| README.md documents all new CLI flags | Req 11.1-11.7 |
| README.md documents xtask export commands | Req 12.1-12.4 |
| TESTING.md CI tier descriptions match workflows | Req 16.1-16.7 |

### Golden/Snapshot Test Updates

The following golden tests will need updates after this change:
- `faultline-render` analysis.json snapshot (new provenance field)
- `faultline-render` index.html snapshot (if HTML changes)
- `faultline-cli` --help snapshot (new subcommands and flags)
- JSON Schema at `schemas/analysis-report.schema.json` (new fields including `ArtifactProvenance`)
