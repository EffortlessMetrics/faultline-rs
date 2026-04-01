//! BDD-style integration tests for LocalizationSession binary narrowing engine.
//!
//! Each scenario follows Given / When / Then structure to document the
//! behavioral contract of the narrowing algorithm.

use faultline_codes::{AmbiguityReason, ObservationClass, ProbeKind};
use faultline_localization::LocalizationSession;
use faultline_types::{
    CommitId, Confidence, LocalizationOutcome, ProbeObservation, RevisionSequence, SearchPolicy,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn commit(name: &str) -> CommitId {
    CommitId(name.to_string())
}

fn make_seq(labels: &[&str]) -> RevisionSequence {
    RevisionSequence {
        revisions: labels.iter().map(|l| commit(l)).collect(),
    }
}

fn obs(name: &str, class: ObservationClass) -> ProbeObservation {
    ProbeObservation {
        commit: commit(name),
        class,
        kind: ProbeKind::Test,
        exit_code: Some(match class {
            ObservationClass::Pass => 0,
            ObservationClass::Skip => 125,
            _ => 1,
        }),
        timed_out: matches!(class, ObservationClass::Indeterminate),
        duration_ms: 1,
        stdout: String::new(),
        stderr: String::new(),
        sequence_index: 0,
        signal_number: None,
        probe_command: String::new(),
        working_dir: String::new(),
        flake_signal: None,
    }
}

/// Drive the session to completion: repeatedly ask for the next probe, record
/// the observation using the provided classifier, and return the final outcome.
fn run_narrowing(
    session: &mut LocalizationSession,
    classify: impl Fn(&CommitId) -> ObservationClass,
) -> LocalizationOutcome {
    while let Some(probe) = session.next_probe() {
        let class = classify(&probe);
        session.record(obs(&probe.0, class)).unwrap();
    }
    session.outcome()
}

fn policy_default() -> SearchPolicy {
    SearchPolicy::default()
}

fn policy_with_max_probes(max: usize) -> SearchPolicy {
    SearchPolicy {
        max_probes: max,
        ..SearchPolicy::default()
    }
}

// ---------------------------------------------------------------------------
// Scenario 1: Binary search on 8 commits finds exact midpoint
// ---------------------------------------------------------------------------

#[test]
fn scenario_binary_search_on_8_commits_finds_exact_midpoint() {
    // Given: 8 commits [c0..c7], c0-c3 pass, c4-c7 fail
    let labels: Vec<String> = (0..8).map(|i| format!("c{i}")).collect();
    let label_refs: Vec<&str> = labels.iter().map(|s| s.as_str()).collect();
    let seq = make_seq(&label_refs);
    let mut session = LocalizationSession::new(seq, policy_default()).unwrap();

    // When: run full narrowing loop
    let outcome = run_narrowing(&mut session, |c| {
        let idx: usize = c.0[1..].parse().unwrap();
        if idx <= 3 {
            ObservationClass::Pass
        } else {
            ObservationClass::Fail
        }
    });

    // Then: FirstBad with first_bad=c4, last_good=c3
    match outcome {
        LocalizationOutcome::FirstBad {
            first_bad,
            last_good,
            confidence,
        } => {
            assert_eq!(first_bad, commit("c4"), "first_bad should be c4");
            assert_eq!(last_good, commit("c3"), "last_good should be c3");
            assert_eq!(confidence, Confidence::high());
        }
        other => panic!("expected FirstBad, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Scenario 2: Two commits narrows immediately
// ---------------------------------------------------------------------------

#[test]
fn scenario_two_commits_narrows_immediately() {
    // Given: 2 commits [good, bad]
    let seq = make_seq(&["good", "bad"]);
    let mut session = LocalizationSession::new(seq, policy_default()).unwrap();

    // When: probes both
    let outcome = run_narrowing(&mut session, |c| {
        if c.0 == "good" {
            ObservationClass::Pass
        } else {
            ObservationClass::Fail
        }
    });

    // Then: FirstBad with first_bad=bad
    match outcome {
        LocalizationOutcome::FirstBad {
            first_bad,
            last_good,
            confidence,
        } => {
            assert_eq!(first_bad, commit("bad"), "first_bad should be 'bad'");
            assert_eq!(last_good, commit("good"), "last_good should be 'good'");
            assert_eq!(confidence, Confidence::high());
        }
        other => panic!("expected FirstBad, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Scenario 3: All pass except last still finds it
// ---------------------------------------------------------------------------

#[test]
fn scenario_all_pass_except_last_still_finds_it() {
    // Given: 5 commits, only last fails
    let labels: Vec<String> = (0..5).map(|i| format!("c{i}")).collect();
    let label_refs: Vec<&str> = labels.iter().map(|s| s.as_str()).collect();
    let seq = make_seq(&label_refs);
    let mut session = LocalizationSession::new(seq, policy_default()).unwrap();

    // When: run narrowing
    let outcome = run_narrowing(&mut session, |c| {
        if c.0 == "c4" {
            ObservationClass::Fail
        } else {
            ObservationClass::Pass
        }
    });

    // Then: FirstBad with first_bad=last (c4)
    match outcome {
        LocalizationOutcome::FirstBad {
            first_bad,
            last_good,
            confidence,
        } => {
            assert_eq!(first_bad, commit("c4"), "first_bad should be c4");
            assert_eq!(last_good, commit("c3"), "last_good should be c3");
            assert_eq!(confidence, Confidence::high());
        }
        other => panic!("expected FirstBad, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Scenario 4: Non-monotonic evidence produces SuspectWindow
// ---------------------------------------------------------------------------

#[test]
fn scenario_non_monotonic_evidence_produces_suspect_window() {
    // Given: 5 commits, c0=pass, c1=fail, c2=pass, c3=fail, c4=fail
    let seq = make_seq(&["c0", "c1", "c2", "c3", "c4"]);
    let mut session = LocalizationSession::new(seq, policy_default()).unwrap();

    // When: session records all observations manually
    let classes = [
        ("c0", ObservationClass::Pass),
        ("c1", ObservationClass::Fail),
        ("c2", ObservationClass::Pass),
        ("c3", ObservationClass::Fail),
        ("c4", ObservationClass::Fail),
    ];
    for (name, class) in &classes {
        session.record(obs(name, *class)).unwrap();
    }

    // Then: outcome contains NonMonotonicEvidence
    let outcome = session.outcome();
    match outcome {
        LocalizationOutcome::SuspectWindow { reasons, .. } => {
            assert!(
                reasons.contains(&AmbiguityReason::NonMonotonicEvidence),
                "expected NonMonotonicEvidence in reasons, got {reasons:?}"
            );
        }
        other => panic!("expected SuspectWindow, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Scenario 5: Max probes exhausted produces appropriate outcome
// ---------------------------------------------------------------------------

#[test]
fn scenario_max_probes_exhausted_limits_narrowing() {
    // Given: 100 commits, max_probes=3
    let labels: Vec<String> = (0..100).map(|i| format!("c{i}")).collect();
    let label_refs: Vec<&str> = labels.iter().map(|s| s.as_str()).collect();
    let seq = make_seq(&label_refs);
    let mut session = LocalizationSession::new(seq, policy_with_max_probes(3)).unwrap();

    // When: 3 probes recorded via the narrowing loop
    let mut probes_issued = 0;
    while let Some(probe) = session.next_probe() {
        let idx: usize = probe.0[1..].parse().unwrap();
        let class = if idx < 50 {
            ObservationClass::Pass
        } else {
            ObservationClass::Fail
        };
        session.record(obs(&probe.0, class)).unwrap();
        probes_issued += 1;
    }

    // Then: next_probe returns None after max_probes, and we got exactly 3
    assert_eq!(probes_issued, 3, "should issue exactly max_probes probes");
    assert!(
        session.next_probe().is_none(),
        "next_probe should return None after max_probes exhausted"
    );

    // Outcome reflects limited probing (Inconclusive with relevant reasons)
    let outcome = session.outcome();
    match outcome {
        LocalizationOutcome::Inconclusive { reasons } => {
            assert!(
                reasons.contains(&AmbiguityReason::MaxProbesExhausted)
                    || reasons.contains(&AmbiguityReason::NeedsMoreProbes),
                "expected MaxProbesExhausted or NeedsMoreProbes in reasons, got {reasons:?}"
            );
        }
        other => panic!("expected Inconclusive, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Scenario 6: Skip-only sequence produces Inconclusive
// ---------------------------------------------------------------------------

#[test]
fn scenario_skip_only_sequence_produces_inconclusive() {
    // Given: 5 commits, all probes return Skip
    let seq = make_seq(&["c0", "c1", "c2", "c3", "c4"]);
    let mut session = LocalizationSession::new(seq, policy_default()).unwrap();

    // When: run narrowing where every probe is Skip
    let outcome = run_narrowing(&mut session, |_| ObservationClass::Skip);

    // Then: Inconclusive
    match outcome {
        LocalizationOutcome::Inconclusive { reasons } => {
            // Without any pass or fail, we expect missing boundary reasons
            assert!(
                reasons.contains(&AmbiguityReason::MissingPassBoundary)
                    || reasons.contains(&AmbiguityReason::MissingFailBoundary),
                "expected missing boundary reasons for skip-only, got {reasons:?}"
            );
        }
        other => panic!("expected Inconclusive, got {other:?}"),
    }
}
