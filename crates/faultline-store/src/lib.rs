use faultline_ports::RunStorePort;
use faultline_types::{AnalysisReport, AnalysisRequest, ProbeObservation, Result, RunHandle};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct FileRunStore {
    root: PathBuf,
}

impl FileRunStore {
    pub fn new(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    fn run_root(&self, run_id: &str) -> PathBuf {
        self.root.join(run_id)
    }

    fn observations_path(&self, run: &RunHandle) -> PathBuf {
        run.root.join("observations.json")
    }

    fn request_path(&self, run: &RunHandle) -> PathBuf {
        run.root.join("request.json")
    }

    fn report_path(&self, run: &RunHandle) -> PathBuf {
        run.root.join("report.json")
    }

    fn read_json_or_default<T>(&self, path: &Path) -> Result<T>
    where
        T: serde::de::DeserializeOwned + Default,
    {
        if !path.exists() {
            return Ok(T::default());
        }
        let raw = fs::read_to_string(path)?;
        Ok(serde_json::from_str(&raw)?)
    }
}

impl RunStorePort for FileRunStore {
    fn prepare_run(&self, request: &AnalysisRequest) -> Result<RunHandle> {
        let run_id = request.fingerprint();
        let root = self.run_root(&run_id);
        let resumed = root.exists();
        fs::create_dir_all(&root)?;
        let handle = RunHandle {
            id: run_id,
            root,
            resumed,
        };
        fs::write(
            self.request_path(&handle),
            serde_json::to_string_pretty(request)?,
        )?;
        Ok(handle)
    }

    fn load_observations(&self, run: &RunHandle) -> Result<Vec<ProbeObservation>> {
        self.read_json_or_default(&self.observations_path(run))
    }

    fn save_observation(&self, run: &RunHandle, observation: &ProbeObservation) -> Result<()> {
        let mut observations: Vec<ProbeObservation> = self.load_observations(run)?;
        if let Some(existing) = observations
            .iter_mut()
            .find(|item| item.commit == observation.commit)
        {
            *existing = observation.clone();
        } else {
            observations.push(observation.clone());
        }
        observations.sort_by(|a, b| a.commit.0.cmp(&b.commit.0));
        fs::write(
            self.observations_path(run),
            serde_json::to_string_pretty(&observations)?,
        )?;
        Ok(())
    }

    fn save_report(&self, run: &RunHandle, report: &AnalysisReport) -> Result<()> {
        fs::write(self.report_path(run), serde_json::to_string_pretty(report)?)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use faultline_codes::{AmbiguityReason, ObservationClass, ProbeKind};
    use faultline_types::{
        AnalysisReport, AnalysisRequest, ChangeStatus, CommitId, Confidence, HistoryMode,
        LocalizationOutcome, PathChange, ProbeObservation, ProbeSpec, RevisionSequence,
        RevisionSpec, SearchPolicy, ShellKind, SubsystemBucket, SurfaceSummary,
    };
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn sample_probe_spec() -> ProbeSpec {
        ProbeSpec::Exec {
            kind: ProbeKind::Test,
            program: "cargo".into(),
            args: vec!["test".into()],
            env: vec![],
            timeout_seconds: 300,
        }
    }

    fn sample_request() -> AnalysisRequest {
        AnalysisRequest {
            repo_root: PathBuf::from("/tmp/repo"),
            good: RevisionSpec("abc123".into()),
            bad: RevisionSpec("def456".into()),
            history_mode: HistoryMode::AncestryPath,
            probe: sample_probe_spec(),
            policy: SearchPolicy::default(),
        }
    }

    fn sample_observation(commit: &str, class: ObservationClass) -> ProbeObservation {
        ProbeObservation {
            commit: CommitId(commit.into()),
            class,
            kind: ProbeKind::Test,
            exit_code: Some(0),
            timed_out: false,
            duration_ms: 100,
            stdout: "ok".into(),
            stderr: String::new(),
        }
    }

    fn sample_report() -> AnalysisReport {
        AnalysisReport {
            run_id: "run-1".into(),
            created_at_epoch_seconds: 1700000000,
            request: sample_request(),
            sequence: RevisionSequence {
                revisions: vec![CommitId("abc123".into()), CommitId("def456".into())],
            },
            observations: vec![sample_observation("abc123", ObservationClass::Pass)],
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

    // --- prepare_run tests ---

    #[test]
    fn prepare_run_creates_directory_and_request_json() {
        let tmp = TempDir::new().unwrap();
        let store = FileRunStore::new(tmp.path().join("runs")).unwrap();
        let request = sample_request();

        let handle = store.prepare_run(&request).unwrap();

        assert!(!handle.resumed);
        assert!(handle.root.exists());
        assert_eq!(handle.id, request.fingerprint());

        let request_path = handle.root.join("request.json");
        assert!(request_path.exists());
        let raw = fs::read_to_string(&request_path).unwrap();
        let deserialized: AnalysisRequest = serde_json::from_str(&raw).unwrap();
        assert_eq!(deserialized, request);
    }

    #[test]
    fn prepare_run_sets_resumed_on_second_call() {
        let tmp = TempDir::new().unwrap();
        let store = FileRunStore::new(tmp.path().join("runs")).unwrap();
        let request = sample_request();

        let handle1 = store.prepare_run(&request).unwrap();
        assert!(!handle1.resumed);

        let handle2 = store.prepare_run(&request).unwrap();
        assert!(handle2.resumed);
        assert_eq!(handle1.id, handle2.id);
    }

    // --- load_observations tests ---

    #[test]
    fn load_observations_returns_empty_when_no_file() {
        let tmp = TempDir::new().unwrap();
        let store = FileRunStore::new(tmp.path().join("runs")).unwrap();
        let request = sample_request();
        let handle = store.prepare_run(&request).unwrap();

        let obs = store.load_observations(&handle).unwrap();
        assert!(obs.is_empty());
    }

    // --- save_observation tests ---

    #[test]
    fn save_and_load_single_observation() {
        let tmp = TempDir::new().unwrap();
        let store = FileRunStore::new(tmp.path().join("runs")).unwrap();
        let request = sample_request();
        let handle = store.prepare_run(&request).unwrap();

        let obs = sample_observation("abc123", ObservationClass::Pass);
        store.save_observation(&handle, &obs).unwrap();

        let loaded = store.load_observations(&handle).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0], obs);
    }

    #[test]
    fn save_observation_upserts_by_commit_id() {
        let tmp = TempDir::new().unwrap();
        let store = FileRunStore::new(tmp.path().join("runs")).unwrap();
        let request = sample_request();
        let handle = store.prepare_run(&request).unwrap();

        let obs1 = sample_observation("abc123", ObservationClass::Pass);
        store.save_observation(&handle, &obs1).unwrap();

        // Save again with different class for same commit — should replace
        let obs2 = sample_observation("abc123", ObservationClass::Fail);
        store.save_observation(&handle, &obs2).unwrap();

        let loaded = store.load_observations(&handle).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].class, ObservationClass::Fail);
    }

