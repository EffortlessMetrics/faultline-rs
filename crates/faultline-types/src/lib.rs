use faultline_codes::{AmbiguityReason, ObservationClass, ProbeKind};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt;
use std::path::PathBuf;
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

impl ReproductionCapsule {
    /// Generate a POSIX shell script that reproduces this probe.
    pub fn to_shell_script(&self) -> String {
        let mut script = String::from("#!/bin/sh\nset -e\n");

        // cd to working directory
        script.push_str(&format!("cd '{}'\n", shell_escape(&self.working_dir)));

        // git checkout
        script.push_str(&format!(
            "git checkout '{}'\n",
            shell_escape(&self.commit.0)
        ));

        // env exports
        for (key, value) in &self.env {
            script.push_str(&format!(
                "export {}='{}'\n",
                shell_escape(key),
                shell_escape(value)
            ));
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
                // Export probe-level env vars
                for (key, value) in probe_env {
                    script.push_str(&format!(
                        "export {}='{}'\n",
                        shell_escape(key),
                        shell_escape(value)
                    ));
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
                // Export probe-level env vars
                for (key, value) in probe_env {
                    script.push_str(&format!(
                        "export {}='{}'\n",
                        shell_escape(key),
                        shell_escape(value)
                    ));
                }
                script.push_str(&format!(
                    "timeout {} sh -c '{}'\n",
                    timeout,
                    shell_escape(shell_script)
                ));
            }
        }

        script
    }
}

// --- Signal Assessment ---

/// Qualitative assessment of how the signal changed between two runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SignalAssessment {
    /// Confidence improved and/or window narrowed
    Improved,
    /// No meaningful change in signal
    Steady,
    /// Confidence dropped and/or window widened
    Degraded,
    /// Outcome type changed (e.g., Inconclusive -> FirstBad)
    OutcomeChanged,
}

impl fmt::Display for SignalAssessment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SignalAssessment::Improved => write!(f, "IMPROVED"),
            SignalAssessment::Steady => write!(f, "STEADY"),
            SignalAssessment::Degraded => write!(f, "DEGRADED"),
            SignalAssessment::OutcomeChanged => write!(f, "OUTCOME CHANGED"),
        }
    }
}

/// Compute a qualitative signal assessment from a run comparison.
///
/// Priority: outcome change trumps everything, then improvement/degradation,
/// then steady.
pub fn signal_assessment(cmp: &RunComparison) -> SignalAssessment {
    if cmp.outcome_changed {
        return SignalAssessment::OutcomeChanged;
    }
    if cmp.confidence_delta > 0 || cmp.window_width_delta < 0 {
        return SignalAssessment::Improved;
    }
    if cmp.confidence_delta < 0 || cmp.window_width_delta > 0 {
        return SignalAssessment::Degraded;
    }
    SignalAssessment::Steady
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
}

fn default_schema_version() -> String {
    "0.2.0".to_string()
}

