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
        stable_hash(payload.as_bytes())
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
            score: 25,
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
        }
    }

    #[test]
    fn all_types_derive_required_traits() {
        assert_serialize_deserialize_debug_clone_partialeq_eq(&CommitId("a".into()));
        assert_serialize_deserialize_debug_clone_partialeq_eq(&RevisionSpec("a".into()));
        assert_serialize_deserialize_debug_clone_partialeq_eq(&HistoryMode::AncestryPath);
        assert_serialize_deserialize_debug_clone_partialeq_eq(&sample_probe_spec());
        assert_serialize_deserialize_debug_clone_partialeq_eq(&SearchPolicy::default());
        assert_serialize_deserialize_debug_clone_partialeq_eq(&sample_analysis_request());
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
        });
        assert_serialize_deserialize_debug_clone_partialeq_eq(&CheckedOutRevision {
            commit: CommitId("a".into()),
            path: PathBuf::from("/tmp"),
        });
        assert_serialize_deserialize_debug_clone_partialeq_eq(&sample_report());
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
        assert_eq!(p.edge_refine_threshold, 6);
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
                        timeout_seconds,
                    }
                }),
        ]
    }

    fn arb_search_policy() -> impl Strategy<Value = SearchPolicy> {
        (1usize..128, 1usize..16).prop_map(|(max_probes, edge_refine_threshold)| SearchPolicy {
            max_probes,
            edge_refine_threshold,
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
        )
            .prop_map(
                |(commit, class, kind, exit_code, timed_out, duration_ms, stdout, stderr)| {
                    ProbeObservation {
                        commit,
                        class,
                        kind,
                        exit_code,
                        timed_out,
                        duration_ms,
                        stdout,
                        stderr,
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
                        run_id,
                        created_at_epoch_seconds,
                        request,
                        sequence,
                        observations,
                        outcome,
                        changed_paths,
                        surface,
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
}
