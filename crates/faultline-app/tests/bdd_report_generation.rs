//! BDD-style integration tests for end-to-end report generation via `FaultlineApp`.
//!
//! Each test follows Given / When / Then structure and exercises the full
//! `localize` pipeline using mock implementations of the four port traits.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;

use faultline_app::FaultlineApp;
use faultline_codes::{AmbiguityReason, ObservationClass, ProbeKind};
use faultline_ports::{CheckoutPort, HistoryPort, ProbePort, RunStorePort};
use faultline_types::{
    AnalysisReport, AnalysisRequest, ChangeStatus, CheckedOutRevision, CommitId, FlakePolicy,
    HistoryMode, PathChange, ProbeObservation, ProbeSpec, RevisionSequence, RevisionSpec,
    RunHandle, SearchPolicy, ShellKind,
};

// ---------------------------------------------------------------------------
// Mock implementations
// ---------------------------------------------------------------------------

/// History port that returns a pre-configured revision sequence and optionally
/// changed paths for a specific commit boundary.
struct MockHistory {
    sequence: RevisionSequence,
    changed_paths: Vec<PathChange>,
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
        Ok(self.changed_paths.clone())
    }

    fn codeowners_for_paths(
        &self,
        _paths: &[String],
    ) -> faultline_types::Result<HashMap<String, Option<String>>> {
        Ok(HashMap::new())
    }

    fn blame_frequency(
        &self,
        _paths: &[String],
    ) -> faultline_types::Result<HashMap<String, Option<String>>> {
        Ok(HashMap::new())
    }
}

/// Checkout port that returns a synthetic checkout path for any commit.
struct MockCheckout;

impl CheckoutPort for MockCheckout {
    fn checkout_revision(&self, commit: &CommitId) -> faultline_types::Result<CheckedOutRevision> {
        Ok(CheckedOutRevision {
            commit: commit.clone(),
            path: PathBuf::from("/tmp/mock-checkout"),
        })
    }

    fn cleanup_checkout(&self, _checkout: &CheckedOutRevision) -> faultline_types::Result<()> {
        Ok(())
    }
}

/// Probe port that looks up the observation class for a commit in a static map.
/// Falls back to `default_class` when the commit is not in the overrides.
struct StaticMockProbe {
    overrides: HashMap<String, ObservationClass>,
    default_class: ObservationClass,
}

impl ProbePort for StaticMockProbe {
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
            flake_signal: None,
        })
    }
}

/// Probe port that returns different classes on successive calls for the same
/// commit.  Each commit has a queue of classes; once exhausted it falls back to
/// `default_class`.  Used to simulate flaky predicates with retries.
struct SequentialMockProbe {
    queues: RefCell<HashMap<String, Vec<ObservationClass>>>,
    default_class: ObservationClass,
}

impl ProbePort for SequentialMockProbe {
    fn run(
        &self,
        checkout: &CheckedOutRevision,
        _probe: &ProbeSpec,
    ) -> faultline_types::Result<ProbeObservation> {
        let class = {
            let mut queues = self.queues.borrow_mut();
            if let Some(queue) = queues.get_mut(&checkout.commit.0) {
                if !queue.is_empty() {
                    queue.remove(0)
                } else {
                    self.default_class
                }
            } else {
                self.default_class
            }
        };

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
            flake_signal: None,
        })
    }
}

/// Minimal run-store that keeps everything in memory (no filesystem).
struct MockRunStore;

