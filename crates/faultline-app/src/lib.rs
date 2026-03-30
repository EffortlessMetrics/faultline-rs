use faultline_codes::ObservationClass;
use faultline_localization::LocalizationSession;
use faultline_ports::{CheckoutPort, HistoryPort, ProbePort, RunStorePort};
use faultline_surface::SurfaceAnalyzer;
use faultline_types::{
    now_epoch_seconds, AnalysisReport, AnalysisRequest, FaultlineError, Result, RunHandle,
};

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
        let run = self.store.prepare_run(&request)?;
        let sequence = self
            .history
            .linearize(&request.good, &request.bad, request.history_mode)?;

        let mut session = LocalizationSession::new(sequence.clone(), request.policy.clone())?;
        for observation in self.store.load_observations(&run)? {
            session.record(observation)?;
        }

        self.ensure_boundary(
            &run,
            &request,
            &mut session,
            0,
            ObservationClass::Pass,
            "known-good",
        )?;
        self.ensure_boundary(
            &run,
            &request,
            &mut session,
            sequence.len() - 1,
            ObservationClass::Fail,
            "known-bad",
        )?;

        let mut probe_count = 0usize;
        let max_probes = session.max_probes();
        while probe_count < max_probes {
            let Some(commit) = session.next_probe() else {
                break;
            };
            let observation = self.probe_commit(&request, &commit)?;
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

    fn ensure_boundary(
        &self,
        run: &RunHandle,
        request: &AnalysisRequest,
        session: &mut LocalizationSession,
        index: usize,
        expected: ObservationClass,
        label: &str,
    ) -> Result<()> {
        let commit = session
            .sequence()
            .revisions
            .get(index)
            .ok_or_else(|| FaultlineError::Domain("missing boundary index".to_string()))?
            .clone();

        if !session.has_observation(&commit) {
            let observation = self.probe_commit(request, &commit)?;
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
                timeout_seconds: 60,
            },
            policy: SearchPolicy {
                max_probes,
                edge_refine_threshold: 6,
            },
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
                timeout_seconds: 60,
            },
            policy: SearchPolicy {
                max_probes,
                edge_refine_threshold: 6,
            },
        }
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
}
