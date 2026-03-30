use faultline_codes::{AmbiguityReason, ObservationClass, ProbeKind};
use serde::{Deserialize, Serialize};
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

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CommitId(pub String);

impl fmt::Display for CommitId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevisionSpec(pub String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HistoryMode {
    AncestryPath,
    FirstParent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShellKind {
    Default,
    PosixSh,
    Cmd,
    PowerShell,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
            ProbeSpec::Exec { timeout_seconds, .. } | ProbeSpec::Shell { timeout_seconds, .. } => {
                *timeout_seconds
            }
        }
    }

    pub fn fingerprint(&self) -> String {
        stable_hash(&serde_json::to_string(self).unwrap_or_else(|_| format!("{:?}", self)))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchPolicy {
    pub max_probes: usize,
    pub edge_refine_threshold: usize,
}

impl Default for SearchPolicy {
    fn default() -> Self {
        Self {
            max_probes: 64,
            edge_refine_threshold: 6,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
        stable_hash(&payload)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProbeObservation {
    pub commit: CommitId,
    pub class: ObservationClass,
    pub kind: ProbeKind,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub duration_ms: u64,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
            score: 35,
            label: "low".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
    TypeChanged,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PathChange {
    pub status: ChangeStatus,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubsystemBucket {
    pub name: String,
    pub change_count: usize,
    pub paths: Vec<String>,
    pub surface_kinds: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurfaceSummary {
    pub total_changes: usize,
    pub buckets: Vec<SubsystemBucket>,
    pub execution_surfaces: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunHandle {
    pub id: String,
    pub root: PathBuf,
    pub resumed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckedOutRevision {
    pub commit: CommitId,
    pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisReport {
    pub run_id: String,
    pub created_at_epoch_seconds: u64,
    pub request: AnalysisRequest,
    pub sequence: RevisionSequence,
    pub observations: Vec<ProbeObservation>,
    pub outcome: LocalizationOutcome,
    pub changed_paths: Vec<PathChange>,
    pub surface: SurfaceSummary,
}

pub fn stable_hash(input: &str) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in input.as_bytes() {
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
