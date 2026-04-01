//! BDD-style integration tests for the `compare_runs` workflow.
//!
//! Each test follows Given / When / Then structure exercising
//! `faultline_types::compare_runs` with realistic `AnalysisReport` instances.

use faultline_codes::{AmbiguityReason, ObservationClass, ProbeKind};
use faultline_types::{
    AnalysisReport, AnalysisRequest, ChangeStatus, CommitId, Confidence, FlakePolicy, HistoryMode,
    LocalizationOutcome, PathChange, ProbeObservation, ProbeSpec, RevisionSequence, RevisionSpec,
    SearchPolicy, SubsystemBucket, SurfaceSummary, SuspectEntry, compare_runs,
};
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a minimal `ProbeSpec` used across all test reports.
fn test_probe() -> ProbeSpec {
    ProbeSpec::Exec {
        kind: ProbeKind::Test,
        program: "cargo".into(),
        args: vec!["test".into()],
        env: vec![],
        timeout_seconds: 300,
    }
}

/// Build a minimal `AnalysisRequest` shared by every report.
fn test_request() -> AnalysisRequest {
    AnalysisRequest {
        repo_root: PathBuf::from("/tmp/repo"),
        good: RevisionSpec("good0".into()),
        bad: RevisionSpec("bad0".into()),
        history_mode: HistoryMode::AncestryPath,
        probe: test_probe(),
        policy: SearchPolicy {
            max_probes: 64,
            flake_policy: FlakePolicy::default(),
        },
    }
}

/// Build an `AnalysisReport` with the given parameters. Everything not
/// specified is set to a sensible default so each scenario only has to
/// override what matters.
fn make_report(
    run_id: &str,
    sequence: Vec<&str>,
    observations: Vec<(&str, ObservationClass)>,
    outcome: LocalizationOutcome,
    suspect_paths: Vec<&str>,
) -> AnalysisReport {
    let revisions: Vec<CommitId> = sequence.iter().map(|s| CommitId(s.to_string())).collect();

    let obs: Vec<ProbeObservation> = observations
        .into_iter()
        .enumerate()
        .map(|(i, (commit, class))| ProbeObservation {
            commit: CommitId(commit.to_string()),
            class,
            kind: ProbeKind::Test,
            exit_code: Some(if class == ObservationClass::Pass {
                0
            } else {
                1
            }),
            timed_out: false,
            duration_ms: 100,
            stdout: String::new(),
            stderr: String::new(),
            sequence_index: i as u64,
            signal_number: None,
            probe_command: String::new(),
            working_dir: String::new(),
            flake_signal: None,
        })
        .collect();

    let suspect_surface: Vec<SuspectEntry> = suspect_paths
        .into_iter()
        .map(|p| SuspectEntry {
            path: p.to_string(),
            priority_score: 100,
            surface_kind: "source".into(),
            change_status: ChangeStatus::Modified,
            is_execution_surface: false,
            owner_hint: None,
        })
        .collect();

    AnalysisReport {
        schema_version: "0.2.0".into(),
        run_id: run_id.to_string(),
        created_at_epoch_seconds: 1_700_000_000,
        request: test_request(),
        sequence: RevisionSequence { revisions },
        observations: obs,
        outcome,
        changed_paths: vec![PathChange {
            status: ChangeStatus::Modified,
            path: "src/lib.rs".into(),
        }],
        surface: SurfaceSummary {
            total_changes: 1,
            buckets: vec![SubsystemBucket {
                name: "src".into(),
                change_count: 1,
                paths: vec!["src/lib.rs".into()],
                surface_kinds: vec!["source".into()],
            }],
            execution_surfaces: vec![],
        },
        suspect_surface,
        reproduction_capsules: vec![],
    }
}

// ---------------------------------------------------------------------------
// Scenario 1: Identical reports show no changes
// ---------------------------------------------------------------------------

