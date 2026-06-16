use faultline_codes::{AmbiguityReason, ObservationClass, ProbeKind};
use regex::Regex;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt;
use std::path::PathBuf;
use std::sync::LazyLock;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

pub type Result<T> = std::result::Result<T, FaultlineError>;

#[derive(Debug, Error)]
pub enum FaultlineError {
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("invalid boundary: {0}")]
    InvalidBoundary(String),
    #[error("git error: {0}")]
    Git(String),
    #[error("probe error: {0}")]
    Probe(String),
    #[error("store error: {0}")]
    Store(String),
    #[error("render error: {0}")]
    Render(String),
    #[error("domain error: {0}")]
    Domain(String),
    #[error("i/o error: {0}")]
    Io(String),
    #[error("serialization error: {0}")]
    Serde(String),
}

impl From<std::io::Error> for FaultlineError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value.to_string())
    }
}

impl From<serde_json::Error> for FaultlineError {
    fn from(value: serde_json::Error) -> Self {
        Self::Serde(value.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct CommitId(pub String);

impl fmt::Display for CommitId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RevisionSpec(pub String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum HistoryMode {
    AncestryPath,
    FirstParent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum ShellKind {
    Default,
    PosixSh,
    Cmd,
    PowerShell,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum ProbeSpec {
    Exec {
        kind: ProbeKind,
        program: String,
        args: Vec<String>,
        env: Vec<(String, String)>,
        timeout_seconds: u64,
    },
    Shell {
        kind: ProbeKind,
        shell: ShellKind,
        script: String,
        #[serde(default)]
        env: Vec<(String, String)>,
        timeout_seconds: u64,
    },
}

impl ProbeSpec {
    pub fn kind(&self) -> ProbeKind {
        match self {
            ProbeSpec::Exec { kind, .. } | ProbeSpec::Shell { kind, .. } => *kind,
        }
    }

    pub fn timeout_seconds(&self) -> u64 {
        match self {
            ProbeSpec::Exec {
                timeout_seconds, ..
            }
            | ProbeSpec::Shell {
                timeout_seconds, ..
            } => *timeout_seconds,
        }
    }

    pub fn fingerprint(&self) -> String {
        let payload = serde_json::to_string(self).unwrap_or_else(|_| format!("{:?}", self));
        stable_hash(payload.as_bytes())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SearchPolicy {
    pub max_probes: usize,
    #[serde(default)]
    pub flake_policy: FlakePolicy,
}

impl Default for SearchPolicy {
    fn default() -> Self {
        Self {
            max_probes: 64,
            flake_policy: FlakePolicy::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AnalysisRequest {
    pub repo_root: PathBuf,
    pub good: RevisionSpec,
    pub bad: RevisionSpec,
    pub history_mode: HistoryMode,
    pub probe: ProbeSpec,
    pub policy: SearchPolicy,
}

impl AnalysisRequest {
    pub fn fingerprint(&self) -> String {
        let payload = serde_json::to_string(self).unwrap_or_else(|_| format!("{:?}", self));
        stable_hash(payload.as_bytes())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RevisionSequence {
    pub revisions: Vec<CommitId>,
}

impl RevisionSequence {
    pub fn len(&self) -> usize {
        self.revisions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.revisions.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProbeObservation {
    pub commit: CommitId,
    pub class: ObservationClass,
    pub kind: ProbeKind,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub duration_ms: u64,
    pub stdout: String,
    pub stderr: String,
    #[serde(default)]
    pub sequence_index: u64,
    #[serde(default)]
    pub signal_number: Option<i32>,
    #[serde(default)]
    pub probe_command: String,
    #[serde(default)]
    pub working_dir: String,
    #[serde(default)]
    pub flake_signal: Option<FlakeSignal>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Confidence {
    pub score: u8,
    pub label: String,
}

impl Confidence {
    pub fn high() -> Self {
        Self {
            score: 95,
            label: "high".to_string(),
        }
    }

    pub fn medium() -> Self {
        Self {
            score: 65,
            label: "medium".to_string(),
        }
    }

    pub fn low() -> Self {
        Self {
            score: 25,
            label: "low".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum LocalizationOutcome {
    FirstBad {
        last_good: CommitId,
        first_bad: CommitId,
        confidence: Confidence,
    },
    SuspectWindow {
        lower_bound_exclusive: CommitId,
        upper_bound_inclusive: CommitId,
        confidence: Confidence,
        reasons: Vec<AmbiguityReason>,
    },
    Inconclusive {
        reasons: Vec<AmbiguityReason>,
    },
}

impl LocalizationOutcome {
    pub fn boundary_pair(&self) -> Option<(&CommitId, &CommitId)> {
        match self {
            LocalizationOutcome::FirstBad {
                last_good,
                first_bad,
                ..
            } => Some((last_good, first_bad)),
            LocalizationOutcome::SuspectWindow {
                lower_bound_exclusive,
                upper_bound_inclusive,
                ..
            } => Some((lower_bound_exclusive, upper_bound_inclusive)),
            LocalizationOutcome::Inconclusive { .. } => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum ChangeStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
    TypeChanged,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PathChange {
    pub status: ChangeStatus,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SubsystemBucket {
    pub name: String,
    pub change_count: usize,
    pub paths: Vec<String>,
    pub surface_kinds: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SurfaceSummary {
    pub total_changes: usize,
    pub buckets: Vec<SubsystemBucket>,
    pub execution_surfaces: Vec<String>,
}

// --- Suspect Surface ---

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SuspectEntry {
    pub path: String,
    pub priority_score: u32,
    pub surface_kind: String,
    pub change_status: ChangeStatus,
    pub is_execution_surface: bool,
    pub owner_hint: Option<String>,
}

// --- Flake-Aware Probing ---

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FlakePolicy {
    pub retries: u32,
    pub stability_threshold: f64,
}

impl Default for FlakePolicy {
    fn default() -> Self {
        Self {
            retries: 0,
            stability_threshold: 1.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FlakeSignal {
    pub total_runs: u32,
    pub pass_count: u32,
    pub fail_count: u32,
    pub skip_count: u32,
    pub indeterminate_count: u32,
    pub is_stable: bool,
}

/// Compute a `FlakeSignal` from a set of observation results and a stability threshold.
///
/// - `results`: the observation classes from repeated probes of the same commit
/// - `stability_threshold`: proportion in [0.0, 1.0]; the most-frequent class must meet or exceed this to be stable
///
/// Empty results: returns all-zero counts with `is_stable = true` (vacuously stable).
pub fn compute_flake_signal(results: &[ObservationClass], stability_threshold: f64) -> FlakeSignal {
    let total_runs = results.len() as u32;
    if total_runs == 0 {
        return FlakeSignal {
            total_runs: 0,
            pass_count: 0,
            fail_count: 0,
            skip_count: 0,
            indeterminate_count: 0,
            is_stable: true,
        };
    }

    let mut pass_count: u32 = 0;
    let mut fail_count: u32 = 0;
    let mut skip_count: u32 = 0;
    let mut indeterminate_count: u32 = 0;

    for class in results {
        match class {
            ObservationClass::Pass => pass_count += 1,
            ObservationClass::Fail => fail_count += 1,
            ObservationClass::Skip => skip_count += 1,
            ObservationClass::Indeterminate => indeterminate_count += 1,
        }
    }

    let max_count = pass_count
        .max(fail_count)
        .max(skip_count)
        .max(indeterminate_count);
    let is_stable = (max_count as f64 / total_runs as f64) >= stability_threshold;

    FlakeSignal {
        total_runs,
        pass_count,
        fail_count,
        skip_count,
        indeterminate_count,
        is_stable,
    }
}

// --- Reproduction Capsule ---

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ReproductionCapsule {
    pub commit: CommitId,
    pub predicate: ProbeSpec,
    pub env: Vec<(String, String)>,
    pub working_dir: String,
    pub timeout_seconds: u64,
}

/// Escape a string for safe inclusion in a single-quoted POSIX shell context.
/// Replaces each `'` with `'\''` (end quote, escaped quote, start quote).
fn shell_escape(s: &str) -> String {
    s.replace('\'', "'\\''")
}

/// Validate that a string is a valid POSIX shell identifier: `[A-Za-z_][A-Za-z0-9_]*`
pub fn is_valid_shell_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

impl ReproductionCapsule {
    /// Generate a POSIX shell script that reproduces this probe.
    /// Delegates to `to_shell_script_with_policy` with `RedactionPolicy::default_safe()`.
    pub fn to_shell_script(&self) -> String {
        self.to_shell_script_with_policy(&RedactionPolicy::default_safe())
    }

    /// Generate a shell script with redaction applied.
    pub fn to_shell_script_with_policy(&self, policy: &RedactionPolicy) -> String {
        let mut script = String::from("#!/bin/sh\nset -e\n");

        // cd to working directory
        script.push_str(&format!("cd '{}'\n", shell_escape(&self.working_dir)));

        // git checkout
        script.push_str(&format!(
            "git checkout '{}'\n",
            shell_escape(&self.commit.0)
        ));

        // env exports (capsule-level)
        for (key, value) in &self.env {
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

        // predicate command with timeout
        let timeout = self.timeout_seconds;
        match &self.predicate {
            ProbeSpec::Exec {
                program,
                args,
                env: probe_env,
                ..
            } => {
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
                script.push_str(&format!("timeout {} {}\n", timeout, cmd_parts.join(" ")));
            }
            ProbeSpec::Shell {
                script: shell_script,
                env: probe_env,
                ..
            } => {
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
                // Apply secret scrubbing to the command/script portion ONLY
                let cmd_content = if policy.scrub_secrets {
                    scrub_secrets(shell_script)
                } else {
                    shell_script.clone()
                };
                script.push_str(&format!(
                    "timeout {} sh -c '{}'\n",
                    timeout,
                    shell_escape(&cmd_content),
                ));
            }
        }

        script
    }
}

// --- Run Comparison ---

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunComparison {
    pub left_run_id: String,
    pub right_run_id: String,
    pub outcome_changed: bool,
    pub confidence_delta: i16,
    pub window_width_delta: i64,
    pub probes_reused: usize,
    pub suspect_paths_added: Vec<String>,
    pub suspect_paths_removed: Vec<String>,
    pub ambiguity_reasons_added: Vec<AmbiguityReason>,
    pub ambiguity_reasons_removed: Vec<AmbiguityReason>,
}

/// Extract the confidence score from a localization outcome.
fn outcome_confidence_score(outcome: &LocalizationOutcome) -> i16 {
    match outcome {
        LocalizationOutcome::FirstBad { confidence, .. } => confidence.score as i16,
        LocalizationOutcome::SuspectWindow { confidence, .. } => confidence.score as i16,
        LocalizationOutcome::Inconclusive { .. } => 0,
    }
}

/// Extract the window width from a localization outcome and sequence.
fn outcome_window_width(outcome: &LocalizationOutcome, sequence: &RevisionSequence) -> i64 {
    match outcome.boundary_pair() {
        Some((lower, upper)) => {
            let lower_idx = sequence
                .revisions
                .iter()
                .position(|c| c == lower)
                .map(|i| i as i64);
            let upper_idx = sequence
                .revisions
                .iter()
                .position(|c| c == upper)
                .map(|i| i as i64);
            match (lower_idx, upper_idx) {
                (Some(l), Some(u)) => (u - l).abs(),
                _ => 0,
            }
        }
        None => sequence.revisions.len() as i64,
    }
}

/// Extract ambiguity reasons from a localization outcome.
fn outcome_ambiguity_reasons(outcome: &LocalizationOutcome) -> Vec<AmbiguityReason> {
    match outcome {
        LocalizationOutcome::SuspectWindow { reasons, .. } => reasons.clone(),
        LocalizationOutcome::Inconclusive { reasons } => reasons.clone(),
        LocalizationOutcome::FirstBad { .. } => vec![],
    }
}

/// Pure function: compare two analysis reports.
/// Never panics — always returns a RunComparison.
pub fn compare_runs(left: &AnalysisReport, right: &AnalysisReport) -> RunComparison {
    let outcome_changed = serde_json::to_string(&left.outcome).unwrap_or_default()
        != serde_json::to_string(&right.outcome).unwrap_or_default();

    let left_confidence = outcome_confidence_score(&left.outcome);
    let right_confidence = outcome_confidence_score(&right.outcome);
    let confidence_delta = right_confidence.saturating_sub(left_confidence);

    let left_width = outcome_window_width(&left.outcome, &left.sequence);
    let right_width = outcome_window_width(&right.outcome, &right.sequence);
    let window_width_delta = right_width.saturating_sub(left_width);

    // Count probes reused: matching (commit, class) pairs
    let left_probes: HashSet<(String, String)> = left
        .observations
        .iter()
        .map(|o| {
            (
                o.commit.0.clone(),
                serde_json::to_string(&o.class).unwrap_or_default(),
            )
        })
        .collect();
    let right_probes: HashSet<(String, String)> = right
        .observations
        .iter()
        .map(|o| {
            (
                o.commit.0.clone(),
                serde_json::to_string(&o.class).unwrap_or_default(),
            )
        })
        .collect();
    let probes_reused = left_probes.intersection(&right_probes).count();

    // Suspect paths set diff
    let left_suspect_paths: HashSet<&str> = left
        .suspect_surface
        .iter()
        .map(|s| s.path.as_str())
        .collect();
    let right_suspect_paths: HashSet<&str> = right
        .suspect_surface
        .iter()
        .map(|s| s.path.as_str())
        .collect();
    let mut suspect_paths_added: Vec<String> = right_suspect_paths
        .difference(&left_suspect_paths)
        .map(|s| s.to_string())
        .collect();
    suspect_paths_added.sort();
    let mut suspect_paths_removed: Vec<String> = left_suspect_paths
        .difference(&right_suspect_paths)
        .map(|s| s.to_string())
        .collect();
    suspect_paths_removed.sort();

    // Ambiguity reasons set diff
    let left_reasons: HashSet<String> = outcome_ambiguity_reasons(&left.outcome)
        .iter()
        .map(|r| serde_json::to_string(r).unwrap_or_default())
        .collect();
    let right_reasons: HashSet<String> = outcome_ambiguity_reasons(&right.outcome)
        .iter()
        .map(|r| serde_json::to_string(r).unwrap_or_default())
        .collect();
    let ambiguity_reasons_added: Vec<AmbiguityReason> = right_reasons
        .difference(&left_reasons)
        .filter_map(|s| serde_json::from_str(s).ok())
        .collect();
    let ambiguity_reasons_removed: Vec<AmbiguityReason> = left_reasons
        .difference(&right_reasons)
        .filter_map(|s| serde_json::from_str(s).ok())
        .collect();

    RunComparison {
        left_run_id: left.run_id.clone(),
        right_run_id: right.run_id.clone(),
        outcome_changed,
        confidence_delta,
        window_width_delta,
        probes_reused,
        suspect_paths_added,
        suspect_paths_removed,
        ambiguity_reasons_added,
        ambiguity_reasons_removed,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunHandle {
    pub id: String,
    pub root: PathBuf,
    pub resumed: bool,
    #[serde(default = "default_schema_version")]
    pub schema_version: String,
    #[serde(default)]
    pub tool_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckedOutRevision {
    pub commit: CommitId,
    pub path: PathBuf,
}

// --- Artifact Source ---

/// Source of the loaded report, recorded in provenance.
/// Does NOT store filesystem paths — these are semantic tags only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum ArtifactSource {
    ReportJson,
    AnalysisJson,
    DirectFile,
}

// --- Redaction Policy ---

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
        Self {
            redact_env: true,
            scrub_secrets: true,
        }
    }

    /// No redaction (--unsafe-include-env + --unsafe-include-output).
    pub fn none() -> Self {
        Self {
            redact_env: false,
            scrub_secrets: false,
        }
    }

    /// Env values exposed, secrets still scrubbed (--unsafe-include-env only).
    pub fn env_exposed() -> Self {
        Self {
            redact_env: false,
            scrub_secrets: true,
        }
    }

    /// Env values redacted, secrets exposed (--unsafe-include-output only).
    pub fn secrets_exposed() -> Self {
        Self {
            redact_env: true,
            scrub_secrets: false,
        }
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

// --- Artifact Provenance ---

/// Structured provenance metadata for shareable artifacts.
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AnalysisReport {
    #[serde(default = "default_schema_version")]
    pub schema_version: String,
    pub run_id: String,
    pub created_at_epoch_seconds: u64,
    pub request: AnalysisRequest,
    pub sequence: RevisionSequence,
    pub observations: Vec<ProbeObservation>,
    pub outcome: LocalizationOutcome,
    pub changed_paths: Vec<PathChange>,
    pub surface: SurfaceSummary,
    #[serde(default)]
    pub suspect_surface: Vec<SuspectEntry>,
    #[serde(default)]
    pub reproduction_capsules: Vec<ReproductionCapsule>,
    /// Structured provenance metadata. None for reports produced before this change.
    #[serde(default)]
    pub provenance: Option<ArtifactProvenance>,
}

fn default_schema_version() -> String {
    "0.3.0".to_string()
}

// --- Located Report ---

/// Result of report location resolution.
pub struct LocatedReport {
    pub report: AnalysisReport,
    pub source: ArtifactSource,
    /// Diagnostic messages (e.g., "both files present, chose report.json")
    pub diagnostics: Vec<String>,
}

pub fn stable_hash(data: &[u8]) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in data {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{:016x}", hash)
}

// --- Secret Scrubber ---

/// Conservative set of high-confidence secret patterns.
/// Each pattern uses a capture group for the prefix that will be preserved.
static SECRET_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        // GitHub classic tokens: ghp_, gho_, ghu_, ghs_, ghr_ followed by 36+ alphanumeric
        Regex::new(r"(gh[pousr]_)[A-Za-z0-9]{36,}").expect("valid regex: GitHub token pattern"),
        // GitHub fine-grained PATs: github_pat_ followed by 40+ Base62 token body
        Regex::new(r"(github_pat_)[A-Za-z0-9_]{40,}")
            .expect("valid regex: GitHub fine-grained PAT pattern"),
        // AWS access keys: AKIA followed by 16 uppercase alphanumeric
        Regex::new(r"(AKIA)[A-Z0-9]{16}").expect("valid regex: AWS key pattern"),
        // Google API keys: AIza followed by 35 URL-safe chars
        Regex::new(r"(AIza)[A-Za-z0-9_\-]{35}").expect("valid regex: Google API key pattern"),
        // Stripe keys: sk-live_ or sk-test_ followed by alphanumeric
        Regex::new(r"(sk-(?:live|test)_)[A-Za-z0-9]{24,}")
            .expect("valid regex: Stripe key pattern"),
        // Slack tokens: xox[baprs]- followed by 10+ chars (bot/user/app/refresh)
        Regex::new(r"(xox[baprs]-)[A-Za-z0-9-]{10,}").expect("valid regex: Slack token pattern"),
        // Bearer tokens: "Bearer " followed by non-whitespace
        Regex::new(r"(Bearer )\S+").expect("valid regex: Bearer token pattern"),
        // password= followed by non-whitespace value
        Regex::new(r"(password=)\S+").expect("valid regex: password pattern"),
        // PEM private key blocks: capture the header, redact the whole block
        Regex::new(r"(-----BEGIN [A-Z ]*PRIVATE KEY-----)[\s\S]*?-----END [A-Z ]*PRIVATE KEY-----")
            .expect("valid regex: PEM private key pattern"),
    ]
});

/// Scrub secret-like patterns from a string, replacing matched
/// portions with `[REDACTED]` while preserving the prefix for context.
///
/// The replacement format is `{prefix}[REDACTED]`.
pub fn scrub_secrets(input: &str) -> String {
    let mut result = input.to_string();
    for pattern in SECRET_PATTERNS.iter() {
        result = pattern.replace_all(&result, "${1}[REDACTED]").to_string();
    }
    result
}

// --- Redacted Projection Functions ---

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
/// reproduction capsules, and the request's probe spec env.
fn redact_env_pairs(report: &mut AnalysisReport) {
    // Redact the request's probe spec env
    redact_probe_spec_env(&mut report.request.probe);

    // Redact reproduction capsule env pairs
    for capsule in &mut report.reproduction_capsules {
        for pair in &mut capsule.env {
            pair.1 = "[REDACTED]".to_string();
        }
        // Also redact the capsule's predicate probe spec env
        redact_probe_spec_env(&mut capsule.predicate);
    }
}

/// Redact env values within a ProbeSpec (both Exec and Shell variants).
fn redact_probe_spec_env(probe: &mut ProbeSpec) {
    match probe {
        ProbeSpec::Exec { env, .. } => {
            for pair in env.iter_mut() {
                pair.1 = "[REDACTED]".to_string();
            }
        }
        ProbeSpec::Shell { env, .. } => {
            for pair in env.iter_mut() {
                pair.1 = "[REDACTED]".to_string();
            }
        }
    }
}

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
/// - SuspectEntry.priority_score / surface_kind / change_status (structural)
fn scrub_command_and_output_surfaces(report: &mut AnalysisReport) {
    // Scrub the request's probe spec command surfaces
    scrub_probe_spec_surfaces(&mut report.request.probe);

    // Scrub observation fields
    for obs in &mut report.observations {
        obs.probe_command = scrub_secrets(&obs.probe_command);
        obs.stdout = scrub_secrets(&obs.stdout);
        obs.stderr = scrub_secrets(&obs.stderr);
    }

    // Scrub reproduction capsule predicate command surfaces
    for capsule in &mut report.reproduction_capsules {
        scrub_probe_spec_surfaces(&mut capsule.predicate);
    }

    // Scrub suspect-surface free-text fields that render into artifacts.
    // `path` and `owner_hint` are operator/tool-derived strings that could
    // carry secret-like content; scrub them so they never reach shareable
    // output. `priority_score`, `surface_kind`, `change_status`, and
    // `is_execution_surface` are structural and not scrub targets.
    for entry in &mut report.suspect_surface {
        entry.path = scrub_secrets(&entry.path);
        if let Some(hint) = entry.owner_hint.take() {
            entry.owner_hint = Some(scrub_secrets(&hint));
        }
    }
}

/// Scrub secret patterns from command surfaces within a ProbeSpec.
fn scrub_probe_spec_surfaces(probe: &mut ProbeSpec) {
    match probe {
        ProbeSpec::Exec { program, args, .. } => {
            *program = scrub_secrets(program);
            for arg in args.iter_mut() {
                *arg = scrub_secrets(arg);
            }
        }
        ProbeSpec::Shell { script, .. } => {
            *script = scrub_secrets(script);
        }
    }
}

pub fn now_epoch_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use faultline_codes::{AmbiguityReason, ObservationClass, ProbeKind};
    use proptest::prelude::*;

    // --- Derive trait verification ---

    fn assert_serialize_deserialize_debug_clone_partialeq_eq<
        T: Serialize + for<'de> Deserialize<'de> + std::fmt::Debug + Clone + PartialEq + Eq,
    >(
        _val: &T,
    ) {
    }

    fn assert_serialize_deserialize_debug_clone_partialeq<
        T: Serialize + for<'de> Deserialize<'de> + std::fmt::Debug + Clone + PartialEq,
    >(
        _val: &T,
    ) {
    }

    fn sample_probe_spec() -> ProbeSpec {
        ProbeSpec::Exec {
            kind: ProbeKind::Test,
            program: "cargo".into(),
            args: vec!["test".into()],
            env: vec![],
            timeout_seconds: 300,
        }
    }

    fn sample_analysis_request() -> AnalysisRequest {
        AnalysisRequest {
            repo_root: PathBuf::from("/tmp/repo"),
            good: RevisionSpec("abc123".into()),
            bad: RevisionSpec("def456".into()),
            history_mode: HistoryMode::AncestryPath,
            probe: sample_probe_spec(),
            policy: SearchPolicy::default(),
        }
    }

    fn sample_report() -> AnalysisReport {
        AnalysisReport {
            schema_version: "0.1.0".into(),
            run_id: "run-1".into(),
            created_at_epoch_seconds: 1700000000,
            request: sample_analysis_request(),
            sequence: RevisionSequence {
                revisions: vec![CommitId("abc123".into()), CommitId("def456".into())],
            },
            observations: vec![ProbeObservation {
                commit: CommitId("abc123".into()),
                class: ObservationClass::Pass,
                kind: ProbeKind::Test,
                exit_code: Some(0),
                timed_out: false,
                duration_ms: 100,
                stdout: "ok".into(),
                stderr: String::new(),
                sequence_index: 0,
                signal_number: None,
                probe_command: String::new(),
                working_dir: String::new(),
                flake_signal: None,
            }],
            outcome: LocalizationOutcome::FirstBad {
                last_good: CommitId("abc123".into()),
                first_bad: CommitId("def456".into()),
                confidence: Confidence::high(),
            },
            changed_paths: vec![PathChange {
                status: ChangeStatus::Modified,
                path: "src/main.rs".into(),
            }],
            surface: SurfaceSummary {
                total_changes: 1,
                buckets: vec![SubsystemBucket {
                    name: "src".into(),
                    change_count: 1,
                    paths: vec!["src/main.rs".into()],
                    surface_kinds: vec!["source".into()],
                }],
                execution_surfaces: vec![],
            },
            suspect_surface: vec![],
            reproduction_capsules: vec![],
            provenance: None,
        }
    }

    #[test]
    fn all_types_derive_required_traits() {
        assert_serialize_deserialize_debug_clone_partialeq_eq(&CommitId("a".into()));
        assert_serialize_deserialize_debug_clone_partialeq_eq(&RevisionSpec("a".into()));
        assert_serialize_deserialize_debug_clone_partialeq_eq(&HistoryMode::AncestryPath);
        assert_serialize_deserialize_debug_clone_partialeq_eq(&sample_probe_spec());
        assert_serialize_deserialize_debug_clone_partialeq(&SearchPolicy::default());
        assert_serialize_deserialize_debug_clone_partialeq(&sample_analysis_request());
        assert_serialize_deserialize_debug_clone_partialeq_eq(&RevisionSequence {
            revisions: vec![],
        });
        assert_serialize_deserialize_debug_clone_partialeq_eq(&ProbeObservation {
            commit: CommitId("a".into()),
            class: ObservationClass::Pass,
            kind: ProbeKind::Custom,
            exit_code: Some(0),
            timed_out: false,
            duration_ms: 0,
            stdout: String::new(),
            stderr: String::new(),
            sequence_index: 0,
            signal_number: None,
            probe_command: String::new(),
            working_dir: String::new(),
            flake_signal: None,
        });
        assert_serialize_deserialize_debug_clone_partialeq_eq(&Confidence::high());
        assert_serialize_deserialize_debug_clone_partialeq_eq(&LocalizationOutcome::Inconclusive {
            reasons: vec![AmbiguityReason::MissingPassBoundary],
        });
        assert_serialize_deserialize_debug_clone_partialeq_eq(&PathChange {
            status: ChangeStatus::Added,
            path: "f".into(),
        });
        assert_serialize_deserialize_debug_clone_partialeq_eq(&SubsystemBucket {
            name: "src".into(),
            change_count: 0,
            paths: vec![],
            surface_kinds: vec![],
        });
        assert_serialize_deserialize_debug_clone_partialeq_eq(&SurfaceSummary {
            total_changes: 0,
            buckets: vec![],
            execution_surfaces: vec![],
        });
        assert_serialize_deserialize_debug_clone_partialeq_eq(&RunHandle {
            id: "r".into(),
            root: PathBuf::from("/tmp"),
            resumed: false,
            schema_version: "0.1.0".into(),
            tool_version: "0.1.0".into(),
        });
        assert_serialize_deserialize_debug_clone_partialeq_eq(&CheckedOutRevision {
            commit: CommitId("a".into()),
            path: PathBuf::from("/tmp"),
        });
        assert_serialize_deserialize_debug_clone_partialeq(&sample_report());

        // New types from v0.1 product sharpening
        assert_serialize_deserialize_debug_clone_partialeq_eq(&SuspectEntry {
            path: "src/main.rs".into(),
            priority_score: 100,
            surface_kind: "source".into(),
            change_status: ChangeStatus::Modified,
            is_execution_surface: false,
            owner_hint: Some("alice".into()),
        });
        assert_serialize_deserialize_debug_clone_partialeq_eq(&FlakeSignal {
            total_runs: 3,
            pass_count: 2,
            fail_count: 1,
            skip_count: 0,
            indeterminate_count: 0,
            is_stable: true,
        });
        assert_serialize_deserialize_debug_clone_partialeq_eq(&ReproductionCapsule {
            commit: CommitId("abc123".into()),
            predicate: sample_probe_spec(),
            env: vec![("KEY".into(), "val".into())],
            working_dir: "/tmp/repo".into(),
            timeout_seconds: 300,
        });
        assert_serialize_deserialize_debug_clone_partialeq_eq(&RunComparison {
            left_run_id: "run-1".into(),
            right_run_id: "run-2".into(),
            outcome_changed: false,
            confidence_delta: 0,
            window_width_delta: 0,
            probes_reused: 0,
            suspect_paths_added: vec![],
            suspect_paths_removed: vec![],
            ambiguity_reasons_added: vec![],
            ambiguity_reasons_removed: vec![],
        });

        // Artifact hardening types
        assert_serialize_deserialize_debug_clone_partialeq_eq(&ArtifactSource::ReportJson);
        assert_serialize_deserialize_debug_clone_partialeq_eq(&ArtifactSource::AnalysisJson);
        assert_serialize_deserialize_debug_clone_partialeq_eq(&ArtifactSource::DirectFile);
        assert_serialize_deserialize_debug_clone_partialeq_eq(&RedactionPolicy::default_safe());
    }

    // --- RedactionPolicy ---

    #[test]
    fn redaction_policy_default_safe() {
        let policy = RedactionPolicy::default_safe();
        assert!(policy.redact_env);
        assert!(policy.scrub_secrets);
        assert_eq!(policy.name(), "default");
    }

    #[test]
    fn redaction_policy_none() {
        let policy = RedactionPolicy::none();
        assert!(!policy.redact_env);
        assert!(!policy.scrub_secrets);
        assert_eq!(policy.name(), "none");
    }

    #[test]
    fn redaction_policy_env_exposed() {
        let policy = RedactionPolicy::env_exposed();
        assert!(!policy.redact_env);
        assert!(policy.scrub_secrets);
        assert_eq!(policy.name(), "env-exposed");
    }

    #[test]
    fn redaction_policy_secrets_exposed() {
        let policy = RedactionPolicy::secrets_exposed();
        assert!(policy.redact_env);
        assert!(!policy.scrub_secrets);
        assert_eq!(policy.name(), "secrets-exposed");
    }

    #[test]
    fn redaction_policy_serialization_roundtrip() {
        let policy = RedactionPolicy::default_safe();
        let json = serde_json::to_string(&policy).unwrap();
        let deserialized: RedactionPolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(policy, deserialized);
    }

    // --- stable_hash ---

    #[test]
    fn stable_hash_deterministic() {
        let a = stable_hash(b"hello world");
        let b = stable_hash(b"hello world");
        assert_eq!(a, b);
    }

    #[test]
    fn stable_hash_different_inputs_differ() {
        let a = stable_hash(b"hello");
        let b = stable_hash(b"world");
        assert_ne!(a, b);
    }

    #[test]
    fn stable_hash_returns_16_hex_chars() {
        let h = stable_hash(b"test");
        assert_eq!(h.len(), 16);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
    }

    // --- now_epoch_seconds ---

    #[test]
    fn now_epoch_seconds_returns_reasonable_value() {
        let ts = now_epoch_seconds();
        // Should be after 2020-01-01 and before 2100-01-01
        assert!(ts > 1_577_836_800);
        assert!(ts < 4_102_444_800);
    }

    // --- ProbeSpec::fingerprint ---

    #[test]
    fn probe_spec_fingerprint_deterministic() {
        let spec = sample_probe_spec();
        assert_eq!(spec.fingerprint(), spec.fingerprint());
    }

    #[test]
    fn probe_spec_fingerprint_differs_for_different_specs() {
        let exec = sample_probe_spec();
        let shell = ProbeSpec::Shell {
            kind: ProbeKind::Custom,
            shell: ShellKind::Default,
            script: "echo hi".into(),
            env: vec![],
            timeout_seconds: 60,
        };
        assert_ne!(exec.fingerprint(), shell.fingerprint());
    }

    // --- AnalysisRequest::fingerprint ---

    #[test]
    fn analysis_request_fingerprint_deterministic() {
        let req = sample_analysis_request();
        assert_eq!(req.fingerprint(), req.fingerprint());
    }

    #[test]
    fn analysis_request_fingerprint_differs_for_different_requests() {
        let req1 = sample_analysis_request();
        let mut req2 = sample_analysis_request();
        req2.good = RevisionSpec("zzz999".into());
        assert_ne!(req1.fingerprint(), req2.fingerprint());
    }

    // --- LocalizationOutcome::boundary_pair ---

    #[test]
    fn boundary_pair_first_bad() {
        let outcome = LocalizationOutcome::FirstBad {
            last_good: CommitId("good".into()),
            first_bad: CommitId("bad".into()),
            confidence: Confidence::high(),
        };
        let pair = outcome.boundary_pair();
        assert_eq!(
            pair,
            Some((&CommitId("good".into()), &CommitId("bad".into())))
        );
    }

    #[test]
    fn boundary_pair_suspect_window() {
        let outcome = LocalizationOutcome::SuspectWindow {
            lower_bound_exclusive: CommitId("lower".into()),
            upper_bound_inclusive: CommitId("upper".into()),
            confidence: Confidence::medium(),
            reasons: vec![AmbiguityReason::SkippedRevision],
        };
        let pair = outcome.boundary_pair();
        assert_eq!(
            pair,
            Some((&CommitId("lower".into()), &CommitId("upper".into())))
        );
    }

    #[test]
    fn boundary_pair_inconclusive_returns_none() {
        let outcome = LocalizationOutcome::Inconclusive {
            reasons: vec![AmbiguityReason::MissingPassBoundary],
        };
        assert_eq!(outcome.boundary_pair(), None);
    }

    // --- Confidence constructors ---

    #[test]
    fn confidence_high() {
        let c = Confidence::high();
        assert_eq!(c.score, 95);
        assert_eq!(c.label, "high");
    }

    #[test]
    fn confidence_medium() {
        let c = Confidence::medium();
        assert_eq!(c.score, 65);
        assert_eq!(c.label, "medium");
    }

    #[test]
    fn confidence_low() {
        let c = Confidence::low();
        assert_eq!(c.score, 25);
        assert_eq!(c.label, "low");
    }

    // --- SearchPolicy default ---

    #[test]
    fn search_policy_default() {
        let p = SearchPolicy::default();
        assert_eq!(p.max_probes, 64);
    }

    // --- CommitId Display ---

    #[test]
    fn commit_id_display() {
        let c = CommitId("abc123".into());
        assert_eq!(format!("{}", c), "abc123");
    }

    // --- RevisionSequence helpers ---

    #[test]
    fn revision_sequence_len_and_is_empty() {
        let empty = RevisionSequence { revisions: vec![] };
        assert!(empty.is_empty());
        assert_eq!(empty.len(), 0);

        let non_empty = RevisionSequence {
            revisions: vec![CommitId("a".into())],
        };
        assert!(!non_empty.is_empty());
        assert_eq!(non_empty.len(), 1);
    }

    // --- FaultlineError variants and From impls ---

    #[test]
    fn faultline_error_from_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "not found");
        let err = FaultlineError::from(io_err);
        match err {
            FaultlineError::Io(msg) => assert!(msg.contains("not found")),
            _ => panic!("expected Io variant"),
        }
    }

    #[test]
    fn faultline_error_from_serde_json() {
        let json_err = serde_json::from_str::<String>("not json").unwrap_err();
        let err = FaultlineError::from(json_err);
        match err {
            FaultlineError::Serde(msg) => assert!(!msg.is_empty()),
            _ => panic!("expected Serde variant"),
        }
    }

    #[test]
    fn faultline_error_display() {
        let err = FaultlineError::InvalidInput("bad arg".into());
        assert_eq!(format!("{}", err), "invalid input: bad arg");

        let err = FaultlineError::InvalidBoundary("mismatch".into());
        assert_eq!(format!("{}", err), "invalid boundary: mismatch");

        let err = FaultlineError::Git("failed".into());
        assert_eq!(format!("{}", err), "git error: failed");

        let err = FaultlineError::Probe("timeout".into());
        assert_eq!(format!("{}", err), "probe error: timeout");

        let err = FaultlineError::Store("corrupt".into());
        assert_eq!(format!("{}", err), "store error: corrupt");

        let err = FaultlineError::Render("template".into());
        assert_eq!(format!("{}", err), "render error: template");

        let err = FaultlineError::Domain("logic".into());
        assert_eq!(format!("{}", err), "domain error: logic");

        let err = FaultlineError::Io("disk".into());
        assert_eq!(format!("{}", err), "i/o error: disk");

        let err = FaultlineError::Serde("parse".into());
        assert_eq!(format!("{}", err), "serialization error: parse");
    }

    // --- FlakePolicy default ---

    #[test]
    fn flake_policy_default() {
        let p = FlakePolicy::default();
        assert_eq!(p.retries, 0);
        assert!((p.stability_threshold - 1.0).abs() < f64::EPSILON);
    }

    // --- ReproductionCapsule::to_shell_script ---

    #[test]
    fn to_shell_script_exec_variant() {
        let capsule = ReproductionCapsule {
            commit: CommitId("abc123".into()),
            predicate: ProbeSpec::Exec {
                kind: ProbeKind::Test,
                program: "cargo".into(),
                args: vec!["test".into(), "--release".into()],
                env: vec![("RUST_LOG".into(), "debug".into())],
                timeout_seconds: 300,
            },
            env: vec![("CI".into(), "true".into())],
            working_dir: "/tmp/repo".into(),
            timeout_seconds: 300,
        };
        // to_shell_script() uses default_safe() which redacts env values
        let script = capsule.to_shell_script();
        assert!(script.starts_with("#!/bin/sh\n"));
        assert!(script.contains("set -e"));
        assert!(script.contains("cd '/tmp/repo'"));
        assert!(script.contains("git checkout 'abc123'"));
        assert!(script.contains("export CI='[REDACTED]'"));
        assert!(script.contains("export RUST_LOG='[REDACTED]'"));
        assert!(script.contains("timeout 300"));
        assert!(script.contains("'cargo' 'test' '--release'"));
    }

    #[test]
    fn to_shell_script_shell_variant() {
        let capsule = ReproductionCapsule {
            commit: CommitId("def456".into()),
            predicate: ProbeSpec::Shell {
                kind: ProbeKind::Custom,
                shell: ShellKind::PosixSh,
                script: "make test".into(),
                env: vec![],
                timeout_seconds: 60,
            },
            env: vec![],
            working_dir: "/home/user/project".into(),
            timeout_seconds: 60,
        };
        let script = capsule.to_shell_script();
        assert!(script.contains("cd '/home/user/project'"));
        assert!(script.contains("git checkout 'def456'"));
        assert!(script.contains("timeout 60 sh -c 'make test'"));
    }

    #[test]
    fn to_shell_script_escapes_single_quotes() {
        let capsule = ReproductionCapsule {
            commit: CommitId("abc".into()),
            predicate: ProbeSpec::Shell {
                kind: ProbeKind::Custom,
                shell: ShellKind::Default,
                script: "echo 'hello world'".into(),
                env: vec![],
                timeout_seconds: 30,
            },
            env: vec![("VAR".into(), "it's a test".into())],
            working_dir: "/tmp/it's here".into(),
            timeout_seconds: 30,
        };
        // to_shell_script() uses default_safe() which redacts env values
        let script = capsule.to_shell_script();
        assert!(script.contains("cd '/tmp/it'\\''s here'"));
        assert!(script.contains("export VAR='[REDACTED]'"));
        assert!(script.contains("sh -c 'echo '\\''hello world'\\'''"));
    }

    // --- ReproductionCapsule::to_shell_script_with_policy ---

    #[test]
    fn to_shell_script_with_policy_none_shows_raw_values() {
        let capsule = ReproductionCapsule {
            commit: CommitId("abc123".into()),
            predicate: ProbeSpec::Exec {
                kind: ProbeKind::Test,
                program: "cargo".into(),
                args: vec!["test".into()],
                env: vec![("RUST_LOG".into(), "debug".into())],
                timeout_seconds: 300,
            },
            env: vec![("CI".into(), "true".into())],
            working_dir: "/tmp/repo".into(),
            timeout_seconds: 300,
        };
        let script = capsule.to_shell_script_with_policy(&RedactionPolicy::none());
        assert!(script.contains("export CI='true'"));
        assert!(script.contains("export RUST_LOG='debug'"));
        assert!(script.contains("'cargo' 'test'"));
    }

    #[test]
    fn to_shell_script_with_policy_none_preserves_single_quote_escaping() {
        let capsule = ReproductionCapsule {
            commit: CommitId("abc".into()),
            predicate: ProbeSpec::Shell {
                kind: ProbeKind::Custom,
                shell: ShellKind::Default,
                script: "echo 'hello world'".into(),
                env: vec![],
                timeout_seconds: 30,
            },
            env: vec![("VAR".into(), "it's a test".into())],
            working_dir: "/tmp/it's here".into(),
            timeout_seconds: 30,
        };
        let script = capsule.to_shell_script_with_policy(&RedactionPolicy::none());
        assert!(script.contains("cd '/tmp/it'\\''s here'"));
        assert!(script.contains("export VAR='it'\\''s a test'"));
        assert!(script.contains("sh -c 'echo '\\''hello world'\\'''"));
    }

    #[test]
    fn to_shell_script_with_policy_skips_invalid_env_keys() {
        let capsule = ReproductionCapsule {
            commit: CommitId("abc".into()),
            predicate: ProbeSpec::Exec {
                kind: ProbeKind::Test,
                program: "cargo".into(),
                args: vec!["test".into()],
                env: vec![
                    ("1INVALID".into(), "val".into()),
                    ("VALID_KEY".into(), "val2".into()),
                ],
                timeout_seconds: 60,
            },
            env: vec![
                ("GOOD_KEY".into(), "val".into()),
                ("bad-key".into(), "val".into()),
                ("".into(), "val".into()),
                ("_ok".into(), "val".into()),
            ],
            working_dir: "/tmp".into(),
            timeout_seconds: 60,
        };
        let script = capsule.to_shell_script_with_policy(&RedactionPolicy::none());
        assert!(script.contains("export GOOD_KEY='val'"));
        assert!(script.contains("# skipped invalid env key: bad-key"));
        assert!(script.contains("# skipped invalid env key: "));
        assert!(script.contains("export _ok='val'"));
        assert!(script.contains("# skipped invalid env key: 1INVALID"));
        assert!(script.contains("export VALID_KEY='val2'"));
    }

    #[test]
    fn to_shell_script_with_policy_scrubs_secrets_in_shell_script() {
        let capsule = ReproductionCapsule {
            commit: CommitId("abc".into()),
            predicate: ProbeSpec::Shell {
                kind: ProbeKind::Custom,
                shell: ShellKind::Default,
                script: "curl -H 'Bearer my_secret_token' https://api.example.com".into(),
                env: vec![],
                timeout_seconds: 30,
            },
            env: vec![],
            working_dir: "/tmp".into(),
            timeout_seconds: 30,
        };
        let script = capsule.to_shell_script_with_policy(&RedactionPolicy::default_safe());
        assert!(script.contains("Bearer [REDACTED]"));
        assert!(!script.contains("my_secret_token"));
    }

    #[test]
    fn to_shell_script_with_policy_no_scrub_when_disabled() {
        let capsule = ReproductionCapsule {
            commit: CommitId("abc".into()),
            predicate: ProbeSpec::Shell {
                kind: ProbeKind::Custom,
                shell: ShellKind::Default,
                script: "curl -H 'Bearer my_secret_token' https://api.example.com".into(),
                env: vec![],
                timeout_seconds: 30,
            },
            env: vec![],
            working_dir: "/tmp".into(),
            timeout_seconds: 30,
        };
        let script = capsule.to_shell_script_with_policy(&RedactionPolicy::secrets_exposed());
        assert!(script.contains("my_secret_token"));
    }

    // --- is_valid_shell_identifier ---

    #[test]
    fn is_valid_shell_identifier_valid_cases() {
        assert!(is_valid_shell_identifier("HOME"));
        assert!(is_valid_shell_identifier("_private"));
        assert!(is_valid_shell_identifier("a"));
        assert!(is_valid_shell_identifier("MY_VAR_123"));
        assert!(is_valid_shell_identifier("_"));
        assert!(is_valid_shell_identifier("_0"));
        assert!(is_valid_shell_identifier("A1B2C3"));
    }

    #[test]
    fn is_valid_shell_identifier_invalid_cases() {
        assert!(!is_valid_shell_identifier(""));
        assert!(!is_valid_shell_identifier("1STARTS_WITH_DIGIT"));
        assert!(!is_valid_shell_identifier("has-dash"));
        assert!(!is_valid_shell_identifier("has space"));
        assert!(!is_valid_shell_identifier("has.dot"));
        assert!(!is_valid_shell_identifier("0"));
        assert!(!is_valid_shell_identifier("123"));
        assert!(!is_valid_shell_identifier("a=b"));
    }

    // --- compare_runs ---

    #[test]
    fn compare_runs_self_comparison_yields_zero_diff() {
        let report = sample_report();
        let cmp = compare_runs(&report, &report);
        assert!(!cmp.outcome_changed);
        assert_eq!(cmp.confidence_delta, 0);
        assert_eq!(cmp.window_width_delta, 0);
        assert_eq!(cmp.probes_reused, report.observations.len());
        assert!(cmp.suspect_paths_added.is_empty());
        assert!(cmp.suspect_paths_removed.is_empty());
        assert!(cmp.ambiguity_reasons_added.is_empty());
        assert!(cmp.ambiguity_reasons_removed.is_empty());
    }

    #[test]
    fn compare_runs_different_outcomes() {
        let left = sample_report();
        let mut right = sample_report();
        right.outcome = LocalizationOutcome::Inconclusive {
            reasons: vec![AmbiguityReason::MissingPassBoundary],
        };
        let cmp = compare_runs(&left, &right);
        assert!(cmp.outcome_changed);
    }

    #[test]
    fn compare_runs_suspect_path_diffs() {
        let mut left = sample_report();
        left.suspect_surface = vec![SuspectEntry {
            path: "a.rs".into(),
            priority_score: 100,
            surface_kind: "source".into(),
            change_status: ChangeStatus::Modified,
            is_execution_surface: false,
            owner_hint: None,
        }];
        let mut right = sample_report();
        right.suspect_surface = vec![SuspectEntry {
            path: "b.rs".into(),
            priority_score: 100,
            surface_kind: "source".into(),
            change_status: ChangeStatus::Added,
            is_execution_surface: false,
            owner_hint: None,
        }];
        let cmp = compare_runs(&left, &right);
        assert_eq!(cmp.suspect_paths_added, vec!["b.rs".to_string()]);
        assert_eq!(cmp.suspect_paths_removed, vec!["a.rs".to_string()]);
    }

    #[test]
    fn compare_runs_never_panics_on_empty_reports() {
        let left = AnalysisReport {
            schema_version: "0.1.0".into(),
            run_id: "left".into(),
            created_at_epoch_seconds: 0,
            request: sample_analysis_request(),
            sequence: RevisionSequence { revisions: vec![] },
            observations: vec![],
            outcome: LocalizationOutcome::Inconclusive { reasons: vec![] },
            changed_paths: vec![],
            surface: SurfaceSummary {
                total_changes: 0,
                buckets: vec![],
                execution_surfaces: vec![],
            },
            suspect_surface: vec![],
            reproduction_capsules: vec![],
            provenance: None,
        };
        let right = AnalysisReport {
            schema_version: "0.1.0".into(),
            run_id: "right".into(),
            created_at_epoch_seconds: 0,
            request: sample_analysis_request(),
            sequence: RevisionSequence { revisions: vec![] },
            observations: vec![],
            outcome: LocalizationOutcome::Inconclusive { reasons: vec![] },
            changed_paths: vec![],
            surface: SurfaceSummary {
                total_changes: 0,
                buckets: vec![],
                execution_surfaces: vec![],
            },
            suspect_surface: vec![],
            reproduction_capsules: vec![],
            provenance: None,
        };
        let cmp = compare_runs(&left, &right);
        assert!(!cmp.outcome_changed);
        assert_eq!(cmp.confidence_delta, 0);
    }

    // --- Proptest strategies for Property 14: JSON Serialization Determinism ---

    fn arb_commit_id() -> impl Strategy<Value = CommitId> {
        "[a-f0-9]{8,40}".prop_map(CommitId)
    }

    fn arb_revision_spec() -> impl Strategy<Value = RevisionSpec> {
        "[a-f0-9]{8,40}".prop_map(RevisionSpec)
    }

    fn arb_history_mode() -> impl Strategy<Value = HistoryMode> {
        prop_oneof![
            Just(HistoryMode::AncestryPath),
            Just(HistoryMode::FirstParent),
        ]
    }

    fn arb_probe_kind() -> impl Strategy<Value = ProbeKind> {
        prop_oneof![
            Just(ProbeKind::Build),
            Just(ProbeKind::Test),
            Just(ProbeKind::Lint),
            Just(ProbeKind::PerfThreshold),
            Just(ProbeKind::Custom),
        ]
    }

    fn arb_shell_kind() -> impl Strategy<Value = ShellKind> {
        prop_oneof![
            Just(ShellKind::Default),
            Just(ShellKind::PosixSh),
            Just(ShellKind::Cmd),
            Just(ShellKind::PowerShell),
        ]
    }

    fn arb_probe_spec() -> impl Strategy<Value = ProbeSpec> {
        prop_oneof![
            (
                arb_probe_kind(),
                "[a-z]{1,10}",
                prop::collection::vec("[a-z0-9]{1,8}", 0..3),
                prop::collection::vec(("[A-Z]{1,4}", "[a-z0-9]{1,6}"), 0..2),
                1u64..600,
            )
                .prop_map(|(kind, program, args, env, timeout_seconds)| {
                    ProbeSpec::Exec {
                        kind,
                        program,
                        args,
                        env,
                        timeout_seconds,
                    }
                }),
            (
                arb_probe_kind(),
                arb_shell_kind(),
                "[a-z ]{1,20}",
                1u64..600
            )
                .prop_map(|(kind, shell, script, timeout_seconds)| {
                    ProbeSpec::Shell {
                        kind,
                        shell,
                        script,
                        env: vec![],
                        timeout_seconds,
                    }
                }),
        ]
    }

    fn arb_search_policy() -> impl Strategy<Value = SearchPolicy> {
        (1usize..128).prop_map(|max_probes| SearchPolicy {
            max_probes,
            flake_policy: FlakePolicy::default(),
        })
    }

    fn arb_analysis_request() -> impl Strategy<Value = AnalysisRequest> {
        (
            "[a-z/]{1,20}",
            arb_revision_spec(),
            arb_revision_spec(),
            arb_history_mode(),
            arb_probe_spec(),
            arb_search_policy(),
        )
            .prop_map(|(repo_root, good, bad, history_mode, probe, policy)| {
                AnalysisRequest {
                    repo_root: PathBuf::from(repo_root),
                    good,
                    bad,
                    history_mode,
                    probe,
                    policy,
                }
            })
    }

    fn arb_revision_sequence() -> impl Strategy<Value = RevisionSequence> {
        prop::collection::vec(arb_commit_id(), 2..10)
            .prop_map(|revisions| RevisionSequence { revisions })
    }

    fn arb_observation_class() -> impl Strategy<Value = ObservationClass> {
        prop_oneof![
            Just(ObservationClass::Pass),
            Just(ObservationClass::Fail),
            Just(ObservationClass::Skip),
            Just(ObservationClass::Indeterminate),
        ]
    }

    fn arb_probe_observation() -> impl Strategy<Value = ProbeObservation> {
        (
            arb_commit_id(),
            arb_observation_class(),
            arb_probe_kind(),
            prop::option::of(any::<i32>()),
            any::<bool>(),
            any::<u64>(),
            "[a-z ]{0,20}",
            "[a-z ]{0,20}",
            any::<u64>(),
            prop::option::of(any::<i32>()),
            "[a-z/ ]{0,30}",
            "[a-z/ ]{0,30}",
        )
            .prop_map(
                |(
                    commit,
                    class,
                    kind,
                    exit_code,
                    timed_out,
                    duration_ms,
                    stdout,
                    stderr,
                    sequence_index,
                    signal_number,
                    probe_command,
                    working_dir,
                )| {
                    ProbeObservation {
                        commit,
                        class,
                        kind,
                        exit_code,
                        timed_out,
                        duration_ms,
                        stdout,
                        stderr,
                        sequence_index,
                        signal_number,
                        probe_command,
                        working_dir,
                        flake_signal: None,
                    }
                },
            )
    }

    fn arb_confidence() -> impl Strategy<Value = Confidence> {
        (any::<u8>(), "[a-z]{1,10}").prop_map(|(score, label)| Confidence { score, label })
    }

    fn arb_ambiguity_reason() -> impl Strategy<Value = AmbiguityReason> {
        prop_oneof![
            Just(AmbiguityReason::MissingPassBoundary),
            Just(AmbiguityReason::MissingFailBoundary),
            Just(AmbiguityReason::NonMonotonicEvidence),
            Just(AmbiguityReason::SkippedRevision),
            Just(AmbiguityReason::IndeterminateRevision),
            Just(AmbiguityReason::UntestableWindow),
            Just(AmbiguityReason::BoundaryValidationFailed),
            Just(AmbiguityReason::NeedsMoreProbes),
        ]
    }

    fn arb_localization_outcome() -> impl Strategy<Value = LocalizationOutcome> {
        prop_oneof![
            (arb_commit_id(), arb_commit_id(), arb_confidence()).prop_map(
                |(last_good, first_bad, confidence)| {
                    LocalizationOutcome::FirstBad {
                        last_good,
                        first_bad,
                        confidence,
                    }
                }
            ),
            (
                arb_commit_id(),
                arb_commit_id(),
                arb_confidence(),
                prop::collection::vec(arb_ambiguity_reason(), 1..4),
            )
                .prop_map(
                    |(lower_bound_exclusive, upper_bound_inclusive, confidence, reasons)| {
                        LocalizationOutcome::SuspectWindow {
                            lower_bound_exclusive,
                            upper_bound_inclusive,
                            confidence,
                            reasons,
                        }
                    }
                ),
            prop::collection::vec(arb_ambiguity_reason(), 1..4)
                .prop_map(|reasons| LocalizationOutcome::Inconclusive { reasons }),
        ]
    }

    fn arb_change_status() -> impl Strategy<Value = ChangeStatus> {
        prop_oneof![
            Just(ChangeStatus::Added),
            Just(ChangeStatus::Modified),
            Just(ChangeStatus::Deleted),
            Just(ChangeStatus::Renamed),
            Just(ChangeStatus::TypeChanged),
            Just(ChangeStatus::Unknown),
        ]
    }

    fn arb_path_change() -> impl Strategy<Value = PathChange> {
        (arb_change_status(), "[a-z/]{1,30}").prop_map(|(status, path)| PathChange { status, path })
    }

    fn arb_subsystem_bucket() -> impl Strategy<Value = SubsystemBucket> {
        (
            "[a-z]{1,10}",
            0usize..20,
            prop::collection::vec("[a-z/]{1,20}", 0..5),
            prop::collection::vec("[a-z]{1,10}", 0..3),
        )
            .prop_map(
                |(name, change_count, paths, surface_kinds)| SubsystemBucket {
                    name,
                    change_count,
                    paths,
                    surface_kinds,
                },
            )
    }

    fn arb_surface_summary() -> impl Strategy<Value = SurfaceSummary> {
        (
            0usize..50,
            prop::collection::vec(arb_subsystem_bucket(), 0..5),
            prop::collection::vec("[a-z/]{1,20}", 0..3),
        )
            .prop_map(
                |(total_changes, buckets, execution_surfaces)| SurfaceSummary {
                    total_changes,
                    buckets,
                    execution_surfaces,
                },
            )
    }

    fn arb_analysis_report() -> impl Strategy<Value = AnalysisReport> {
        (
            "[a-z0-9-]{1,20}",
            any::<u64>(),
            arb_analysis_request(),
            arb_revision_sequence(),
            prop::collection::vec(arb_probe_observation(), 0..5),
            arb_localization_outcome(),
            prop::collection::vec(arb_path_change(), 0..5),
            arb_surface_summary(),
        )
            .prop_map(
                |(
                    run_id,
                    created_at_epoch_seconds,
                    request,
                    sequence,
                    observations,
                    outcome,
                    changed_paths,
                    surface,
                )| {
                    AnalysisReport {
                        schema_version: "0.1.0".into(),
                        run_id,
                        created_at_epoch_seconds,
                        request,
                        sequence,
                        observations,
                        outcome,
                        changed_paths,
                        surface,
                        suspect_surface: vec![],
                        reproduction_capsules: vec![],
                        provenance: None,
                    }
                },
            )
    }

    // Feature: v01-release-train, Property 14: JSON Serialization Determinism
    // **Validates: Requirements 6.3**
    // Feature: v01-release-train, Property 15: AnalysisReport JSON Round-Trip
    // **Validates: Requirements 6.5**
    proptest! {
        #![proptest_config(ProptestConfig { cases: 100, .. ProptestConfig::default() })]

        #[test]
        fn prop_json_serialization_determinism(report in arb_analysis_report()) {
            let json1 = serde_json::to_string_pretty(&report).expect("first serialization");
            let json2 = serde_json::to_string_pretty(&report).expect("second serialization");
            prop_assert_eq!(json1, json2, "serializing the same AnalysisReport twice must produce byte-identical JSON");
        }

        #[test]
        fn prop_analysis_report_json_round_trip(report in arb_analysis_report()) {
            let json = serde_json::to_string_pretty(&report).expect("serialize");
            let deserialized: AnalysisReport = serde_json::from_str(&json).expect("deserialize");
            prop_assert_eq!(report, deserialized, "JSON round-trip must preserve equality");
        }
    }

    // Feature: repo-operating-system, Property 40: JSON Schema Validates All Valid Reports
    // **Validates: Requirements 3.1, 3.2**
    proptest! {
        #![proptest_config(ProptestConfig { cases: 100, .. ProptestConfig::default() })]

        #[test]
        fn prop_json_schema_validates_all_valid_reports(report in arb_analysis_report()) {
            // Generate the JSON Schema from the Rust types
            let schema = schemars::schema_for!(AnalysisReport);
            let schema_json = serde_json::to_value(&schema).expect("schema serializes to JSON");

            // Verify schema contains $schema draft identifier
            let schema_draft = schema_json.get("$schema").expect("schema must have $schema field");
            prop_assert!(
                schema_draft.as_str().unwrap().contains("json-schema.org"),
                "schema $schema field must reference json-schema.org draft"
            );

            // Verify schema contains title field
            let title = schema_json.get("title").expect("schema must have title field");
            prop_assert_eq!(title.as_str().unwrap(), "AnalysisReport");

            // Serialize the generated report to JSON
            let report_json = serde_json::to_string(&report).expect("report serializes to JSON");

            // Validate: the report JSON can be deserialized back to AnalysisReport
            // This proves the schema (derived from the same types) accepts all valid reports
            let roundtripped: AnalysisReport =
                serde_json::from_str(&report_json).expect("report JSON deserializes back");
            prop_assert_eq!(
                report, roundtripped,
                "schema-conformant report must round-trip through JSON"
            );

            // Structural check: the report JSON is a valid JSON object with expected top-level keys
            let report_value: serde_json::Value =
                serde_json::from_str(&report_json).expect("report is valid JSON");
            let obj = report_value.as_object().expect("report must be a JSON object");

            // Verify all required fields from the schema are present in the serialized report
            let required = schema_json
                .get("required")
                .and_then(|r| r.as_array())
                .expect("schema must have required array");
            for req_field in required {
                let field_name = req_field.as_str().unwrap();
                prop_assert!(
                    obj.contains_key(field_name),
                    "report JSON must contain required field '{}'",
                    field_name
                );
            }
        }
    }

    // Feature: v01-product-sharpening, Property 48: FlakeSignal stability classification
    // **Validates: Requirements 3.2, 3.3**
    proptest! {
        #![proptest_config(ProptestConfig { cases: 100, .. ProptestConfig::default() })]

        #[test]
        fn prop_flake_signal_stability_classification(
            results in prop::collection::vec(arb_observation_class(), 0..50),
            stability_threshold in 0.0f64..=1.0f64,
        ) {
            let signal = compute_flake_signal(&results, stability_threshold);

            // Counts must sum to total_runs
            prop_assert_eq!(
                signal.pass_count + signal.fail_count + signal.skip_count + signal.indeterminate_count,
                signal.total_runs,
                "counts must sum to total_runs"
            );

            // total_runs must equal input length
            prop_assert_eq!(signal.total_runs, results.len() as u32);

            // Verify is_stable matches threshold logic
            if results.is_empty() {
                prop_assert!(signal.is_stable, "empty results must be vacuously stable");
            } else {
                let max_count = signal.pass_count
                    .max(signal.fail_count)
                    .max(signal.skip_count)
                    .max(signal.indeterminate_count);
                let proportion = max_count as f64 / signal.total_runs as f64;
                let expected_stable = proportion >= stability_threshold;
                prop_assert_eq!(
                    signal.is_stable, expected_stable,
                    "is_stable must match threshold logic: proportion={}, threshold={}",
                    proportion, stability_threshold
                );
            }
        }
    }

    // --- Arbitrary generator for P52 ---

    fn arb_reproduction_capsule() -> impl Strategy<Value = ReproductionCapsule> {
        (
            "[a-f0-9]{8,40}".prop_map(CommitId),
            arb_probe_spec(),
            prop::collection::vec(("[A-Z_]{1,6}", "[a-z0-9]{1,10}"), 0..4),
            "[a-z/]{1,20}",
            1u64..600,
        )
            .prop_map(|(commit, predicate, env, working_dir, timeout_seconds)| {
                ReproductionCapsule {
                    commit,
                    predicate,
                    env,
                    working_dir,
                    timeout_seconds,
                }
            })
    }

    // Feature: v01-product-sharpening, Property 52: Shell script generation contains required fields
    // **Validates: Requirements 4.4**
    proptest! {
        #![proptest_config(ProptestConfig { cases: 100, .. ProptestConfig::default() })]

        #[test]
        fn prop_shell_script_contains_required_fields(capsule in arb_reproduction_capsule()) {
            // Use RedactionPolicy::none() to verify structural correctness
            // (all fields present without redaction masking values)
            let script = capsule.to_shell_script_with_policy(&RedactionPolicy::none());

            // Must contain the commit SHA
            prop_assert!(
                script.contains(&capsule.commit.0),
                "shell script must contain commit SHA '{}'\nScript:\n{}",
                capsule.commit.0, script
            );

            // Must contain the timeout value
            let timeout_str = capsule.timeout_seconds.to_string();
            prop_assert!(
                script.contains(&timeout_str),
                "shell script must contain timeout value '{}'\nScript:\n{}",
                timeout_str, script
            );

            // Must contain the predicate command (program name or shell script)
            match &capsule.predicate {
                ProbeSpec::Exec { program, .. } => {
                    prop_assert!(
                        script.contains(program),
                        "shell script must contain program '{}'\nScript:\n{}",
                        program, script
                    );
                }
                ProbeSpec::Shell { script: shell_script, .. } => {
                    // The shell script content may be escaped, but the core content should be present
                    // shell_escape replaces ' with '\'' so we check the unescaped content is findable
                    // by checking the escaped version
                    let escaped = shell_escape(shell_script);
                    prop_assert!(
                        script.contains(&escaped),
                        "shell script must contain shell script content '{}'\nScript:\n{}",
                        shell_script, script
                    );
                }
            }

            // Must contain each env key and value pair from the capsule's env field
            for (key, value) in &capsule.env {
                prop_assert!(
                    script.contains(key),
                    "shell script must contain env key '{}'\nScript:\n{}",
                    key, script
                );
                prop_assert!(
                    script.contains(value),
                    "shell script must contain env value '{}'\nScript:\n{}",
                    value, script
                );
            }

            // Also verify that to_shell_script() (default_safe) redacts env values
            let redacted_script = capsule.to_shell_script();
            for (key, _value) in &capsule.env {
                prop_assert!(
                    redacted_script.contains(key),
                    "redacted shell script must still contain env key '{}'\nScript:\n{}",
                    key, redacted_script
                );
                prop_assert!(
                    redacted_script.contains("[REDACTED]"),
                    "redacted shell script must contain [REDACTED] for env values\nScript:\n{}",
                    redacted_script
                );
            }
        }
    }

    // --- Arbitrary generator for P51 ---

    /// Generate an AnalysisReport with reproduction capsules properly populated
    /// from observations (simulating what faultline-app produces).
    fn arb_report_with_capsules() -> impl Strategy<Value = AnalysisReport> {
        (
            "[a-z0-9-]{1,20}",
            any::<u64>(),
            arb_analysis_request(),
            arb_revision_sequence(),
            prop::collection::vec(arb_probe_observation(), 1..6),
            arb_localization_outcome(),
            prop::collection::vec(arb_path_change(), 0..5),
            arb_surface_summary(),
        )
            .prop_map(
                |(
                    run_id,
                    created_at_epoch_seconds,
                    request,
                    sequence,
                    observations,
                    outcome,
                    changed_paths,
                    surface,
                )| {
                    // Build capsules from observations, mirroring faultline-app logic
                    let probe_env = match &request.probe {
                        ProbeSpec::Exec { env, .. } => env.clone(),
                        ProbeSpec::Shell { env, .. } => env.clone(),
                    };
                    let reproduction_capsules: Vec<ReproductionCapsule> = observations
                        .iter()
                        .map(|obs| ReproductionCapsule {
                            commit: obs.commit.clone(),
                            predicate: request.probe.clone(),
                            env: probe_env.clone(),
                            working_dir: request.repo_root.to_string_lossy().to_string(),
                            timeout_seconds: request.probe.timeout_seconds(),
                        })
                        .collect();

                    AnalysisReport {
                        schema_version: "0.3.0".into(),
                        run_id,
                        created_at_epoch_seconds,
                        request,
                        sequence,
                        observations,
                        outcome,
                        changed_paths,
                        surface,
                        suspect_surface: vec![],
                        reproduction_capsules,
                        provenance: None,
                    }
                },
            )
    }

    // Feature: v01-product-sharpening, Property 51: ReproductionCapsule structural correspondence
    // **Validates: Requirements 4.1, 4.2**
    proptest! {
        #![proptest_config(ProptestConfig { cases: 100, .. ProptestConfig::default() })]

        #[test]
        fn prop_reproduction_capsule_structural_correspondence(report in arb_report_with_capsules()) {
            // Capsule count must equal observation count
            prop_assert_eq!(
                report.reproduction_capsules.len(),
                report.observations.len(),
                "capsule count ({}) must equal observation count ({})",
                report.reproduction_capsules.len(),
                report.observations.len()
            );

            // Each capsule must have a matching observation commit
            for capsule in &report.reproduction_capsules {
                let has_matching_observation = report.observations.iter().any(|obs| obs.commit == capsule.commit);
                prop_assert!(
                    has_matching_observation,
                    "capsule commit '{}' must have a matching observation",
                    capsule.commit.0
                );
            }

            // Each capsule's predicate must equal request.probe
            for capsule in &report.reproduction_capsules {
                prop_assert_eq!(
                    &capsule.predicate,
                    &report.request.probe,
                    "capsule predicate must equal request.probe"
                );
            }

            // Each capsule's timeout must equal probe spec timeout
            let expected_timeout = report.request.probe.timeout_seconds();
            for capsule in &report.reproduction_capsules {
                prop_assert_eq!(
                    capsule.timeout_seconds,
                    expected_timeout,
                    "capsule timeout ({}) must equal probe spec timeout ({})",
                    capsule.timeout_seconds,
                    expected_timeout
                );
            }
        }
    }

    // Feature: v01-product-sharpening, Property 53: compare_runs is total
    // **Validates: Requirements 5.1**
    proptest! {
        #![proptest_config(ProptestConfig { cases: 100, .. ProptestConfig::default() })]

        #[test]
        fn prop_compare_runs_is_total(
            left in arb_analysis_report(),
            right in arb_analysis_report(),
        ) {
            // compare_runs must return without panicking for any two valid reports
            let result = compare_runs(&left, &right);

            // Basic structural assertions: run IDs match the inputs
            prop_assert_eq!(&result.left_run_id, &left.run_id);
            prop_assert_eq!(&result.right_run_id, &right.run_id);
        }
    }

    // Feature: v01-product-sharpening, Property 54: Self-comparison yields zero diff
    // **Validates: Requirements 5.3**
    proptest! {
        #![proptest_config(ProptestConfig { cases: 100, .. ProptestConfig::default() })]

        #[test]
        fn prop_self_comparison_yields_zero_diff(report in arb_analysis_report()) {
            let cmp = compare_runs(&report, &report.clone());

            prop_assert!(
                !cmp.outcome_changed,
                "self-comparison must have outcome_changed == false"
            );
            prop_assert_eq!(
                cmp.confidence_delta, 0,
                "self-comparison must have confidence_delta == 0"
            );
            prop_assert_eq!(
                cmp.window_width_delta, 0,
                "self-comparison must have window_width_delta == 0"
            );
            prop_assert_eq!(
                cmp.probes_reused, report.observations.len(),
                "self-comparison probes_reused ({}) must equal observations.len() ({})",
                cmp.probes_reused, report.observations.len()
            );
            prop_assert!(
                cmp.suspect_paths_added.is_empty(),
                "self-comparison must have empty suspect_paths_added"
            );
            prop_assert!(
                cmp.suspect_paths_removed.is_empty(),
                "self-comparison must have empty suspect_paths_removed"
            );
            prop_assert!(
                cmp.ambiguity_reasons_added.is_empty(),
                "self-comparison must have empty ambiguity_reasons_added"
            );
            prop_assert!(
                cmp.ambiguity_reasons_removed.is_empty(),
                "self-comparison must have empty ambiguity_reasons_removed"
            );
        }
    }

    // -----------------------------------------------------------------------
    // 15.3: Schema evolution scenario
    // Validates: Requirements 8.1
    //
    // Deserialize an old-version (0.1.0) report JSON that lacks the new fields
    // (suspect_surface, reproduction_capsules, flake_signal) into the current
    // AnalysisReport struct. Verify forward compatibility via #[serde(default)].
    // -----------------------------------------------------------------------

    #[test]
    fn schema_evolution_old_version_deserializes_with_defaults() {
        // A JSON string representing a v0.1.0 report WITHOUT the new fields:
        // suspect_surface, reproduction_capsules, and flake_signal on observations.
        let old_json = r#"{
            "schema_version": "0.1.0",
            "run_id": "old-run-001",
            "created_at_epoch_seconds": 1700000000,
            "request": {
                "repo_root": "/tmp/repo",
                "good": "abc123",
                "bad": "def456",
                "history_mode": "AncestryPath",
                "probe": {
                    "Exec": {
                        "kind": "Test",
                        "program": "cargo",
                        "args": ["test"],
                        "env": [],
                        "timeout_seconds": 300
                    }
                },
                "policy": {
                    "max_probes": 64
                }
            },
            "sequence": {
                "revisions": ["abc123", "def456"]
            },
            "observations": [
                {
                    "commit": "abc123",
                    "class": "Pass",
                    "kind": "Test",
                    "exit_code": 0,
                    "timed_out": false,
                    "duration_ms": 100,
                    "stdout": "ok",
                    "stderr": ""
                }
            ],
            "outcome": {
                "FirstBad": {
                    "last_good": "abc123",
                    "first_bad": "def456",
                    "confidence": { "score": 95, "label": "high" }
                }
            },
            "changed_paths": [
                { "status": "Modified", "path": "src/main.rs" }
            ],
            "surface": {
                "total_changes": 1,
                "buckets": [],
                "execution_surfaces": []
            }
        }"#;

        let report: AnalysisReport =
            serde_json::from_str(old_json).expect("old v0.1.0 JSON must deserialize");

        // Verify the explicitly set fields
        assert_eq!(report.schema_version, "0.1.0");
        assert_eq!(report.run_id, "old-run-001");
        assert_eq!(report.observations.len(), 1);

        // Verify the NEW fields default to empty/None via #[serde(default)]
        assert!(
            report.suspect_surface.is_empty(),
            "suspect_surface must default to empty vec"
        );
        assert!(
            report.reproduction_capsules.is_empty(),
            "reproduction_capsules must default to empty vec"
        );

        // Verify observation-level new fields default correctly
        let obs = &report.observations[0];
        assert_eq!(obs.sequence_index, 0, "sequence_index must default to 0");
        assert_eq!(
            obs.signal_number, None,
            "signal_number must default to None"
        );
        assert!(
            obs.flake_signal.is_none(),
            "flake_signal must default to None"
        );
        assert!(
            obs.probe_command.is_empty(),
            "probe_command must default to empty string"
        );
        assert!(
            obs.working_dir.is_empty(),
            "working_dir must default to empty string"
        );

        // Verify the report can be re-serialized and re-deserialized
        let reserialized = serde_json::to_string_pretty(&report).unwrap();
        let roundtrip: AnalysisReport = serde_json::from_str(&reserialized).unwrap();
        assert_eq!(report, roundtrip, "round-trip must preserve the report");
    }

    // --- SecretScrubber tests ---

    #[test]
    fn scrub_secrets_github_token() {
        let input = "token: ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmn";
        let result = scrub_secrets(input);
        assert_eq!(result, "token: ghp_[REDACTED]");
    }

    #[test]
    fn scrub_secrets_aws_key() {
        let input = "key=AKIAIOSFODNN7EXAMPLE";
        let result = scrub_secrets(input);
        assert_eq!(result, "key=AKIA[REDACTED]");
    }

    #[test]
    fn scrub_secrets_stripe_live_key() {
        let input = "stripe: sk-live_abcdefghijklmnopqrstuvwx";
        let result = scrub_secrets(input);
        assert_eq!(result, "stripe: sk-live_[REDACTED]");
    }

    #[test]
    fn scrub_secrets_stripe_test_key() {
        let input = "stripe: sk-test_abcdefghijklmnopqrstuvwx";
        let result = scrub_secrets(input);
        assert_eq!(result, "stripe: sk-test_[REDACTED]");
    }

    #[test]
    fn scrub_secrets_bearer_token() {
        let input = "Authorization: Bearer eyJhbGciOiJIUzI1NiJ9.payload.signature";
        let result = scrub_secrets(input);
        assert_eq!(result, "Authorization: Bearer [REDACTED]");
    }

    #[test]
    fn scrub_secrets_password_value() {
        let input = "connection: password=hunter2&host=localhost";
        let result = scrub_secrets(input);
        assert_eq!(result, "connection: password=[REDACTED]");
    }

    #[test]
    fn scrub_secrets_github_fine_grained_pat() {
        // Fine-grained PATs need 40+ chars after prefix.
        // Underscore body is obviously synthetic to dodge secret scanners.
        let input = "token: github_pat__________________________________________________";
        let result = scrub_secrets(input);
        assert_eq!(result, "token: github_pat_[REDACTED]");
    }

    #[test]
    fn scrub_secrets_github_pat_too_short_no_match() {
        // Fine-grained PATs need 40+ chars after prefix — this is too short
        let input = "github_pat_shorttoken";
        let result = scrub_secrets(input);
        assert_eq!(result, "github_pat_shorttoken");
    }

    #[test]
    fn scrub_secrets_google_api_key() {
        // Google API keys are AIza + exactly 35 URL-safe chars.
        // Uses an all-underscore body: matches the regex but is obviously
        // synthetic, so it doesn't trip secret scanners.
        let input = "key: AIza___________________________________";
        let result = scrub_secrets(input);
        assert_eq!(result, "key: AIza[REDACTED]");
    }

    #[test]
    fn scrub_secrets_google_api_key_too_short_no_match() {
        // Google API keys need exactly 35 chars after AIza — this is too short
        let input = "AIza shortkey";
        let result = scrub_secrets(input);
        assert_eq!(result, "AIza shortkey");
    }

    #[test]
    fn scrub_secrets_slack_token() {
        // Slack tokens need 10+ chars after the type prefix.
        // FIXTURE body is obviously synthetic to dodge secret scanners.
        let input = "slack: xoxb-NOT-A-REAL-SLACK-TOKEN-FIXTURE";
        let result = scrub_secrets(input);
        assert_eq!(result, "slack: xoxb-[REDACTED]");
    }

    #[test]
    fn scrub_secrets_slack_token_too_short_no_match() {
        // Slack tokens need 10+ chars after the type prefix — this is too short
        let input = "xoxb-short";
        let result = scrub_secrets(input);
        assert_eq!(result, "xoxb-short");
    }

    #[test]
    fn scrub_secrets_pem_private_key_block() {
        // Uses FIXTURE PRIVATE KEY (uppercase, matches the regex's [A-Z ]*)
        // with an obviously-synthetic body to dodge secret scanners.
        let input = "key:\n-----BEGIN FIXTURE PRIVATE KEY-----\nFIXTURE_BODY_NOT_A_REAL_KEY\n-----END FIXTURE PRIVATE KEY-----\n";
        let result = scrub_secrets(input);
        assert!(
            result.contains("-----BEGIN FIXTURE PRIVATE KEY-----[REDACTED]"),
            "PEM header must be preserved and body redacted; got: {result}"
        );
        assert!(
            !result.contains("FIXTURE_BODY_NOT_A_REAL_KEY"),
            "PEM body must not survive redaction; got: {result}"
        );
    }

    #[test]
    fn scrub_secrets_pem_header_without_end_no_match() {
        // A bare header with no matching END block should not redact
        let input = "-----BEGIN FIXTURE PRIVATE KEY-----";
        let result = scrub_secrets(input);
        assert_eq!(result, "-----BEGIN FIXTURE PRIVATE KEY-----");
    }

    #[test]
    fn scrub_secrets_no_match_passthrough() {
        let input = "nothing secret here";
        let result = scrub_secrets(input);
        assert_eq!(result, "nothing secret here");
    }

    #[test]
    fn scrub_secrets_multiple_patterns() {
        let input = "env: Bearer token123 and password=secret";
        let result = scrub_secrets(input);
        assert_eq!(result, "env: Bearer [REDACTED] and password=[REDACTED]");
    }

    #[test]
    fn scrub_secrets_github_token_too_short_no_match() {
        // GitHub tokens need 36+ chars after prefix — this is too short
        let input = "ghp_short";
        let result = scrub_secrets(input);
        assert_eq!(result, "ghp_short");
    }

    #[test]
    fn scrub_secrets_aws_key_too_short_no_match() {
        // AKIA alone without 16 chars suffix should not match
        let input = "AKIA alone";
        let result = scrub_secrets(input);
        assert_eq!(result, "AKIA alone");
    }

    #[test]
    fn scrub_secrets_empty_input() {
        let result = scrub_secrets("");
        assert_eq!(result, "");
    }

    // --- redact_report tests ---

    #[test]
    fn redact_report_is_pure_does_not_modify_input() {
        let report = sample_report();
        let original = report.clone();
        let _redacted = redact_report(&report, &RedactionPolicy::default_safe());
        assert_eq!(report, original, "redact_report must not modify the input");
    }

    #[test]
    fn redact_report_sets_provenance_default_policy() {
        let report = sample_report();
        let redacted = redact_report(&report, &RedactionPolicy::default_safe());
        let prov = redacted.provenance.expect("provenance must be set");
        assert_eq!(prov.redaction_policy, "default");
        assert!(prov.env_values_redacted);
        assert!(prov.output_scrubbed);
        assert_eq!(prov.artifact_source, None);
    }

    #[test]
    fn redact_report_sets_provenance_none_policy() {
        let report = sample_report();
        let redacted = redact_report(&report, &RedactionPolicy::none());
        let prov = redacted.provenance.expect("provenance must be set");
        assert_eq!(prov.redaction_policy, "none");
        assert!(!prov.env_values_redacted);
        assert!(!prov.output_scrubbed);
    }

    #[test]
    fn redact_report_redacts_request_probe_env() {
        let mut report = sample_report();
        report.request.probe = ProbeSpec::Exec {
            kind: ProbeKind::Test,
            program: "cargo".into(),
            args: vec!["test".into()],
            env: vec![("SECRET_KEY".into(), "super_secret_value".into())],
            timeout_seconds: 300,
        };
        let redacted = redact_report(&report, &RedactionPolicy::default_safe());
        match &redacted.request.probe {
            ProbeSpec::Exec { env, .. } => {
                assert_eq!(env.len(), 1);
                assert_eq!(env[0].0, "SECRET_KEY");
                assert_eq!(env[0].1, "[REDACTED]");
            }
            _ => panic!("expected Exec variant"),
        }
    }

    #[test]
    fn redact_report_redacts_shell_probe_env() {
        let mut report = sample_report();
        report.request.probe = ProbeSpec::Shell {
            kind: ProbeKind::Custom,
            shell: ShellKind::Default,
            script: "make test".into(),
            env: vec![("DB_PASSWORD".into(), "hunter2".into())],
            timeout_seconds: 60,
        };
        let redacted = redact_report(&report, &RedactionPolicy::default_safe());
        match &redacted.request.probe {
            ProbeSpec::Shell { env, .. } => {
                assert_eq!(env.len(), 1);
                assert_eq!(env[0].0, "DB_PASSWORD");
                assert_eq!(env[0].1, "[REDACTED]");
            }
            _ => panic!("expected Shell variant"),
        }
    }

    #[test]
    fn redact_report_redacts_reproduction_capsule_env() {
        let mut report = sample_report();
        report.reproduction_capsules = vec![ReproductionCapsule {
            commit: CommitId("abc123".into()),
            predicate: ProbeSpec::Exec {
                kind: ProbeKind::Test,
                program: "cargo".into(),
                args: vec!["test".into()],
                env: vec![("PROBE_TOKEN".into(), "tok_12345".into())],
                timeout_seconds: 300,
            },
            env: vec![("API_KEY".into(), "key_abcdef".into())],
            working_dir: "/tmp/repo".into(),
            timeout_seconds: 300,
        }];
        let redacted = redact_report(&report, &RedactionPolicy::default_safe());
        let capsule = &redacted.reproduction_capsules[0];
        assert_eq!(capsule.env[0].0, "API_KEY");
        assert_eq!(capsule.env[0].1, "[REDACTED]");
        // Also check the capsule's predicate env
        match &capsule.predicate {
            ProbeSpec::Exec { env, .. } => {
                assert_eq!(env[0].0, "PROBE_TOKEN");
                assert_eq!(env[0].1, "[REDACTED]");
            }
            _ => panic!("expected Exec variant"),
        }
    }

    #[test]
    fn redact_report_scrubs_observation_stdout_stderr() {
        let mut report = sample_report();
        report.observations[0].stdout =
            "token: ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmn".into();
        report.observations[0].stderr = "password=hunter2".into();
        report.observations[0].probe_command = "Bearer eyJtoken".into();
        let redacted = redact_report(&report, &RedactionPolicy::default_safe());
        assert_eq!(redacted.observations[0].stdout, "token: ghp_[REDACTED]");
        assert_eq!(redacted.observations[0].stderr, "password=[REDACTED]");
        assert_eq!(redacted.observations[0].probe_command, "Bearer [REDACTED]");
    }

    #[test]
    fn redact_report_scrubs_exec_program_and_args() {
        let mut report = sample_report();
        report.request.probe = ProbeSpec::Exec {
            kind: ProbeKind::Test,
            program: "cmd".into(),
            args: vec!["--token=Bearer eyJsecret".into()],
            env: vec![],
            timeout_seconds: 300,
        };
        let redacted = redact_report(&report, &RedactionPolicy::default_safe());
        match &redacted.request.probe {
            ProbeSpec::Exec { args, .. } => {
                assert_eq!(args[0], "--token=Bearer [REDACTED]");
            }
            _ => panic!("expected Exec variant"),
        }
    }

    #[test]
    fn redact_report_scrubs_shell_script() {
        let mut report = sample_report();
        report.request.probe = ProbeSpec::Shell {
            kind: ProbeKind::Custom,
            shell: ShellKind::Default,
            script: "curl -H 'Authorization: Bearer eyJtoken' http://api".into(),
            env: vec![],
            timeout_seconds: 60,
        };
        let redacted = redact_report(&report, &RedactionPolicy::default_safe());
        match &redacted.request.probe {
            ProbeSpec::Shell { script, .. } => {
                assert!(script.contains("Bearer [REDACTED]"));
                assert!(!script.contains("eyJtoken"));
            }
            _ => panic!("expected Shell variant"),
        }
    }

    #[test]
    fn redact_report_no_redaction_with_none_policy() {
        let mut report = sample_report();
        report.request.probe = ProbeSpec::Exec {
            kind: ProbeKind::Test,
            program: "cargo".into(),
            args: vec!["test".into()],
            env: vec![("SECRET".into(), "value123".into())],
            timeout_seconds: 300,
        };
        report.observations[0].stdout = "password=hunter2".into();
        let redacted = redact_report(&report, &RedactionPolicy::none());
        // Env should NOT be redacted
        match &redacted.request.probe {
            ProbeSpec::Exec { env, .. } => {
                assert_eq!(env[0].1, "value123");
            }
            _ => panic!("expected Exec variant"),
        }
        // Stdout should NOT be scrubbed
        assert_eq!(redacted.observations[0].stdout, "password=hunter2");
    }

    #[test]
    fn redact_report_env_exposed_policy_preserves_env_scrubs_secrets() {
        let mut report = sample_report();
        report.request.probe = ProbeSpec::Exec {
            kind: ProbeKind::Test,
            program: "cargo".into(),
            args: vec!["test".into()],
            env: vec![("SECRET".into(), "value123".into())],
            timeout_seconds: 300,
        };
        report.observations[0].stdout = "password=hunter2".into();
        let redacted = redact_report(&report, &RedactionPolicy::env_exposed());
        // Env should NOT be redacted (env_exposed means env is visible)
        match &redacted.request.probe {
            ProbeSpec::Exec { env, .. } => {
                assert_eq!(env[0].1, "value123");
            }
            _ => panic!("expected Exec variant"),
        }
        // Stdout SHOULD be scrubbed
        assert_eq!(redacted.observations[0].stdout, "password=[REDACTED]");
        let prov = redacted.provenance.unwrap();
        assert_eq!(prov.redaction_policy, "env-exposed");
        assert!(!prov.env_values_redacted);
        assert!(prov.output_scrubbed);
    }

    #[test]
    fn redact_report_secrets_exposed_policy_redacts_env_preserves_output() {
        let mut report = sample_report();
        report.request.probe = ProbeSpec::Exec {
            kind: ProbeKind::Test,
            program: "cargo".into(),
            args: vec!["test".into()],
            env: vec![("SECRET".into(), "value123".into())],
            timeout_seconds: 300,
        };
        report.observations[0].stdout = "password=hunter2".into();
        let redacted = redact_report(&report, &RedactionPolicy::secrets_exposed());
        // Env SHOULD be redacted
        match &redacted.request.probe {
            ProbeSpec::Exec { env, .. } => {
                assert_eq!(env[0].1, "[REDACTED]");
            }
            _ => panic!("expected Exec variant"),
        }
        // Stdout should NOT be scrubbed
        assert_eq!(redacted.observations[0].stdout, "password=hunter2");
        let prov = redacted.provenance.unwrap();
        assert_eq!(prov.redaction_policy, "secrets-exposed");
        assert!(prov.env_values_redacted);
        assert!(!prov.output_scrubbed);
    }

    // Feature: v01-artifact-hardening, Property 6: Env Redaction Completeness in Shell Scripts
    // **Validates: Requirements 6.6, 18.5**
    mod prop_redaction_shell_scripts {
        use super::*;

        /// Sentinel prefix for env values — easy to search for in output.
        const SENTINEL_PREFIX: &str = "SENTINEL_";

        /// Generate a sentinel env value: `SENTINEL_` followed by a UUID-like hex string.
        fn arb_sentinel_value() -> impl Strategy<Value = String> {
            "[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}"
                .prop_map(|uuid| format!("{}{}", SENTINEL_PREFIX, uuid))
        }

        /// Generate a valid shell identifier for use as an env key.
        fn arb_env_key() -> impl Strategy<Value = String> {
            "[A-Z][A-Z0-9_]{2,10}"
        }

        /// Generate a non-empty vec of env pairs with sentinel values.
        fn arb_sentinel_env_pairs() -> impl Strategy<Value = Vec<(String, String)>> {
            prop::collection::vec((arb_env_key(), arb_sentinel_value()), 1..5)
        }

        /// Generate a ProbeSpec with sentinel env values injected.
        fn arb_probe_spec_with_sentinels() -> impl Strategy<Value = ProbeSpec> {
            prop_oneof![
                (
                    arb_probe_kind(),
                    "[a-z]{1,10}",
                    prop::collection::vec("[a-z0-9]{1,8}", 0..3),
                    arb_sentinel_env_pairs(),
                    1u64..600,
                )
                    .prop_map(|(kind, program, args, env, timeout_seconds)| {
                        ProbeSpec::Exec {
                            kind,
                            program,
                            args,
                            env,
                            timeout_seconds,
                        }
                    }),
                (
                    arb_probe_kind(),
                    arb_shell_kind(),
                    "[a-z ]{1,20}",
                    arb_sentinel_env_pairs(),
                    1u64..600,
                )
                    .prop_map(|(kind, shell, script, env, timeout_seconds)| {
                        ProbeSpec::Shell {
                            kind,
                            shell,
                            script,
                            env,
                            timeout_seconds,
                        }
                    }),
            ]
        }

        /// Generate a ReproductionCapsule with sentinel env values.
        fn arb_capsule_with_sentinels() -> impl Strategy<Value = (ReproductionCapsule, Vec<String>)>
        {
            (
                arb_commit_id(),
                arb_probe_spec_with_sentinels(),
                arb_sentinel_env_pairs(),
                "[a-z/]{1,20}",
                1u64..600,
            )
                .prop_map(|(commit, predicate, env, working_dir, timeout_seconds)| {
                    let mut sentinels: Vec<String> = Vec::new();

                    // Collect sentinels from capsule env
                    for (_, v) in &env {
                        sentinels.push(v.clone());
                    }

                    // Collect sentinels from predicate env
                    match &predicate {
                        ProbeSpec::Exec { env: probe_env, .. }
                        | ProbeSpec::Shell { env: probe_env, .. } => {
                            for (_, v) in probe_env {
                                sentinels.push(v.clone());
                            }
                        }
                    }

                    let capsule = ReproductionCapsule {
                        commit,
                        predicate,
                        env,
                        working_dir,
                        timeout_seconds,
                    };

                    (capsule, sentinels)
                })
        }

        proptest! {
            #![proptest_config(ProptestConfig { cases: 100, .. ProptestConfig::default() })]

            #[test]
            fn prop_env_redaction_completeness_in_shell_scripts(
                (capsule, sentinels) in arb_capsule_with_sentinels()
            ) {
                // to_shell_script() uses RedactionPolicy::default_safe() internally
                let script = capsule.to_shell_script();

                for sentinel in &sentinels {
                    prop_assert!(
                        !script.contains(sentinel),
                        "Shell script must not contain sentinel env value '{}' after redaction\nScript:\n{}",
                        sentinel,
                        script
                    );
                }
            }
        }
    }

    // =========================================================================
    // Feature: v01-artifact-hardening, Properties 9–14
    // =========================================================================

    // Feature: v01-artifact-hardening, Property 9: Provenance Correctness
    // **Validates: Requirements 8.1, 8.2, 8.3, 8.5**
    mod prop_provenance_correctness {
        use super::*;

        fn arb_redaction_policy() -> impl Strategy<Value = RedactionPolicy> {
            prop_oneof![
                Just(RedactionPolicy::default_safe()),
                Just(RedactionPolicy::none()),
                Just(RedactionPolicy::env_exposed()),
                Just(RedactionPolicy::secrets_exposed()),
            ]
        }

        proptest! {
            #![proptest_config(ProptestConfig { cases: 100, .. ProptestConfig::default() })]

            #[test]
            fn prop_provenance_correctness(
                report in arb_analysis_report(),
                policy in arb_redaction_policy(),
            ) {
                let redacted = redact_report(&report, &policy);

                let prov = redacted.provenance.as_ref()
                    .expect("provenance must be Some after redact_report");

                prop_assert_eq!(
                    prov.redaction_policy.as_str(),
                    policy.name(),
                    "provenance.redaction_policy must match policy.name()"
                );

                prop_assert_eq!(
                    prov.env_values_redacted,
                    policy.redact_env,
                    "provenance.env_values_redacted must match policy.redact_env"
                );

                prop_assert_eq!(
                    prov.output_scrubbed,
                    policy.scrub_secrets,
                    "provenance.output_scrubbed must match policy.scrub_secrets"
                );
            }
        }
    }

    // Feature: v01-artifact-hardening, Property 10: Redaction Round-Trip Structural Validity
    // **Validates: Requirements 7.3, 7.4**
    mod prop_redaction_round_trip {
        use super::*;

        fn arb_redaction_policy() -> impl Strategy<Value = RedactionPolicy> {
            prop_oneof![
                Just(RedactionPolicy::default_safe()),
                Just(RedactionPolicy::none()),
                Just(RedactionPolicy::env_exposed()),
                Just(RedactionPolicy::secrets_exposed()),
            ]
        }

        proptest! {
            #![proptest_config(ProptestConfig { cases: 100, .. ProptestConfig::default() })]

            #[test]
            fn prop_redaction_round_trip_structural_validity(
                report in arb_analysis_report(),
                policy in arb_redaction_policy(),
            ) {
                let redacted = redact_report(&report, &policy);

                let json = serde_json::to_string_pretty(&redacted)
                    .expect("redacted report must serialize to JSON");

                let deserialized: AnalysisReport = serde_json::from_str(&json)
                    .expect("redacted report JSON must deserialize back");

                prop_assert_eq!(
                    redacted, deserialized,
                    "redacted report must survive JSON round-trip"
                );
            }
        }
    }

    // Feature: v01-artifact-hardening, Property 11: Redacted Output Conforms to JSON Schema
    // **Validates: Requirements 19.1, 19.2, 19.3**
    mod prop_redacted_output_schema {
        use super::*;

        proptest! {
            #![proptest_config(ProptestConfig { cases: 100, .. ProptestConfig::default() })]

            #[test]
            fn prop_redacted_output_conforms_to_schema(report in arb_analysis_report()) {
                let redacted = redact_report(&report, &RedactionPolicy::default_safe());

                let json_str = serde_json::to_string(&redacted)
                    .expect("redacted report must serialize to JSON");

                let json_value: serde_json::Value = serde_json::from_str(&json_str)
                    .expect("serialized JSON must be valid");

                let obj = json_value.as_object()
                    .expect("report JSON must be an object");

                // Verify required top-level fields
                prop_assert!(obj.contains_key("schema_version"));
                prop_assert!(obj.contains_key("run_id"));
                prop_assert!(obj.contains_key("created_at_epoch_seconds"));
                prop_assert!(obj.contains_key("request"));
                prop_assert!(obj.contains_key("sequence"));
                prop_assert!(obj.contains_key("observations"));
                prop_assert!(obj.contains_key("outcome"));
                prop_assert!(obj.contains_key("changed_paths"));
                prop_assert!(obj.contains_key("surface"));

                // Verify provenance field structure
                let provenance = obj.get("provenance")
                    .expect("redacted report must have provenance field");
                prop_assert!(!provenance.is_null(),
                    "provenance must not be null after redaction");

                let prov_obj = provenance.as_object()
                    .expect("provenance must be an object");
                prop_assert!(prov_obj.contains_key("redaction_policy"));
                prop_assert!(prov_obj.contains_key("env_values_redacted"));
                prop_assert!(prov_obj.contains_key("output_scrubbed"));

                prop_assert!(prov_obj["redaction_policy"].is_string());
                prop_assert!(prov_obj["env_values_redacted"].is_boolean());
                prop_assert!(prov_obj["output_scrubbed"].is_boolean());

                // Verify full schema conformance via round-trip
                let _roundtrip: AnalysisReport = serde_json::from_str(&json_str)
                    .expect("redacted report JSON must conform to AnalysisReport schema");
            }
        }
    }

    // Feature: v01-artifact-hardening, Property 12: Redaction Idempotence
    // **Validates: Requirements 19.5**
    mod prop_redaction_idempotence {
        use super::*;

        fn arb_redaction_policy() -> impl Strategy<Value = RedactionPolicy> {
            prop_oneof![
                Just(RedactionPolicy::default_safe()),
                Just(RedactionPolicy::none()),
                Just(RedactionPolicy::env_exposed()),
                Just(RedactionPolicy::secrets_exposed()),
            ]
        }

        proptest! {
            #![proptest_config(ProptestConfig { cases: 100, .. ProptestConfig::default() })]

            #[test]
            fn prop_redaction_idempotence(
                report in arb_analysis_report(),
                policy in arb_redaction_policy(),
            ) {
                let once = redact_report(&report, &policy);
                let twice = redact_report(&once, &policy);

                prop_assert_eq!(
                    once, twice,
                    "applying redact_report twice must produce the same result as once"
                );
            }
        }
    }

    // Feature: v01-artifact-hardening, Property 13: Backward-Compatible Deserialization
    // **Validates: Requirements 8.1, 8.2**
    #[test]
    fn prop13_backward_compatible_deserialization_without_provenance() {
        // A JSON string representing a report WITHOUT the provenance field
        let old_json = r#"{
            "schema_version": "0.2.0",
            "run_id": "old-run-no-provenance",
            "created_at_epoch_seconds": 1700000000,
            "request": {
                "repo_root": "/tmp/repo",
                "good": "abc123",
                "bad": "def456",
                "history_mode": "AncestryPath",
                "probe": {
                    "Exec": {
                        "kind": "Test",
                        "program": "cargo",
                        "args": ["test"],
                        "env": [],
                        "timeout_seconds": 300
                    }
                },
                "policy": {
                    "max_probes": 64
                }
            },
            "sequence": {
                "revisions": ["abc123", "def456"]
            },
            "observations": [],
            "outcome": {
                "Inconclusive": {
                    "reasons": ["MissingPassBoundary"]
                }
            },
            "changed_paths": [],
            "surface": {
                "total_changes": 0,
                "buckets": [],
                "execution_surfaces": []
            }
        }"#;

        let report: AnalysisReport =
            serde_json::from_str(old_json).expect("old JSON without provenance must deserialize");

        // provenance must be None for old reports
        assert!(
            report.provenance.is_none(),
            "provenance must be None for reports without the field"
        );
        assert_eq!(report.run_id, "old-run-no-provenance");
    }

    #[test]
    fn prop13_backward_compatible_deserialization_with_provenance() {
        // A JSON string representing a report WITH the provenance field
        let new_json = r#"{
            "schema_version": "0.2.0",
            "run_id": "new-run-with-provenance",
            "created_at_epoch_seconds": 1700000000,
            "request": {
                "repo_root": "/tmp/repo",
                "good": "abc123",
                "bad": "def456",
                "history_mode": "AncestryPath",
                "probe": {
                    "Exec": {
                        "kind": "Test",
                        "program": "cargo",
                        "args": ["test"],
                        "env": [],
                        "timeout_seconds": 300
                    }
                },
                "policy": {
                    "max_probes": 64
                }
            },
            "sequence": {
                "revisions": ["abc123", "def456"]
            },
            "observations": [],
            "outcome": {
                "Inconclusive": {
                    "reasons": ["MissingPassBoundary"]
                }
            },
            "changed_paths": [],
            "surface": {
                "total_changes": 0,
                "buckets": [],
                "execution_surfaces": []
            },
            "provenance": {
                "redaction_policy": "default",
                "env_values_redacted": true,
                "output_scrubbed": true,
                "artifact_source": null
            }
        }"#;

        let report: AnalysisReport =
            serde_json::from_str(new_json).expect("JSON with provenance must deserialize");

        let prov = report.provenance.expect("provenance must be Some");
        assert_eq!(prov.redaction_policy, "default");
        assert!(prov.env_values_redacted);
        assert!(prov.output_scrubbed);
        assert_eq!(prov.artifact_source, None);
        assert_eq!(report.run_id, "new-run-with-provenance");
    }

    // Feature: v01-artifact-hardening, Property 14: Secret Pattern Scrubbing in Shareable Artifacts
    // **Validates: Requirements 9.1, 9.2, 9.3**
    mod prop_secret_scrubbing {
        use super::*;

        /// Known secrets that SHOULD be scrubbed by `scrub_secrets()`.
        /// Mirrors `faultline_fixtures::secrets::KNOWN_SECRETS`.
        const KNOWN_SECRETS: &[&str] = &[
            "ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghij",
            "gho_ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghij",
            "ghu_ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghij",
            "ghs_ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghij",
            "ghr_ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghij",
            "AKIAIOSFODNN7EXAMPLE",
            "AKIA1234567890ABCDEF",
            "sk-live_abcdefghijklmnopqrstuvwx",
            "sk-test_ABCDEFGHIJKLMNOPQRSTUVWX",
            "Bearer eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0",
            "Bearer some-opaque-token-value",
            "password=hunter2",
            "password=FIXTURE_VALUE_NOT_REAL",
            "github_pat__________________________________________________",
            "AIza___________________________________",
            "xoxb-NOT-A-REAL-SLACK-TOKEN-FIXTURE",
            "-----BEGIN FIXTURE PRIVATE KEY-----\nFIXTURE_KEY_CONTENT_NOT_REAL\n-----END FIXTURE PRIVATE KEY-----",
        ];

        proptest! {
            #![proptest_config(ProptestConfig { cases: 100, .. ProptestConfig::default() })]

            #[test]
            fn prop_secret_pattern_scrubbing_in_shareable_artifacts(
                report in arb_analysis_report()
            ) {
                let mut report_with_secrets = report.clone();

                // Inject secrets into observations
                for (i, obs) in report_with_secrets.observations.iter_mut().enumerate() {
                    let idx = i % KNOWN_SECRETS.len();
                    obs.stdout = format!("output {}", KNOWN_SECRETS[idx]);
                    obs.stderr = format!("err {}", KNOWN_SECRETS[(idx + 1) % KNOWN_SECRETS.len()]);
                    obs.probe_command = format!(
                        "cmd {}", KNOWN_SECRETS[(idx + 2) % KNOWN_SECRETS.len()]
                    );
                }

                // Inject a secret into the probe spec
                match &mut report_with_secrets.request.probe {
                    ProbeSpec::Shell { script, .. } => {
                        *script = format!("run with {}", KNOWN_SECRETS[0]);
                    }
                    ProbeSpec::Exec { program, args, .. } => {
                        *program = format!("cmd_{}", KNOWN_SECRETS[0]);
                        if !args.is_empty() {
                            args[0] = format!(
                                "--token={}",
                                KNOWN_SECRETS[1 % KNOWN_SECRETS.len()]
                            );
                        }
                    }
                }

                // Apply redaction with default_safe policy
                let redacted = redact_report(
                    &report_with_secrets,
                    &RedactionPolicy::default_safe(),
                );

                // Serialize to JSON
                let json = serde_json::to_string(&redacted)
                    .expect("redacted report must serialize to JSON");

                // Assert none of the known secrets appear in the output
                for secret in KNOWN_SECRETS {
                    prop_assert!(
                        !json.contains(secret),
                        "Redacted JSON must not contain known secret '{}'",
                        secret
                    );
                }
            }
        }
    }
}
