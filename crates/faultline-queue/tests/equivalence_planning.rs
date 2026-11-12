mod common;

use common::{add, endpoints, packet};
use faultline_integration::{IntegrationOutcome, StateEquivalence};
use faultline_queue::{DiagnosisStatus, EvidenceSource, diagnose};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn weak_midpoint_does_not_block_eligible_probes() -> TestResult {
    let mut input = packet(20)?;
    input
        .sequence
        .states
        .get_mut(10)
        .ok_or("midpoint")?
        .equivalence = StateEquivalence::PatchEquivalent;
    endpoints(&mut input)?;
    let report = diagnose(&input)?;
    assert_eq!(
        report.status,
        DiagnosisStatus::NeedsEvidence,
        "an unusable midpoint must not block usable candidates"
    );
    let plan = report.next_probe.ok_or("eligible next probe")?;
    assert_ne!(plan.state.ordinal, 10);
    assert!(
        plan.state
            .equivalence
            .satisfies(input.policy.required_equivalence)
    );
    Ok(())
}

#[test]
fn conservative_scan_skips_weak_states_without_proving_the_prefix() -> TestResult {
    let mut input = packet(4)?;
    input.policy.assume_monotonic = false;
    input
        .sequence
        .states
        .get_mut(1)
        .ok_or("prefix")?
        .equivalence = StateEquivalence::PatchEquivalent;
    endpoints(&mut input)?;
    let report = diagnose(&input)?;
    assert_eq!(
        report
            .next_probe
            .ok_or("skip weak prefix and probe eligible state")?
            .state
            .ordinal,
        2
    );
    add(
        &mut input,
        2,
        "earlier-failure",
        false,
        EvidenceSource::Provider,
    )?;
    let report = diagnose(&input)?;
    assert_eq!(report.status, DiagnosisStatus::Suspect);
    assert!(report.next_probe.is_none());
    match report.outcome {
        Some(IntegrationOutcome::SuspectTransitions {
            lower_bound_exclusive,
            upper_bound_inclusive,
            transitions,
            ..
        }) => {
            assert_eq!(lower_bound_exclusive.ordinal, 0);
            assert_eq!(upper_bound_inclusive.ordinal, 2);
            assert_eq!(transitions.len(), 2);
        }
        _ => return Err("missing prefix must remain a suspect interval".into()),
    }
    Ok(())
}

#[test]
fn eligible_boundary_can_localize_despite_a_weak_earlier_midpoint() -> TestResult {
    let mut input = packet(20)?;
    input
        .sequence
        .states
        .get_mut(10)
        .ok_or("midpoint")?
        .equivalence = StateEquivalence::PatchEquivalent;
    endpoints(&mut input)?;
    for attempt in 0..=5 {
        let report = diagnose(&input)?;
        if let Some(plan) = report.next_probe {
            assert!(attempt < 5, "planner exceeded its native-launch budget");
            assert_ne!(plan.state.ordinal, 10);
            add(
                &mut input,
                plan.state.ordinal,
                &format!("native-{attempt}"),
                plan.state.ordinal < 14,
                EvidenceSource::Native,
            )?;
        } else {
            assert_eq!(
                report.status,
                DiagnosisStatus::Localized,
                "weak earlier state must not block a supported later boundary"
            );
            match report.outcome {
                Some(IntegrationOutcome::FirstFailingTransition { first_failing, .. }) => {
                    assert_eq!(first_failing.ordinal, 14)
                }
                _ => return Err("missing supported transition".into()),
            }
            return Ok(());
        }
    }
    Err("planner did not converge".into())
}

#[test]
fn unavailable_interior_retains_a_window_without_spending_budget() -> TestResult {
    let mut input = packet(3)?;
    for state in input.sequence.states.iter_mut().skip(1).take(2) {
        state.equivalence = StateEquivalence::Unverified;
    }
    endpoints(&mut input)?;
    let report = diagnose(&input)?;
    assert_eq!(report.status, DiagnosisStatus::Suspect);
    assert!(report.next_probe.is_none());
    assert_eq!(report.recorded_launches, 0);
    assert_eq!(report.remaining_launches, input.policy.max_launches);
    match report.outcome {
        Some(IntegrationOutcome::SuspectTransitions { transitions, .. }) => {
            assert_eq!(transitions.len(), 3)
        }
        _ => return Err("unavailable states must remain in the suspect window".into()),
    }
    Ok(())
}