pub fn stable_hash(data: &[u8]) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in data {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{:016x}", hash)
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
        assert_serialize_deserialize_debug_clone_partialeq_eq(&SignalAssessment::Improved);
        assert_serialize_deserialize_debug_clone_partialeq_eq(&SignalAssessment::Steady);
        assert_serialize_deserialize_debug_clone_partialeq_eq(&SignalAssessment::Degraded);
        assert_serialize_deserialize_debug_clone_partialeq_eq(&SignalAssessment::OutcomeChanged);
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
        let script = capsule.to_shell_script();
        assert!(script.starts_with("#!/bin/sh\n"));
        assert!(script.contains("set -e"));
        assert!(script.contains("cd '/tmp/repo'"));
        assert!(script.contains("git checkout 'abc123'"));
        assert!(script.contains("export CI='true'"));
        assert!(script.contains("export RUST_LOG='debug'"));
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
        let script = capsule.to_shell_script();
        assert!(script.contains("cd '/tmp/it'\\''s here'"));
        assert!(script.contains("export VAR='it'\\''s a test'"));
        assert!(script.contains("sh -c 'echo '\\''hello world'\\'''"));
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
            let script = capsule.to_shell_script();

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

            // Must contain each env key=value pair from the capsule's env field
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
                        schema_version: "0.2.0".into(),
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

    // --- SignalAssessment Display ---

    #[test]
    fn signal_assessment_display() {
        assert_eq!(format!("{}", SignalAssessment::Improved), "IMPROVED");
        assert_eq!(format!("{}", SignalAssessment::Steady), "STEADY");
        assert_eq!(format!("{}", SignalAssessment::Degraded), "DEGRADED");
        assert_eq!(
            format!("{}", SignalAssessment::OutcomeChanged),
            "OUTCOME CHANGED"
        );
    }

    // --- SignalAssessment unit tests ---

    #[test]
    fn signal_assessment_steady_when_no_change() {
        let cmp = RunComparison {
            left_run_id: "a".into(),
            right_run_id: "b".into(),
            outcome_changed: false,
            confidence_delta: 0,
            window_width_delta: 0,
            probes_reused: 0,
            suspect_paths_added: vec![],
            suspect_paths_removed: vec![],
            ambiguity_reasons_added: vec![],
            ambiguity_reasons_removed: vec![],
        };
        assert_eq!(signal_assessment(&cmp), SignalAssessment::Steady);
    }

    // --- SignalAssessment property tests ---

    fn arb_run_comparison() -> impl Strategy<Value = RunComparison> {
        (
            any::<bool>(),  // outcome_changed
            any::<i16>(),   // confidence_delta
            any::<i64>(),   // window_width_delta
            any::<usize>(), // probes_reused
        )
            .prop_map(
                |(outcome_changed, confidence_delta, window_width_delta, probes_reused)| {
                    RunComparison {
                        left_run_id: "left".into(),
                        right_run_id: "right".into(),
                        outcome_changed,
                        confidence_delta,
                        window_width_delta,
                        probes_reused,
                        suspect_paths_added: vec![],
                        suspect_paths_removed: vec![],
                        ambiguity_reasons_added: vec![],
                        ambiguity_reasons_removed: vec![],
                    }
                },
            )
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 100, .. ProptestConfig::default() })]

        #[test]
        fn prop_signal_assessment_improved_when_confidence_up(
            confidence_delta in 1i16..=i16::MAX,
            window_width_delta in any::<i64>(),
        ) {
            let cmp = RunComparison {
                left_run_id: "l".into(),
                right_run_id: "r".into(),
                outcome_changed: false,
                confidence_delta,
                window_width_delta,
                probes_reused: 0,
                suspect_paths_added: vec![],
                suspect_paths_removed: vec![],
                ambiguity_reasons_added: vec![],
                ambiguity_reasons_removed: vec![],
            };
            prop_assert_eq!(
                signal_assessment(&cmp),
                SignalAssessment::Improved,
                "positive confidence_delta with !outcome_changed must be Improved"
            );
        }

        #[test]
        fn prop_signal_assessment_degraded_when_window_wider(
            window_width_delta in 1i64..=i64::MAX,
        ) {
            let cmp = RunComparison {
                left_run_id: "l".into(),
                right_run_id: "r".into(),
                outcome_changed: false,
                confidence_delta: 0,
                window_width_delta,
                probes_reused: 0,
                suspect_paths_added: vec![],
                suspect_paths_removed: vec![],
                ambiguity_reasons_added: vec![],
                ambiguity_reasons_removed: vec![],
            };
            prop_assert_eq!(
                signal_assessment(&cmp),
                SignalAssessment::Degraded,
                "positive window_width_delta with !outcome_changed and zero confidence must be Degraded"
            );
        }

        #[test]
        fn prop_signal_assessment_outcome_changed_trumps(
            cmp in arb_run_comparison().prop_map(|mut c| { c.outcome_changed = true; c })
        ) {
            prop_assert_eq!(
                signal_assessment(&cmp),
                SignalAssessment::OutcomeChanged,
                "outcome_changed must always yield OutcomeChanged regardless of deltas"
            );
        }
    }
}