#[test]
fn identical_reports_show_no_changes() {
    // Given: two identical AnalysisReports
    let commits = vec!["c0", "c1", "c2", "c3"];
    let observations = vec![
        ("c0", ObservationClass::Pass),
        ("c1", ObservationClass::Pass),
        ("c2", ObservationClass::Fail),
        ("c3", ObservationClass::Fail),
    ];
    let outcome = LocalizationOutcome::FirstBad {
        last_good: CommitId("c1".into()),
        first_bad: CommitId("c2".into()),
        confidence: Confidence::high(),
    };

    let left = make_report(
        "run-A",
        commits.clone(),
        observations.clone(),
        outcome.clone(),
        vec!["a.rs"],
    );
    let right = make_report(
        "run-B",
        commits,
        observations.clone(),
        outcome,
        vec!["a.rs"],
    );

    // When
    let cmp = compare_runs(&left, &right);

    // Then
    assert!(
        !cmp.outcome_changed,
        "identical outcomes should not be flagged as changed"
    );
    assert_eq!(
        cmp.confidence_delta, 0,
        "same confidence should yield zero delta"
    );
    assert_eq!(
        cmp.window_width_delta, 0,
        "same window should yield zero width delta"
    );
    assert_eq!(
        cmp.probes_reused,
        observations.len(),
        "all observations should be reused when they match exactly"
    );
    assert!(cmp.suspect_paths_added.is_empty());
    assert!(cmp.suspect_paths_removed.is_empty());
    assert!(cmp.ambiguity_reasons_added.is_empty());
    assert!(cmp.ambiguity_reasons_removed.is_empty());
}

// ---------------------------------------------------------------------------
// Scenario 2: Improved run shows positive confidence delta
// ---------------------------------------------------------------------------

#[test]
fn improved_run_shows_positive_confidence_delta() {
    // Given: left report with SuspectWindow (confidence=50)
    let commits = vec!["c0", "c1", "c2", "c3"];
    let left = make_report(
        "run-left",
        commits.clone(),
        vec![
            ("c0", ObservationClass::Pass),
            ("c3", ObservationClass::Fail),
        ],
        LocalizationOutcome::SuspectWindow {
            lower_bound_exclusive: CommitId("c0".into()),
            upper_bound_inclusive: CommitId("c3".into()),
            confidence: Confidence {
                score: 50,
                label: "medium-low".into(),
            },
            reasons: vec![AmbiguityReason::NeedsMoreProbes],
        },
        vec![],
    );

    // Given: right report with FirstBad (confidence=100)
    let right = make_report(
        "run-right",
        commits,
        vec![
            ("c0", ObservationClass::Pass),
            ("c1", ObservationClass::Pass),
            ("c2", ObservationClass::Fail),
            ("c3", ObservationClass::Fail),
        ],
        LocalizationOutcome::FirstBad {
            last_good: CommitId("c1".into()),
            first_bad: CommitId("c2".into()),
            confidence: Confidence {
                score: 100,
                label: "definitive".into(),
            },
        },
        vec![],
    );

    // When
    let cmp = compare_runs(&left, &right);

    // Then
    assert!(
        cmp.outcome_changed,
        "SuspectWindow -> FirstBad should be an outcome change"
    );
    assert_eq!(cmp.confidence_delta, 50, "100 - 50 = 50 positive delta");
}

// ---------------------------------------------------------------------------
// Scenario 3: Narrowed window shows negative width delta
// ---------------------------------------------------------------------------