impl RunStorePort for MockRunStore {
    fn prepare_run(&self, _request: &AnalysisRequest) -> faultline_types::Result<RunHandle> {
        Ok(RunHandle {
            id: "bdd-run".to_string(),
            root: PathBuf::from("/tmp/bdd-run"),
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

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_sequence(n: usize) -> RevisionSequence {
    RevisionSequence {
        revisions: (0..n).map(|i| CommitId(format!("c{i}"))).collect(),
    }
}

fn make_request(n: usize, max_probes: usize) -> AnalysisRequest {
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
        policy: SearchPolicy {
            max_probes,
            flake_policy: FlakePolicy::default(),
        },
    }
}

fn make_request_with_flake(
    n: usize,
    max_probes: usize,
    retries: u32,
    stability_threshold: f64,
) -> AnalysisRequest {
    let mut req = make_request(n, max_probes);
    req.policy.flake_policy = FlakePolicy {
        retries,
        stability_threshold,
    };
    req
}

// ---------------------------------------------------------------------------
// Scenario 1: Clean regression produces FirstBad report
// ---------------------------------------------------------------------------

#[test]
fn clean_regression_produces_first_bad_report() {
    // Given: 5 commits where c0..c3 pass and c4 fails (fault introduced at c4)
    let history = MockHistory {
        sequence: make_sequence(5),
        changed_paths: Vec::new(),
    };

    let overrides: HashMap<String, ObservationClass> = [
        ("c0".into(), ObservationClass::Pass),
        ("c1".into(), ObservationClass::Pass),
        ("c2".into(), ObservationClass::Pass),
        ("c3".into(), ObservationClass::Pass),
        ("c4".into(), ObservationClass::Fail),
    ]
    .into_iter()
    .collect();

    let probe = StaticMockProbe {
        overrides,
        default_class: ObservationClass::Fail,
    };
    let checkout = MockCheckout;
    let store = MockRunStore;

    let app = FaultlineApp::new(&history, &checkout, &probe, &store);
    let request = make_request(5, 64);

    // When: localize is called
    let result = app.localize(request);

    // Then: outcome is FirstBad with the correct boundary
    let run = result.expect("localize should succeed");
    let report = &run.report;

    match &report.outcome {
        faultline_types::LocalizationOutcome::FirstBad {
            last_good,
            first_bad,
            ..
        } => {
            assert_eq!(last_good.0, "c3", "last good commit should be c3");
            assert_eq!(first_bad.0, "c4", "first bad commit should be c4");
        }
        other => panic!("expected FirstBad, got: {other:?}"),
    }

    // Observations should include at least the 2 boundary probes
    assert!(
        report.observations.len() >= 2,
        "expected at least 2 observations, got {}",
        report.observations.len()
    );

    // Report metadata
    assert_eq!(report.run_id, "bdd-run");
    assert!(
        !report.schema_version.is_empty(),
        "schema_version should be set"
    );
}

// ---------------------------------------------------------------------------
// Scenario 2: Flaky predicate produces SuspectWindow
// ---------------------------------------------------------------------------

#[test]
fn flaky_predicate_produces_suspect_window() {
    // Given: 5 commits with retries=2, stability_threshold=0.6
    //
    //   The boundaries are stable: c0 always passes, c4 always fails.
    //   The middle commit c2 alternates between Skip and Fail, which causes
    //   the majority class to be Skip.  Skip between pass/fail boundaries
    //   produces SuspectWindow with SkippedRevision.
    //
    //   With retries=2 each commit is probed 3 times:
    //     c0: [Pass, Pass, Pass]  -> Pass  (boundary good)
    //     c4: [Fail, Fail, Fail]  -> Fail  (boundary bad)
    //     c2: [Skip, Skip, Fail]  -> Skip  (majority, 2/3)
    //     c3: [Fail, Fail, Fail]  -> Fail
    //     c1: [Pass, Pass, Pass]  -> Pass
    //
    //   After bisection converges:  pass boundary c1, fail boundary c3,
    //   c2=Skip between them -> SuspectWindow { SkippedRevision }.

    let history = MockHistory {
        sequence: make_sequence(5),
        changed_paths: Vec::new(),
    };

    let mut queues: HashMap<String, Vec<ObservationClass>> = HashMap::new();
    // Boundary: c0 always pass, c4 always fail (3 calls each for retries)
    queues.insert(
        "c0".into(),
        vec![
            ObservationClass::Pass,
            ObservationClass::Pass,
            ObservationClass::Pass,
        ],
    );
    queues.insert(
        "c4".into(),
        vec![
            ObservationClass::Fail,
            ObservationClass::Fail,
            ObservationClass::Fail,
        ],
    );
    // c2 is the first bisection target (midpoint of 0..4): returns Skip majority
    queues.insert(
        "c2".into(),
        vec![
            ObservationClass::Skip,
            ObservationClass::Skip,
            ObservationClass::Fail,
        ],
    );
    // c3 fails cleanly
    queues.insert(
        "c3".into(),
        vec![
            ObservationClass::Fail,
            ObservationClass::Fail,
            ObservationClass::Fail,
        ],
    );
    // c1 passes cleanly
    queues.insert(
        "c1".into(),
        vec![
            ObservationClass::Pass,
            ObservationClass::Pass,
            ObservationClass::Pass,
        ],
    );

    let probe = SequentialMockProbe {
        queues: RefCell::new(queues),
        default_class: ObservationClass::Fail,
    };
    let checkout = MockCheckout;
    let store = MockRunStore;

    let app = FaultlineApp::new(&history, &checkout, &probe, &store);
    let request = make_request_with_flake(5, 64, 2, 0.6);

    // When: localize is called
    let result = app.localize(request);

    // Then: outcome is SuspectWindow with a skip/flake-related reason
    let run = result.expect("localize should succeed");
    let report = &run.report;

    match &report.outcome {
        faultline_types::LocalizationOutcome::SuspectWindow { reasons, .. } => {
            assert!(
                reasons
                    .iter()
                    .any(|r| matches!(r, AmbiguityReason::SkippedRevision)),
                "expected SkippedRevision in reasons, got: {reasons:?}"
            );
        }
        other => panic!("expected SuspectWindow, got: {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Scenario 3: All-skip produces Inconclusive
// ---------------------------------------------------------------------------

#[test]
fn all_skip_middle_produces_inconclusive() {
    // Given: 5 commits where good/bad boundaries are valid but every middle
    //        commit returns Skip.  The probe budget is tight (max_probes=3):
    //        2 boundary probes + 1 bisection probe exhausts the budget,
    //        leaving unobserved commits between the boundaries.
    //
    //   c0 = Pass (boundary good)
    //   c4 = Fail (boundary bad)
    //   c1, c2, c3 = Skip
    //
    //   After boundaries + 1 bisection probe: 3 observations total,
    //   max_probes reached.  Unobserved commits remain -> Inconclusive.

    let history = MockHistory {
        sequence: make_sequence(5),
        changed_paths: Vec::new(),
    };

    let overrides: HashMap<String, ObservationClass> = [
        ("c0".into(), ObservationClass::Pass),
        ("c4".into(), ObservationClass::Fail),
        ("c1".into(), ObservationClass::Skip),
        ("c2".into(), ObservationClass::Skip),
        ("c3".into(), ObservationClass::Skip),
    ]
    .into_iter()
    .collect();

    let probe = StaticMockProbe {
        overrides,
        default_class: ObservationClass::Skip,
    };
    let checkout = MockCheckout;
    let store = MockRunStore;

    let app = FaultlineApp::new(&history, &checkout, &probe, &store);
    let request = make_request(5, 3);

    // When: localize is called
    let result = app.localize(request);

    // Then: outcome is Inconclusive (budget exhausted with unobserved commits)
    let run = result.expect("localize should succeed");
    let report = &run.report;

    match &report.outcome {
        faultline_types::LocalizationOutcome::Inconclusive { reasons } => {
            assert!(
                !reasons.is_empty(),
                "Inconclusive should carry at least one reason"
            );
        }
        other => panic!("expected Inconclusive, got: {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Scenario 4: Report contains suspect surface when changed_paths available
// ---------------------------------------------------------------------------

#[test]
fn report_contains_suspect_surface_when_changed_paths_available() {
    // Given: 5 commits with a clean regression at c4, and the history port
    //        reports changed paths (source file modifications) between the
    //        boundary commits.
    let changed_paths = vec![
        PathChange {
            status: ChangeStatus::Modified,
            path: "src/main.rs".into(),
        },
        PathChange {
            status: ChangeStatus::Added,
            path: "src/bug.rs".into(),
        },
    ];

    let history = MockHistory {
        sequence: make_sequence(5),
        changed_paths,
    };

    let overrides: HashMap<String, ObservationClass> = [
        ("c0".into(), ObservationClass::Pass),
        ("c1".into(), ObservationClass::Pass),
        ("c2".into(), ObservationClass::Pass),
        ("c3".into(), ObservationClass::Pass),
        ("c4".into(), ObservationClass::Fail),
    ]
    .into_iter()
    .collect();

    let probe = StaticMockProbe {
        overrides,
        default_class: ObservationClass::Fail,
    };
    let checkout = MockCheckout;
    let store = MockRunStore;

    let app = FaultlineApp::new(&history, &checkout, &probe, &store);
    let request = make_request(5, 64);

    // When: localize is called
    let result = app.localize(request);

    // Then: report.suspect_surface is non-empty and contains the changed paths
    let run = result.expect("localize should succeed");
    let report = &run.report;

    assert!(
        !report.suspect_surface.is_empty(),
        "suspect_surface should be non-empty when changed_paths are available"
    );

    let suspect_paths: Vec<&str> = report
        .suspect_surface
        .iter()
        .map(|e| e.path.as_str())
        .collect();
    assert!(
        suspect_paths.contains(&"src/main.rs"),
        "expected src/main.rs in suspect surface, got: {suspect_paths:?}"
    );
    assert!(
        suspect_paths.contains(&"src/bug.rs"),
        "expected src/bug.rs in suspect surface, got: {suspect_paths:?}"
    );
}
