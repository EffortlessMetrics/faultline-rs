use faultline_codes::ObservationClass;
use faultline_localization::LocalizationSession;
use faultline_ports::{CheckoutPort, HistoryPort, ProbePort, RunStorePort};
use faultline_surface::SurfaceAnalyzer;
use faultline_types::{
    AnalysisReport, AnalysisRequest, FaultlineError, Result, RunHandle, now_epoch_seconds,
};

/// Options controlling the behavior of `FaultlineApp::localize`.
#[derive(Debug, Clone, Default)]
pub struct LocalizeOptions {
    /// If true, clear cached observations before starting (--force).
    pub force: bool,
    /// If true, delete the entire run directory before prepare_run (--fresh).
    pub fresh: bool,
    /// If true, skip rendering (--no-render). The app layer does not own
    /// rendering, but this flag is threaded through for the caller's use.
    pub no_render: bool,
}

#[derive(Debug, Clone)]
pub struct LocalizedRun {
    pub run: RunHandle,
    pub report: AnalysisReport,
}

pub struct FaultlineApp<'a> {
    history: &'a dyn HistoryPort,
    checkout: &'a dyn CheckoutPort,
    probe: &'a dyn ProbePort,
    store: &'a dyn RunStorePort,
    surface: SurfaceAnalyzer,
}

impl<'a> FaultlineApp<'a> {
    pub fn new(
        history: &'a dyn HistoryPort,
        checkout: &'a dyn CheckoutPort,
        probe: &'a dyn ProbePort,
        store: &'a dyn RunStorePort,
    ) -> Self {
        Self {
            history,
            checkout,
            probe,
            store,
            surface: SurfaceAnalyzer,
        }
    }

    pub fn localize(&self, request: AnalysisRequest) -> Result<LocalizedRun> {
        self.localize_with_options(request, LocalizeOptions::default())
    }

    pub fn localize_with_options(
        &self,
        request: AnalysisRequest,
        options: LocalizeOptions,
    ) -> Result<LocalizedRun> {
        // --fresh: get the run handle first to know the root, then delete and re-create
        if options.fresh {
            // First prepare to discover the run directory path
            let handle = self.store.prepare_run(&request)?;
            self.store.delete_run(&handle)?;
            // Fall through — prepare_run below will create it fresh
        }

        let run = self.store.prepare_run(&request)?;

        // --force: clear cached observations before starting the loop
        if options.force {
            self.store.clear_observations(&run)?;
        }

        let sequence = self
            .history
            .linearize(&request.good, &request.bad, request.history_mode)?;

        let mut session = LocalizationSession::new(sequence.clone(), request.policy.clone())?;

        // Replay cached observations, preserving their existing sequence_index values.
        let cached = self.store.load_observations(&run)?;
        let mut next_sequence_index: u64 = if cached.is_empty() {
            0
        } else {
            cached.iter().map(|o| o.sequence_index).max().unwrap_or(0) + 1
        };
        for observation in cached {
            session.record(observation)?;
        }

        self.ensure_boundary(
            &run,
            &request,
            &mut session,
            0,
            ObservationClass::Pass,
            "known-good",
            &mut next_sequence_index,
        )?;
        self.ensure_boundary(
            &run,
            &request,
            &mut session,
            sequence.len() - 1,
            ObservationClass::Fail,
            "known-bad",
            &mut next_sequence_index,
        )?;

        let mut probe_count = 0usize;
        let max_probes = session.max_probes();
        while probe_count < max_probes {
            let Some(commit) = session.next_probe() else {
                break;
            };
            let mut observation = self.probe_commit(&request, &commit)?;
            observation.sequence_index = next_sequence_index;
            next_sequence_index += 1;

            // Full-log persistence: if stdout/stderr was truncated, save full logs
            self.persist_truncated_logs(&run, &observation);

            self.store.save_observation(&run, &observation)?;
            session.record(observation)?;
            probe_count += 1;
        }

        let outcome = session.outcome();
        let changed_paths = if let Some((from, to)) = outcome.boundary_pair() {
            self.history.changed_paths(from, to)?
        } else {
            Vec::new()
        };
        let surface = self.surface.summarize(&changed_paths);
        let report = AnalysisReport {
            schema_version: "0.1.0".into(),
            run_id: run.id.clone(),
            created_at_epoch_seconds: now_epoch_seconds(),
            request,
            sequence,
            observations: session.observation_list(),
            outcome,
            changed_paths,
            surface,
        };
        self.store.save_report(&run, &report)?;
        Ok(LocalizedRun { run, report })
    }