#[test]
fn narrowed_window_shows_negative_width_delta() {
    // Given: left report with window width 10 (indices 0..10 in a sequence of 11)
    let wide_commits: Vec<&str> = (0..=10)
        .map(|i| match i {
            0 => "w0",
            1 => "w1",
            2 => "w2",
            3 => "w3",
            4 => "w4",
            5 => "w5",
            6 => "w6",
            7 => "w7",
            8 => "w8",
            9 => "w9",
            10 => "w10",
            _ => unreachable!(),
        })
        .collect();

    let left = make_report(
        "run-wide",
        wide_commits.clone(),
        vec![
            ("w0", ObservationClass::Pass),
            ("w10", ObservationClass::Fail),
        ],
        LocalizationOutcome::SuspectWindow {
            lower_bound_exclusive: CommitId("w0".into()),
            upper_bound_inclusive: CommitId("w10".into()),
            confidence: Confidence::low(),
            reasons: vec![AmbiguityReason::NeedsMoreProbes],
        },
        vec![],
    );

    // Given: right report with window width 3 (indices 4..7 in the same sequence)
    let right = make_report(
        "run-narrow",
        wide_commits,
        vec![
            ("w0", ObservationClass::Pass),
            ("w4", ObservationClass::Pass),
            ("w7", ObservationClass::Fail),
            ("w10", ObservationClass::Fail),
        ],
        LocalizationOutcome::SuspectWindow {
            lower_bound_exclusive: CommitId("w4".into()),
            upper_bound_inclusive: CommitId("w7".into()),
            confidence: Confidence::medium(),
            reasons: vec![AmbiguityReason::NeedsMoreProbes],
        },
        vec![],
    );

    // When
    let cmp = compare_runs(&left, &right);

    // Then: right width (3) - left width (10) = -7, which is negative
    assert!(
        cmp.window_width_delta < 0,
        "narrower window should produce a negative delta, got {}",
        cmp.window_width_delta
    );
    assert_eq!(cmp.window_width_delta, -7, "3 - 10 = -7");
}

// ---------------------------------------------------------------------------
// Scenario 4: New suspect paths detected
// ---------------------------------------------------------------------------

#[test]
fn new_suspect_paths_detected() {
    // Given: left has suspect_surface=[a.rs]
    let commits = vec!["c0", "c1"];
    let outcome = LocalizationOutcome::FirstBad {
        last_good: CommitId("c0".into()),
        first_bad: CommitId("c1".into()),
        confidence: Confidence::high(),
    };
    let obs = vec![
        ("c0", ObservationClass::Pass),
        ("c1", ObservationClass::Fail),
    ];

    let left = make_report(
        "run-L",
        commits.clone(),
        obs.clone(),
        outcome.clone(),
        vec!["a.rs"],
    );

    // Given: right has suspect_surface=[a.rs, b.rs, c.rs]
    let right = make_report("run-R", commits, obs, outcome, vec!["a.rs", "b.rs", "c.rs"]);

    // When
    let cmp = compare_runs(&left, &right);

    // Then
    assert_eq!(
        cmp.suspect_paths_added,
        vec!["b.rs".to_string(), "c.rs".to_string()],
        "b.rs and c.rs were added"
    );
    assert!(
        cmp.suspect_paths_removed.is_empty(),
        "no paths were removed"
    );
}

// ---------------------------------------------------------------------------
// Scenario 5: Probes reused when same commits tested
// ---------------------------------------------------------------------------

#[test]
fn probes_reused_when_same_commits_tested() {
    // Given: left has observations for commits [a, b, c]
    let left = make_report(
        "run-1",
        vec!["a", "b", "c", "d"],
        vec![
            ("a", ObservationClass::Pass),
            ("b", ObservationClass::Pass),
            ("c", ObservationClass::Fail),
        ],
        LocalizationOutcome::FirstBad {
            last_good: CommitId("b".into()),
            first_bad: CommitId("c".into()),
            confidence: Confidence::high(),
        },
        vec![],
    );

    // Given: right has observations for commits [b, c, d]
    let right = make_report(
        "run-2",
        vec!["a", "b", "c", "d"],
        vec![
            ("b", ObservationClass::Pass),
            ("c", ObservationClass::Fail),
            ("d", ObservationClass::Fail),
        ],
        LocalizationOutcome::FirstBad {
            last_good: CommitId("b".into()),
            first_bad: CommitId("c".into()),
            confidence: Confidence::high(),
        },
        vec![],
    );

    // When
    let cmp = compare_runs(&left, &right);

    // Then: commits b (Pass) and c (Fail) appear in both with the same class
    assert!(
        cmp.probes_reused >= 2,
        "at least commits b and c should be reused, got {}",
        cmp.probes_reused
    );
}