    #[test]
    fn save_observation_sorts_by_commit_id() {
        let tmp = TempDir::new().unwrap();
        let store = FileRunStore::new(tmp.path().join("runs")).unwrap();
        let request = sample_request();
        let handle = store.prepare_run(&request).unwrap();

        // Save in reverse order
        store
            .save_observation(&handle, &sample_observation("ccc", ObservationClass::Pass))
            .unwrap();
        store
            .save_observation(&handle, &sample_observation("aaa", ObservationClass::Fail))
            .unwrap();
        store
            .save_observation(&handle, &sample_observation("bbb", ObservationClass::Skip))
            .unwrap();

        let loaded = store.load_observations(&handle).unwrap();
        assert_eq!(loaded.len(), 3);
        assert_eq!(loaded[0].commit.0, "aaa");
        assert_eq!(loaded[1].commit.0, "bbb");
        assert_eq!(loaded[2].commit.0, "ccc");
    }

    // --- save_report tests ---

    #[test]
    fn save_and_read_report() {
        let tmp = TempDir::new().unwrap();
        let store = FileRunStore::new(tmp.path().join("runs")).unwrap();
        let request = sample_request();
        let handle = store.prepare_run(&request).unwrap();

        let report = sample_report();
        store.save_report(&handle, &report).unwrap();

        let report_path = handle.root.join("report.json");
        assert!(report_path.exists());
        let raw = fs::read_to_string(&report_path).unwrap();
        let deserialized: AnalysisReport = serde_json::from_str(&raw).unwrap();
        assert_eq!(deserialized, report);
    }

    // --- FileRunStore::new tests ---

    #[test]
    fn new_creates_root_directory() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("deep").join("nested").join("runs");
        assert!(!root.exists());