    #[allow(clippy::too_many_arguments)]
    fn ensure_boundary(
        &self,
        run: &RunHandle,
        request: &AnalysisRequest,
        session: &mut LocalizationSession,
        index: usize,
        expected: ObservationClass,
        label: &str,
        next_sequence_index: &mut u64,
    ) -> Result<()> {
        let commit = session
            .sequence()
            .revisions
            .get(index)
            .ok_or_else(|| FaultlineError::Domain("missing boundary index".to_string()))?
            .clone();

        if !session.has_observation(&commit) {
            let mut observation = self.probe_commit(request, &commit)?;
            observation.sequence_index = *next_sequence_index;
            *next_sequence_index += 1;

            // Full-log persistence for boundary probes
            self.persist_truncated_logs(run, &observation);

            self.store.save_observation(run, &observation)?;
            session.record(observation)?;
        }

        let observed = session
            .get_observation(&commit)
            .ok_or_else(|| FaultlineError::Domain("boundary observation missing".to_string()))?;
        if observed.class != expected {
            return Err(FaultlineError::InvalidBoundary(format!(
                "{label} boundary {} evaluated as {:?}; expected {:?}",
                commit.0, observed.class, expected
            )));
        }
        Ok(())
    }

    /// If stdout or stderr ends with "[truncated]", persist the (truncated)
    /// output to per-commit log files via the store. In the current architecture
    /// the probe adapter truncates in-memory and the full output is not available
    /// at the app layer — this call saves what we have so the log files exist
    /// for diagnostic purposes.
    fn persist_truncated_logs(
        &self,
        run: &RunHandle,
        observation: &faultline_types::ProbeObservation,
    ) {
        let stdout_truncated = observation.stdout.ends_with("[truncated]");
        let stderr_truncated = observation.stderr.ends_with("[truncated]");
        if stdout_truncated || stderr_truncated {
            let _ = self.store.save_probe_logs(
                run,
                &observation.commit.0,
                &observation.stdout,
                &observation.stderr,
            );
        }
    }

    fn probe_commit(
        &self,
        request: &AnalysisRequest,
        commit: &faultline_types::CommitId,
    ) -> Result<faultline_types::ProbeObservation> {
        let checkout = self.checkout.checkout_revision(commit)?;
        let result = self.probe.run(&checkout, &request.probe);
        let cleanup = self.checkout.cleanup_checkout(&checkout);
        match (result, cleanup) {
            (Ok(observation), Ok(())) => Ok(observation),
            (Err(err), Ok(())) => Err(err),
            (Ok(_), Err(cleanup_err)) => Err(cleanup_err),
            (Err(err), Err(_cleanup_err)) => Err(err),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use faultline_codes::{ObservationClass, ProbeKind};
    use faultline_ports::{CheckoutPort, HistoryPort, ProbePort, RunStorePort};
    use faultline_types::{
        AnalysisReport, AnalysisRequest, CheckedOutRevision, CommitId, HistoryMode, PathChange,
        ProbeObservation, ProbeSpec, RevisionSequence, RevisionSpec, RunHandle, SearchPolicy,
        ShellKind,
    };
    use proptest::prelude::*;
    use std::cell::Cell;
    use std::path::PathBuf;

    // ── Mock HistoryPort ──────────────────────────────────────────────
    struct MockHistory {
        sequence: RevisionSequence,
    }

    impl HistoryPort for MockHistory {
        fn linearize(
            &self,
            _good: &RevisionSpec,
            _bad: &RevisionSpec,
            _mode: HistoryMode,
        ) -> faultline_types::Result<RevisionSequence> {
            Ok(self.sequence.clone())
        }

        fn changed_paths(
            &self,
            _from: &CommitId,
            _to: &CommitId,
        ) -> faultline_types::Result<Vec<PathChange>> {
            Ok(Vec::new())
        }
    }

    // ── Mock CheckoutPort ─────────────────────────────────────────────
    struct MockCheckout;

    impl CheckoutPort for MockCheckout {
        fn checkout_revision(
            &self,
            commit: &CommitId,
        ) -> faultline_types::Result<CheckedOutRevision> {
            Ok(CheckedOutRevision {
                commit: commit.clone(),
                path: PathBuf::from("/tmp/mock"),
            })
        }

        fn cleanup_checkout(&self, _checkout: &CheckedOutRevision) -> faultline_types::Result<()> {
            Ok(())
        }
    }

    // ── Mock ProbePort (tracks call count) ────────────────────────────
    struct MockProbe {
        /// Tracks total probe invocations.
        call_count: Cell<usize>,
    }

    impl ProbePort for MockProbe {
        fn run(
            &self,
            checkout: &CheckedOutRevision,
            _probe: &ProbeSpec,
        ) -> faultline_types::Result<ProbeObservation> {
            self.call_count.set(self.call_count.get() + 1);

            // First commit → Pass, last commit → Fail, everything else → Fail
            // (keeps narrowing going as long as possible)
            let commit_num: usize = checkout
                .commit
                .0
                .strip_prefix("c")
                .unwrap_or("0")
                .parse()
                .unwrap_or(0);

            let class = if commit_num == 0 {
                ObservationClass::Pass
            } else {
                ObservationClass::Fail
            };

            Ok(ProbeObservation {
                commit: checkout.commit.clone(),
                class,
                kind: ProbeKind::Test,
                exit_code: Some(if class == ObservationClass::Pass {
                    0
                } else {
                    1
                }),
                timed_out: false,
                duration_ms: 1,
                stdout: String::new(),
                stderr: String::new(),
                sequence_index: 0,
                signal_number: None,
                probe_command: String::new(),
                working_dir: String::new(),
            })
        }
    }

    // ── Mock RunStorePort ─────────────────────────────────────────────
    struct MockRunStore;

    impl RunStorePort for MockRunStore {
        fn prepare_run(&self, _request: &AnalysisRequest) -> faultline_types::Result<RunHandle> {
            Ok(RunHandle {
                id: "mock-run".to_string(),
                root: PathBuf::from("/tmp/mock-run"),
                resumed: false,
                schema_version: "0.1.0".into(),
                tool_version: String::new(),
            })
        }

        fn load_observations(
            &self,
            _run: &RunHandle,
        ) -> faultline_types::Result<Vec<ProbeObservation>> {
            Ok(Vec::new())
        }

        fn save_observation(
            &self,
            _run: &RunHandle,
            _observation: &ProbeObservation,
        ) -> faultline_types::Result<()> {
            Ok(())
        }

        fn save_report(
            &self,
            _run: &RunHandle,
            _report: &AnalysisReport,
        ) -> faultline_types::Result<()> {
            Ok(())
        }

        fn load_report(&self, _run: &RunHandle) -> faultline_types::Result<Option<AnalysisReport>> {
            Ok(None)
        }

        fn save_probe_logs(
            &self,
            _run: &RunHandle,
            _commit_sha: &str,
            _stdout: &str,
            _stderr: &str,
        ) -> faultline_types::Result<()> {
            Ok(())
        }

        fn clear_observations(&self, _run: &RunHandle) -> faultline_types::Result<()> {
            Ok(())
        }

        fn delete_run(&self, _run: &RunHandle) -> faultline_types::Result<()> {
            Ok(())
        }
    }

    /// Build a revision sequence of `n` commits labelled c0..c{n-1}.
    fn make_sequence(n: usize) -> RevisionSequence {
        RevisionSequence {
            revisions: (0..n).map(|i| CommitId(format!("c{i}"))).collect(),
        }
    }

    fn make_request(max_probes: usize) -> AnalysisRequest {
        AnalysisRequest {
            repo_root: PathBuf::from("/tmp/repo"),
            good: RevisionSpec("c0".into()),
            bad: RevisionSpec("c19".into()),
            history_mode: HistoryMode::AncestryPath,
            probe: ProbeSpec::Shell {
                kind: ProbeKind::Test,
                shell: ShellKind::Default,
                script: "true".into(),
                env: vec![],
                timeout_seconds: 60,
            },
            policy: SearchPolicy { max_probes },
        }
    }

    // ── Configurable MockProbePort (returns specific classes per commit) ─
    struct ConfigurableMockProbe {
        /// Maps commit ID string → ObservationClass to return.
        overrides: std::collections::HashMap<String, ObservationClass>,
        /// Default class for commits not in overrides.
        default_class: ObservationClass,
    }

    impl ProbePort for ConfigurableMockProbe {
        fn run(
            &self,
            checkout: &CheckedOutRevision,
            _probe: &ProbeSpec,
        ) -> faultline_types::Result<ProbeObservation> {
            let class = self
                .overrides
                .get(&checkout.commit.0)
                .copied()
                .unwrap_or(self.default_class);

            Ok(ProbeObservation {
                commit: checkout.commit.clone(),
                class,
                kind: ProbeKind::Test,
                exit_code: Some(if class == ObservationClass::Pass {
                    0
                } else {
                    1
                }),
                timed_out: false,
                duration_ms: 1,
                stdout: String::new(),
                stderr: String::new(),
                sequence_index: 0,
                signal_number: None,
                probe_command: String::new(),
                working_dir: String::new(),
            })
        }
    }

    fn make_request_for_sequence(n: usize, max_probes: usize) -> AnalysisRequest {
        AnalysisRequest {
            repo_root: PathBuf::from("/tmp/repo"),
            good: RevisionSpec("c0".into()),
            bad: RevisionSpec(format!("c{}", n - 1)),
            history_mode: HistoryMode::AncestryPath,
            probe: ProbeSpec::Shell {
                kind: ProbeKind::Test,
                shell: ShellKind::Default,
                script: "true".into(),
                env: vec![],
                timeout_seconds: 60,
            },
            policy: SearchPolicy { max_probes },
        }
    }

    // ── Tracking ProbePort (records probed commits) ─────────────────
    struct TrackingProbe {
        /// Maps commit ID string → ObservationClass to return.
        overrides: std::collections::HashMap<String, ObservationClass>,
        default_class: ObservationClass,
        /// Records which commits were actually probed (in order).
        probed: std::cell::RefCell<Vec<String>>,
    }

    impl ProbePort for TrackingProbe {
        fn run(
            &self,
            checkout: &CheckedOutRevision,
            _probe: &ProbeSpec,
        ) -> faultline_types::Result<ProbeObservation> {
            self.probed.borrow_mut().push(checkout.commit.0.clone());

            let class = self
                .overrides
                .get(&checkout.commit.0)
                .copied()
                .unwrap_or(self.default_class);

            Ok(ProbeObservation {
                commit: checkout.commit.clone(),
                class,
                kind: ProbeKind::Test,
                exit_code: Some(match class {
                    ObservationClass::Pass => 0,
                    ObservationClass::Skip => 125,
                    _ => 1,
                }),
                timed_out: false,
                duration_ms: 1,
                stdout: String::new(),
                stderr: String::new(),
                sequence_index: 0,
                signal_number: None,
                probe_command: String::new(),
                working_dir: String::new(),
            })
        }
    }

    // ── Mock RunStorePort that returns cached observations (resumed run) ─
    struct CachedRunStore {
        cached_observations: Vec<ProbeObservation>,
    }

    impl RunStorePort for CachedRunStore {
        fn prepare_run(&self, _request: &AnalysisRequest) -> faultline_types::Result<RunHandle> {
            Ok(RunHandle {
                id: "resumed-run".to_string(),
                root: PathBuf::from("/tmp/resumed-run"),
                resumed: true,
                schema_version: "0.1.0".into(),
                tool_version: String::new(),
            })
        }

        fn load_observations(
            &self,
            _run: &RunHandle,
        ) -> faultline_types::Result<Vec<ProbeObservation>> {
            Ok(self.cached_observations.clone())
        }

        fn save_observation(
            &self,
            _run: &RunHandle,
            _observation: &ProbeObservation,
        ) -> faultline_types::Result<()> {
            Ok(())
        }

        fn save_report(
            &self,
            _run: &RunHandle,
            _report: &AnalysisReport,
        ) -> faultline_types::Result<()> {
            Ok(())
        }

        fn load_report(&self, _run: &RunHandle) -> faultline_types::Result<Option<AnalysisReport>> {
            Ok(None)
        }

        fn save_probe_logs(
            &self,
            _run: &RunHandle,
            _commit_sha: &str,
            _stdout: &str,
            _stderr: &str,
        ) -> faultline_types::Result<()> {
            Ok(())
        }

        fn clear_observations(&self, _run: &RunHandle) -> faultline_types::Result<()> {
            Ok(())
        }

        fn delete_run(&self, _run: &RunHandle) -> faultline_types::Result<()> {
            Ok(())
        }
    }

    // ── Integration test: cached-resume scenario ─────────────────────
    // Validates: Requirements 4.3, 4.4, 12.7
    //
    // Simulates a resumed run where boundary observations (c0=Pass, c4=Fail)
    // and some intermediate observations are already cached. Verifies that:
    // - Cached commits are NOT re-probed
    // - The localization loop only probes uncached commits
    // - The final outcome is FirstBad with the correct boundary pair
    #[test]
    fn integration_cached_resume_skips_cached_commits() {
        // Sequence: c0, c1, c2, c3, c4 (5 commits)
        let sequence = make_sequence(5);
        let history = MockHistory {
            sequence: sequence.clone(),
        };
        let checkout = MockCheckout;

        // Pre-cached observations: boundaries + c2 (the midpoint)
        let cached_observations = vec![
            ProbeObservation {
                commit: CommitId("c0".into()),
                class: ObservationClass::Pass,
                kind: ProbeKind::Test,
                exit_code: Some(0),
                timed_out: false,
                duration_ms: 10,
                stdout: String::new(),
                stderr: String::new(),
                sequence_index: 0,
                signal_number: None,
                probe_command: String::new(),
                working_dir: String::new(),
            },
            ProbeObservation {
                commit: CommitId("c4".into()),
                class: ObservationClass::Fail,
                kind: ProbeKind::Test,
                exit_code: Some(1),
                timed_out: false,
                duration_ms: 10,
                stdout: String::new(),
                stderr: String::new(),
                sequence_index: 1,
                signal_number: None,
                probe_command: String::new(),
                working_dir: String::new(),
            },
            ProbeObservation {
                commit: CommitId("c2".into()),
                class: ObservationClass::Fail,
                kind: ProbeKind::Test,
                exit_code: Some(1),
                timed_out: false,
                duration_ms: 10,
                stdout: String::new(),
                stderr: String::new(),
                sequence_index: 2,
                signal_number: None,
                probe_command: String::new(),
                working_dir: String::new(),
            },
        ];

        // The probe should return Pass for c1 (so we get FirstBad at c1→c2)
        let mut overrides = std::collections::HashMap::new();
        overrides.insert("c1".into(), ObservationClass::Pass);
        overrides.insert("c3".into(), ObservationClass::Fail);

        let probe = TrackingProbe {
            overrides,
            default_class: ObservationClass::Fail,
            probed: std::cell::RefCell::new(Vec::new()),
        };

        let store = CachedRunStore {
            cached_observations,
        };

        let app = FaultlineApp::new(&history, &checkout, &probe, &store);
        let request = make_request_for_sequence(5, 20);

        let result = app.localize(request).expect("localize should succeed");

        // Verify cached commits were NOT re-probed
        let probed_commits = probe.probed.borrow();
        assert!(
            !probed_commits.contains(&"c0".to_string()),
            "c0 was cached (Pass) and should not have been re-probed"
        );
        assert!(
            !probed_commits.contains(&"c4".to_string()),
            "c4 was cached (Fail) and should not have been re-probed"
        );
        assert!(
            !probed_commits.contains(&"c2".to_string()),
            "c2 was cached (Fail) and should not have been re-probed"
        );

        // Verify the outcome is FirstBad with last_good=c1, first_bad=c2
        match &result.report.outcome {
            faultline_types::LocalizationOutcome::FirstBad {
                last_good,
                first_bad,
                ..
            } => {
                assert_eq!(last_good.0, "c1", "last_good should be c1");
                assert_eq!(first_bad.0, "c2", "first_bad should be c2");
            }
            other => panic!("expected FirstBad outcome, got: {:?}", other),
        }

        // Verify schema_version is set on the report (Task 13.9)
        assert_eq!(
            result.report.schema_version, "0.1.0",
            "schema_version should be 0.1.0 on resumed run"
        );

        // Verify the run handle indicates resumed
        assert!(result.run.resumed, "run should be marked as resumed");

        // Verify sequence indices from cached observations are preserved (Task 13.7)
        let obs_c0 = result
            .report
            .observations
            .iter()
            .find(|o| o.commit.0 == "c0")
            .expect("c0 must be in observations");
        assert_eq!(
            obs_c0.sequence_index, 0,
            "cached c0 sequence_index must be preserved as 0"
        );
        let obs_c4 = result
            .report
            .observations
            .iter()
            .find(|o| o.commit.0 == "c4")
            .expect("c4 must be in observations");
        assert_eq!(
            obs_c4.sequence_index, 1,
            "cached c4 sequence_index must be preserved as 1"
        );
        let obs_c2 = result
            .report
            .observations
            .iter()
            .find(|o| o.commit.0 == "c2")
            .expect("c2 must be in observations");
        assert_eq!(
            obs_c2.sequence_index, 2,
            "cached c2 sequence_index must be preserved as 2"
        );

        // Verify newly probed commits get sequence indices > max cached (2)
        let obs_c1 = result
            .report
            .observations
            .iter()
            .find(|o| o.commit.0 == "c1")
            .expect("c1 must be in observations");
        assert!(
            obs_c1.sequence_index > 2,
            "newly probed c1 sequence_index ({}) must be > max cached index (2)",
            obs_c1.sequence_index
        );
    }

    // ── Integration tests: boundary validation with mock ports ──────
    // Validates: Requirements 10.1, 10.2, 10.3, 10.4, 10.5

    #[test]
    fn integration_good_boundary_fail_yields_invalid_boundary() {
        let sequence = make_sequence(5);
        let history = MockHistory {
            sequence: sequence.clone(),
        };
        let checkout = MockCheckout;

        let mut overrides = std::collections::HashMap::new();
        overrides.insert("c0".to_string(), ObservationClass::Fail); // mismatch
        overrides.insert("c4".to_string(), ObservationClass::Fail);

        let probe = ConfigurableMockProbe {
            overrides,
            default_class: ObservationClass::Fail,
        };
        let store = MockRunStore;

        let app = FaultlineApp::new(&history, &checkout, &probe, &store);
        let request = make_request_for_sequence(5, 20);

        match app.localize(request) {
            Err(FaultlineError::InvalidBoundary(msg)) => {
                assert!(
                    msg.contains("known-good"),
                    "error should mention known-good, got: {msg}"
                );
                assert!(
                    msg.contains("Fail") && msg.contains("Pass"),
                    "error should mention expected Pass and actual Fail, got: {msg}"
                );
            }
            other => panic!("expected InvalidBoundary error, got: {other:?}"),
        }
    }

    #[test]
    fn integration_bad_boundary_pass_yields_invalid_boundary() {
        let sequence = make_sequence(5);
        let history = MockHistory {
            sequence: sequence.clone(),
        };
        let checkout = MockCheckout;

        let mut overrides = std::collections::HashMap::new();
        overrides.insert("c0".to_string(), ObservationClass::Pass);
        overrides.insert("c4".to_string(), ObservationClass::Pass); // mismatch

        let probe = ConfigurableMockProbe {
            overrides,
            default_class: ObservationClass::Pass,
        };
        let store = MockRunStore;

        let app = FaultlineApp::new(&history, &checkout, &probe, &store);
        let request = make_request_for_sequence(5, 20);

        match app.localize(request) {
            Err(FaultlineError::InvalidBoundary(msg)) => {
                assert!(
                    msg.contains("known-bad"),
                    "error should mention known-bad, got: {msg}"
                );
                assert!(
                    msg.contains("Pass") && msg.contains("Fail"),
                    "error should mention expected Fail and actual Pass, got: {msg}"
                );
            }
            other => panic!("expected InvalidBoundary error, got: {other:?}"),
        }
    }

    // Validates: Requirement 10.5
    //
    // Pre-caches boundary observations (c0=Pass, c{n-1}=Fail) via CachedRunStore.
    // Uses TrackingProbe to verify that boundaries are NOT re-probed when cached.
    // The localization should complete successfully using the cached boundary data.
    #[test]
    fn integration_cached_boundary_observations_reused_no_reprobe() {
        // Sequence: c0, c1, c2, c3, c4 (5 commits)
        let sequence = make_sequence(5);
        let history = MockHistory {
            sequence: sequence.clone(),
        };
        let checkout = MockCheckout;

        // Pre-cache ONLY the boundary observations
        let cached_observations = vec![
            ProbeObservation {
                commit: CommitId("c0".into()),
                class: ObservationClass::Pass,
                kind: ProbeKind::Test,
                exit_code: Some(0),
                timed_out: false,
                duration_ms: 10,
                stdout: String::new(),
                stderr: String::new(),
                sequence_index: 0,
                signal_number: None,
                probe_command: String::new(),
                working_dir: String::new(),
            },
            ProbeObservation {
                commit: CommitId("c4".into()),
                class: ObservationClass::Fail,
                kind: ProbeKind::Test,
                exit_code: Some(1),
                timed_out: false,
                duration_ms: 10,
                stdout: String::new(),
                stderr: String::new(),
                sequence_index: 1,
                signal_number: None,
                probe_command: String::new(),
                working_dir: String::new(),
            },
        ];

        // Interior commits: c1=Pass, c2=Fail → FirstBad at c1→c2
        let mut overrides = std::collections::HashMap::new();
        overrides.insert("c1".into(), ObservationClass::Pass);
        overrides.insert("c2".into(), ObservationClass::Fail);
        overrides.insert("c3".into(), ObservationClass::Fail);

        let probe = TrackingProbe {
            overrides,
            default_class: ObservationClass::Fail,
            probed: std::cell::RefCell::new(Vec::new()),
        };

        let store = CachedRunStore {
            cached_observations,
        };

        let app = FaultlineApp::new(&history, &checkout, &probe, &store);
        let request = make_request_for_sequence(5, 20);

        let result = app.localize(request).expect("localize should succeed");

        // Verify boundary commits were NOT re-probed
        let probed_commits = probe.probed.borrow();
        assert!(
            !probed_commits.contains(&"c0".to_string()),
            "c0 was cached (Pass boundary) and should not have been re-probed"
        );
        assert!(
            !probed_commits.contains(&"c4".to_string()),
            "c4 was cached (Fail boundary) and should not have been re-probed"
        );

        // Verify interior commits WERE probed (they were not cached)
        assert!(
            probed_commits.contains(&"c2".to_string()),
            "c2 was not cached and should have been probed"
        );

        // Verify localization completed successfully with correct outcome
        match &result.report.outcome {
            faultline_types::LocalizationOutcome::FirstBad {
                last_good,
                first_bad,
                ..
            } => {
                assert_eq!(last_good.0, "c1", "last_good should be c1");
                assert_eq!(first_bad.0, "c2", "first_bad should be c2");
            }
            other => panic!("expected FirstBad outcome, got: {other:?}"),
        }

        // Verify schema_version is set on the report (Task 13.9)
        assert_eq!(
            result.report.schema_version, "0.1.0",
            "schema_version should be 0.1.0"
        );
    }

    // Feature: v01-release-train, Property 9: Probe Count Respects Max Probes
    // **Validates: Requirements 3.8**
    proptest! {
        #![proptest_config(ProptestConfig { cases: 100, .. ProptestConfig::default() })]

        #[test]
        fn prop_probe_count_respects_max_probes(max_probes in 1usize..=10) {
            let num_commits = 20;
            let sequence = make_sequence(num_commits);
            let history = MockHistory { sequence: sequence.clone() };
            let checkout = MockCheckout;
            let probe = MockProbe {
                call_count: Cell::new(0),
            };
            let store = MockRunStore;

            let app = FaultlineApp::new(&history, &checkout, &probe, &store);
            let request = make_request(max_probes);

            let _result = app.localize(request);

            let total_probes = probe.call_count.get();
            // The +2 accounts for the two boundary validation probes
            // (good boundary + bad boundary) which are separate from the
            // narrowing loop's max_probes budget.
            prop_assert!(
                total_probes <= max_probes + 2,
                "total probe executions ({}) exceeded max_probes ({}) + 2 boundary probes = {}",
                total_probes,
                max_probes,
                max_probes + 2,
            );
        }
    }

    // Feature: v01-release-train, Property 20: Boundary Validation Rejects Mismatched Classes
    // **Validates: Requirements 10.1, 10.2, 10.3, 10.4**
    proptest! {
        #![proptest_config(ProptestConfig { cases: 100, .. ProptestConfig::default() })]

        #[test]
        fn prop_good_boundary_fail_yields_invalid_boundary(n in 3usize..=20) {
            // Good boundary (c0) returns Fail instead of Pass → InvalidBoundary
            let sequence = make_sequence(n);
            let history = MockHistory { sequence };
            let checkout = MockCheckout;

            let mut overrides = std::collections::HashMap::new();
            // Good boundary returns Fail (mismatch)
            overrides.insert("c0".to_string(), ObservationClass::Fail);
            // Bad boundary returns Fail (correct)
            overrides.insert(format!("c{}", n - 1), ObservationClass::Fail);

            let probe = ConfigurableMockProbe {
                overrides,
                default_class: ObservationClass::Fail,
            };
            let store = MockRunStore;

            let app = FaultlineApp::new(&history, &checkout, &probe, &store);
            let request = make_request_for_sequence(n, 10);

            let result = app.localize(request);
            match result {
                Err(FaultlineError::InvalidBoundary(msg)) => {
                    prop_assert!(
                        msg.contains("known-good"),
                        "error message should mention known-good boundary, got: {}", msg
                    );
                    prop_assert!(
                        msg.contains("Fail") && msg.contains("Pass"),
                        "error message should mention expected (Pass) and actual (Fail) classes, got: {}", msg
                    );
                }
                other => {
                    prop_assert!(false, "expected InvalidBoundary error, got: {:?}", other);
                }
            }
        }

        #[test]
        fn prop_bad_boundary_pass_yields_invalid_boundary(n in 3usize..=20) {
            // Bad boundary (c{n-1}) returns Pass instead of Fail → InvalidBoundary
            let sequence = make_sequence(n);
            let history = MockHistory { sequence };
            let checkout = MockCheckout;

            let mut overrides = std::collections::HashMap::new();
            // Good boundary returns Pass (correct)
            overrides.insert("c0".to_string(), ObservationClass::Pass);
            // Bad boundary returns Pass (mismatch)
            overrides.insert(format!("c{}", n - 1), ObservationClass::Pass);

            let probe = ConfigurableMockProbe {
                overrides,
                default_class: ObservationClass::Pass,
            };
            let store = MockRunStore;

            let app = FaultlineApp::new(&history, &checkout, &probe, &store);
            let request = make_request_for_sequence(n, 10);

            let result = app.localize(request);
            match result {
                Err(FaultlineError::InvalidBoundary(msg)) => {
                    prop_assert!(
                        msg.contains("known-bad"),
                        "error message should mention known-bad boundary, got: {}", msg
                    );
                    prop_assert!(
                        msg.contains("Pass") && msg.contains("Fail"),
                        "error message should mention expected (Fail) and actual (Pass) classes, got: {}", msg
                    );
                }
                other => {
                    prop_assert!(false, "expected InvalidBoundary error, got: {:?}", other);
                }
            }
        }
    }

    // ── Integration test: full localization loop with mock ports ─────
    // Validates: Requirements 3.1, 3.2, 3.8, 3.9
    //
    // Sets up 10 commits (c0–c9) with a known transition at c5:
    //   c0–c4 = Pass, c5–c9 = Fail
    // Wires FaultlineApp with mock ports (fresh run, no cache).
    // Verifies:
    //   1. FirstBad outcome with last_good=c4, first_bad=c5
    //   2. Report contains all expected fields
    //   3. Observation count is reasonable (≤ log2(10) + 2 boundary probes)
    #[test]
    fn integration_full_localization_loop_with_mock_ports() {
        let n = 10;
        let sequence = make_sequence(n);
        let history = MockHistory {
            sequence: sequence.clone(),
        };
        let checkout = MockCheckout;

        // c0–c4 = Pass, c5–c9 = Fail
        let mut overrides = std::collections::HashMap::new();
        for i in 0..5 {
            overrides.insert(format!("c{i}"), ObservationClass::Pass);
        }
        for i in 5..10 {
            overrides.insert(format!("c{i}"), ObservationClass::Fail);
        }

        let probe = TrackingProbe {
            overrides,
            default_class: ObservationClass::Fail,
            probed: std::cell::RefCell::new(Vec::new()),
        };
        let store = MockRunStore;

        let app = FaultlineApp::new(&history, &checkout, &probe, &store);
        let request = make_request_for_sequence(n, 30);

        let result = app.localize(request).expect("localize should succeed");
        let report = &result.report;

        // 1. Verify FirstBad outcome with correct boundary pair
        match &report.outcome {
            faultline_types::LocalizationOutcome::FirstBad {
                last_good,
                first_bad,
                confidence,
            } => {
                assert_eq!(last_good.0, "c4", "last_good should be c4");
                assert_eq!(first_bad.0, "c5", "first_bad should be c5");
                // Req 3.9: both boundaries backed by direct observations
                let has_pass = report
                    .observations
                    .iter()
                    .any(|o| o.commit.0 == "c4" && o.class == ObservationClass::Pass);
                let has_fail = report
                    .observations
                    .iter()
                    .any(|o| o.commit.0 == "c5" && o.class == ObservationClass::Fail);
                assert!(has_pass, "last_good c4 must have a direct Pass observation");
                assert!(has_fail, "first_bad c5 must have a direct Fail observation");
                assert!(
                    confidence.score > 0,
                    "confidence score should be positive for FirstBad"
                );
            }
            other => panic!("expected FirstBad outcome, got: {:?}", other),
        }

        // 2. Verify report contains all expected fields
        assert_eq!(report.run_id, "mock-run", "run_id should match mock");
        assert!(
            report.created_at_epoch_seconds > 0,
            "created_at should be set"
        );
        assert_eq!(
            report.schema_version, "0.1.0",
            "schema_version should be 0.1.0"
        );
        assert_eq!(
            report.sequence.revisions.len(),
            n,
            "sequence should have {n} commits"
        );
        assert!(
            !report.observations.is_empty(),
            "observations should not be empty"
        );

        // 3. Verify observation count is reasonable: ≤ log2(n) + 2 boundary probes
        //    For n=10: log2(10) ≈ 3.32, ceil → 4, plus 2 boundary = 6
        //    We allow a small margin: log2(n) + 2 boundary probes + 1 extra
        let max_expected = (n as f64).log2().ceil() as usize + 2 + 1;
        let probed_commits = probe.probed.borrow();
        assert!(
            probed_commits.len() <= max_expected,
            "probe count ({}) should be ≤ log2({n}) + 2 + 1 = {max_expected}",
            probed_commits.len(),
        );

        // Also verify total observations in the report are reasonable
        assert!(
            report.observations.len() <= max_expected,
            "observation count ({}) should be ≤ {max_expected}",
            report.observations.len(),
        );
    }
}