        let _store = FileRunStore::new(&root).unwrap();
        assert!(root.exists());
    }

    // --- Property tests: Property 11 — Run Store Round-Trip ---

    use proptest::prelude::*;

    fn arb_probe_kind() -> impl Strategy<Value = ProbeKind> {
        prop_oneof![
            Just(ProbeKind::Build),
            Just(ProbeKind::Test),
            Just(ProbeKind::Lint),
            Just(ProbeKind::PerfThreshold),
            Just(ProbeKind::Custom),
        ]
    }

    fn arb_observation_class() -> impl Strategy<Value = ObservationClass> {
        prop_oneof![
            Just(ObservationClass::Pass),
            Just(ObservationClass::Fail),
            Just(ObservationClass::Skip),
            Just(ObservationClass::Indeterminate),
        ]
    }

    fn arb_commit_id() -> impl Strategy<Value = CommitId> {
        "[a-f0-9]{8,40}".prop_map(CommitId)
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

    fn arb_revision_spec() -> impl Strategy<Value = RevisionSpec> {
        "[a-f0-9]{8,40}".prop_map(RevisionSpec)
    }

    fn arb_history_mode() -> impl Strategy<Value = HistoryMode> {
        prop_oneof![
            Just(HistoryMode::AncestryPath),
            Just(HistoryMode::FirstParent),
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

    fn arb_revision_sequence() -> impl Strategy<Value = RevisionSequence> {
        prop::collection::vec(arb_commit_id(), 2..10)
            .prop_map(|revisions| RevisionSequence { revisions })
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

    // Feature: v01-release-train, Property 11: Run Store Round-Trip
    // **Validates: Requirements 4.2, 4.5, 4.6**
    proptest! {
        #![proptest_config(ProptestConfig { cases: 100, .. ProptestConfig::default() })]

        #[test]
        fn prop_observation_round_trip(obs in arb_probe_observation()) {
            let tmp = TempDir::new().unwrap();
            let store = FileRunStore::new(tmp.path().join("runs")).unwrap();
            let request = sample_request();
            let handle = store.prepare_run(&request).unwrap();

            store.save_observation(&handle, &obs).unwrap();
            let loaded = store.load_observations(&handle).unwrap();

            prop_assert!(
                loaded.iter().any(|o| *o == obs),
                "saved ProbeObservation must be present in loaded observations"
            );
        }

        #[test]
        fn prop_report_round_trip(report in arb_analysis_report()) {
            let tmp = TempDir::new().unwrap();
            let store = FileRunStore::new(tmp.path().join("runs")).unwrap();
            let request = sample_request();
            let handle = store.prepare_run(&request).unwrap();

            store.save_report(&handle, &report).unwrap();

            let report_path = handle.root.join("report.json");
            let raw = fs::read_to_string(&report_path).unwrap();
            let deserialized: AnalysisReport = serde_json::from_str(&raw).unwrap();
            prop_assert_eq!(report, deserialized, "AnalysisReport round-trip must preserve equality");
        }

        #[test]
        fn prop_request_round_trip(request in arb_analysis_request()) {
            let tmp = TempDir::new().unwrap();
            let store = FileRunStore::new(tmp.path().join("runs")).unwrap();

            let handle = store.prepare_run(&request).unwrap();

            let request_path = handle.root.join("request.json");
            let raw = fs::read_to_string(&request_path).unwrap();
            let deserialized: AnalysisRequest = serde_json::from_str(&raw).unwrap();
            prop_assert_eq!(request, deserialized, "AnalysisRequest round-trip must preserve equality");
        }

        // Feature: v01-release-train, Property 12: Run Store Resumability
        // **Validates: Requirements 4.3**
        #[test]
        fn prop_run_store_resumability(
            request in arb_analysis_request(),
            observations in prop::collection::vec(arb_probe_observation(), 1..=5),
        ) {
            let tmp = TempDir::new().unwrap();
            let store = FileRunStore::new(tmp.path().join("runs")).unwrap();

            // First prepare_run — should NOT be resumed
            let handle1 = store.prepare_run(&request).unwrap();
            prop_assert!(!handle1.resumed, "first prepare_run must have resumed == false");

            // Save all observations via the first handle
            for obs in &observations {
                store.save_observation(&handle1, obs).unwrap();
            }

            // Second prepare_run — should be resumed
            let handle2 = store.prepare_run(&request).unwrap();
            prop_assert!(handle2.resumed, "second prepare_run must have resumed == true");
            prop_assert_eq!(&handle1.id, &handle2.id, "both handles must share the same run ID");

            // Load observations from the second handle
            let loaded = store.load_observations(&handle2).unwrap();

            // Verify all saved observations are present in the loaded list.
            // Note: save_observation upserts by commit ID, so if multiple observations
            // share the same commit, only the last one is kept.
            let mut expected_by_commit: std::collections::HashMap<&str, &ProbeObservation> =
                std::collections::HashMap::new();
            for obs in &observations {
                expected_by_commit.insert(&obs.commit.0, obs);
            }

            for (commit_id, expected_obs) in &expected_by_commit {
                prop_assert!(
                    loaded.iter().any(|o| o == *expected_obs),
                    "observation for commit {} must be present in loaded observations after resume",
                    commit_id
                );
            }

            prop_assert_eq!(
                loaded.len(),
                expected_by_commit.len(),
                "loaded observation count must match unique commit count"
            );
        }
    }
}
