mod common;

use common::{add, endpoints, packet, receipt};
use faultline_integration::{IntegrationOutcome, LocalizationBasis, StateEquivalence};
use faultline_queue::{DiagnosisStatus, EvidenceSource, LaunchRecord, PhaseConclusion, StateEvidence, diagnose};
use proptest::prelude::*;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn every_twenty_pr_boundary_needs_at_most_five_midpoint_launches() -> TestResult {
    for failure_at in 1..=20 {
        let mut input = packet(20)?;
        endpoints(&mut input)?;
        let mut count = 0;
        loop {
            let report = diagnose(&input)?;
            if let Some(plan) = report.next_probe {
                assert_eq!(plan.maximum_new_launches, 1);
                assert!(count < 5, "budget exceeded for boundary {failure_at}");
                count += 1;
                add(&mut input, plan.state.ordinal, &format!("native-{count}"), plan.state.ordinal < failure_at, EvidenceSource::Native)?;
            } else {
                assert_eq!(report.status, DiagnosisStatus::Localized);
                match report.outcome {
                    Some(IntegrationOutcome::FirstFailingTransition { first_failing, .. }) => assert_eq!(first_failing.ordinal, failure_at),
                    other => return Err(format!("wrong outcome: {other:?}").into()),
                }
                assert_eq!(report.recorded_launches, count);
                assert_eq!(report.provider_receipts, 2);
                break;
            }
        }
    }
    Ok(())
}

#[test]
fn all_compatible_prefix_evidence_requires_no_launches() -> TestResult {
    let mut input = packet(20)?;
    input.policy.max_launches = 0;
    for ordinal in 0..=20 {
        add(&mut input, ordinal, &format!("provider-{ordinal}"), ordinal < 14, EvidenceSource::Provider)?;
    }
    let report = diagnose(&input)?;
    assert_eq!(report.status, DiagnosisStatus::Localized);
    assert_eq!(report.recorded_launches, 0);
    assert!(report.next_probe.is_none());
    assert!(matches!(report.outcome, Some(IntegrationOutcome::FirstFailingTransition { basis: LocalizationBasis::CompletePrefixObserved, .. })));
    Ok(())
}

#[test]
fn no_assumption_requires_the_whole_earlier_prefix() -> TestResult {
    let mut input = packet(20)?;
    input.policy.assume_monotonic = false;
    input.policy.max_launches = 20;
    endpoints(&mut input)?;
    add(&mut input, 13, "sample13", true, EvidenceSource::Provider)?;
    add(&mut input, 14, "sample14", false, EvidenceSource::Provider)?;
    let report = diagnose(&input)?;
    assert_eq!(report.status, DiagnosisStatus::NeedsEvidence);
    assert_eq!(report.next_probe.ok_or("missing next probe")?.state.ordinal, 1);
    for ordinal in 1..13 {
        add(&mut input, ordinal, &format!("sample{ordinal}"), true, EvidenceSource::Provider)?;
    }
    let report = diagnose(&input)?;
    assert_eq!(report.status, DiagnosisStatus::Localized);
    assert!(matches!(report.outcome, Some(IntegrationOutcome::FirstFailingTransition { basis: LocalizationBasis::CompletePrefixObserved, .. })));
    Ok(())
}

#[test]
fn non_monotonic_samples_stop_binary_planning() -> TestResult {
    let mut input = packet(20)?;
    endpoints(&mut input)?;
    add(&mut input, 4, "fail4", false, EvidenceSource::Provider)?;
    add(&mut input, 8, "pass8", true, EvidenceSource::Provider)?;
    let report = diagnose(&input)?;
    assert_eq!(report.status, DiagnosisStatus::NonMonotonic);
    assert!(report.next_probe.is_none());
    Ok(())
}

#[test]
fn incomplete_samples_do_not_hide_non_monotonic_evidence() -> TestResult {
    let mut input = packet(20)?;
    input.policy.minimum_attempts = 2;
    add(&mut input, 4, "fail4", false, EvidenceSource::Provider)?;
    add(&mut input, 8, "pass8", true, EvidenceSource::Provider)?;
    assert_eq!(diagnose(&input)?.status, DiagnosisStatus::NonMonotonic);
    Ok(())
}

#[test]
fn conflicting_attempts_are_not_overwritten_or_majority_voted() -> TestResult {
    let mut input = packet(20)?;
    endpoints(&mut input)?;
    add(&mut input, 10, "pass10a", true, EvidenceSource::Provider)?;
    add(&mut input, 10, "pass10b", true, EvidenceSource::Provider)?;
    add(&mut input, 10, "fail10", false, EvidenceSource::Provider)?;
    let report = diagnose(&input)?;
    assert_eq!(report.status, DiagnosisStatus::Suspect);
    assert!(report.next_probe.is_none());
    let summary = report.states.get(10).ok_or("state10")?;
    assert_eq!(summary.evidence, StateEvidence::Unstable);
    assert_eq!(summary.receipt_ids.len(), 3);
    Ok(())
}

#[test]
fn foreign_context_and_target_failures_never_become_boundaries() -> TestResult {
    for field in ["snapshot", "phase", "predicate", "context", "commit", "tree", "state"] {
        let mut input = packet(20)?;
        endpoints(&mut input)?;
        let mut bad = receipt(&input, 1, "foreign", false, EvidenceSource::Provider)?;
        match field {
            "snapshot" => bad.snapshot_id = "foreign".into(),
            "phase" => bad.phase = "foreign".into(),
            "predicate" => bad.predicate_id = "foreign".into(),
            "context" => bad.context_id = "foreign".into(),
            "commit" => bad.commit = "foreign".into(),
            "tree" => bad.tree = "foreign".into(),
            _ => bad.state_id = "foreign".into(),
        }
        input.receipts.push(bad);
        let report = diagnose(&input)?;
        assert_eq!(report.excluded_receipts.len(), 1);
        assert_eq!(report.next_probe.ok_or("next")?.state.ordinal, 10);
    }
    let mut input = packet(20)?;
    endpoints(&mut input)?;
    let mut other = receipt(&input, 10, "other", false, EvidenceSource::Provider)?;
    other.failure_signature = Some("unrelated-assertion".into());
    input.receipts.push(other);
    let report = diagnose(&input)?;
    assert_eq!(report.states.get(10).ok_or("10")?.evidence, StateEvidence::OtherFailure);
    assert_ne!(report.status, DiagnosisStatus::Localized);
    Ok(())
}

#[test]
fn pending_launch_is_charged_and_blocks_duplicate_work() -> TestResult {
    let mut input = packet(20)?;
    endpoints(&mut input)?;
    let state = input.sequence.states.get(10).ok_or("state")?;
    input.launches.push(LaunchRecord { attempt_id: "pending".into(), state_id: state.id.0.clone(), commit: state.commit.0.clone(), tree: state.tree.0.clone() });
    let report = diagnose(&input)?;
    assert_eq!(report.status, DiagnosisStatus::AwaitingEvidence);
    assert_eq!(report.recorded_launches, 1);
    assert_eq!(report.remaining_launches, 4);
    assert!(report.next_probe.is_none());
    Ok(())
}

#[test]
fn pending_attempt_blocks_finalization_even_with_an_adjacent_provider_boundary() -> TestResult {
    let mut input = packet(1)?;
    endpoints(&mut input)?;
    let state = input.sequence.states.get(1).ok_or("state")?;
    input.launches.push(LaunchRecord { attempt_id: "pending".into(), state_id: state.id.0.clone(), commit: state.commit.0.clone(), tree: state.tree.0.clone() });
    let report = diagnose(&input)?;
    assert_eq!(report.status, DiagnosisStatus::AwaitingEvidence);
    assert!(report.outcome.is_none());
    Ok(())
}

#[test]
fn native_receipt_requires_the_matching_charged_launch() -> TestResult {
    let mut input = packet(20)?;
    input.receipts.push(receipt(&input, 10, "unpaid", true, EvidenceSource::Native)?);
    assert!(diagnose(&input).is_err());
    Ok(())
}

#[test]
fn native_launch_cannot_be_relabelled_as_provider_evidence() -> TestResult {
    let mut input = packet(20)?;
    add(&mut input, 10, "paid", true, EvidenceSource::Native)?;
    input.receipts.first_mut().ok_or("receipt")?.source = EvidenceSource::Provider;
    assert!(diagnose(&input).is_err());
    Ok(())
}

#[test]
fn duplicate_attempt_is_idempotent_but_rewritten_attempt_is_invalid() -> TestResult {
    let mut input = packet(20)?;
    endpoints(&mut input)?;
    add(&mut input, 10, "native", true, EvidenceSource::Native)?;
    let once = diagnose(&input)?;
    input.receipts.push(input.receipts.last().ok_or("receipt")?.clone());
    input.launches.push(input.launches.last().ok_or("launch")?.clone());
    assert_eq!(diagnose(&input)?, once);
    input.receipts.last_mut().ok_or("receipt")?.log_id = "rewritten".into();
    assert!(diagnose(&input).is_err());
    Ok(())
}

#[test]
fn exhausted_budget_preserves_the_supported_window() -> TestResult {
    let mut input = packet(20)?;
    input.policy.max_launches = 1;
    endpoints(&mut input)?;
    add(&mut input, 10, "native", true, EvidenceSource::Native)?;
    let report = diagnose(&input)?;
    assert_eq!(report.status, DiagnosisStatus::BudgetExhausted);
    assert!(report.next_probe.is_none());
    match report.outcome {
        Some(IntegrationOutcome::SuspectTransitions { lower_bound_exclusive, upper_bound_inclusive, .. }) => {
            assert_eq!(lower_bound_exclusive.ordinal, 10);
            assert_eq!(upper_bound_inclusive.ordinal, 20);
        }
        _ => return Err("lost budget-limited suspect window".into()),
    }
    Ok(())
}

#[test]
fn operational_attempts_are_charged_not_converted_to_code_votes() -> TestResult {
    let mut input = packet(20)?;
    endpoints(&mut input)?;
    add(&mut input, 10, "infra", false, EvidenceSource::Native)?;
    let operational = input.receipts.last_mut().ok_or("infra")?;
    operational.conclusion = PhaseConclusion::Operational;
    operational.failure_signature = None;
    add(&mut input, 10, "clean", true, EvidenceSource::Native)?;
    let report = diagnose(&input)?;
    assert_eq!(report.recorded_launches, 2);
    assert_eq!(report.states.get(10).ok_or("state")?.evidence, StateEvidence::Passing);
    assert_eq!(report.states.get(10).ok_or("state")?.operational_attempts, 1);
    Ok(())
}

#[test]
fn stable_sample_requirement_charges_each_retry() -> TestResult {
    let mut input = packet(20)?;
    input.policy.minimum_attempts = 2;
    input.policy.max_launches = 10;
    endpoints(&mut input)?;
    add(&mut input, 0, "base2", true, EvidenceSource::Provider)?;
    add(&mut input, 20, "head2", false, EvidenceSource::Provider)?;
    for attempt in 0..=10 {
        let report = diagnose(&input)?;
        if let Some(plan) = report.next_probe {
            assert!(attempt < 10);
            add(&mut input, plan.state.ordinal, &format!("retry-{attempt}"), plan.state.ordinal < 14, EvidenceSource::Native)?;
        } else {
            assert_eq!(report.status, DiagnosisStatus::Localized);
            assert_eq!(report.recorded_launches % 2, 0);
            return Ok(());
        }
    }
    Err("sample requirement failed to converge".into())
}

#[test]
fn baseline_failure_does_not_blame_the_first_queued_pr() -> TestResult {
    let mut input = packet(20)?;
    add(&mut input, 0, "base", false, EvidenceSource::Provider)?;
    add(&mut input, 20, "head", false, EvidenceSource::Provider)?;
    let report = diagnose(&input)?;
    assert_eq!(report.status, DiagnosisStatus::Inconclusive);
    assert!(report.reason_codes.contains(&"baseline_already_failing".into()));
    assert!(report.next_probe.is_none());
    Ok(())
}

#[test]
fn passing_head_never_starts_a_diagnostic_probe() -> TestResult {
    let mut input = packet(20)?;
    add(&mut input, 20, "head", true, EvidenceSource::Provider)?;
    let report = diagnose(&input)?;
    assert_eq!(report.status, DiagnosisStatus::NoTargetFailure);
    assert!(report.next_probe.is_none());
    Ok(())
}

#[test]
fn weak_state_evidence_cannot_define_an_exact_provider_boundary() -> TestResult {
    let mut input = packet(1)?;
    input.policy.required_equivalence = StateEquivalence::Unverified;
    input.sequence.states.get_mut(1).ok_or("head")?.equivalence = StateEquivalence::PatchEquivalent;
    endpoints(&mut input)?;
    let report = diagnose(&input)?;
    assert_ne!(report.status, DiagnosisStatus::Localized);
    assert_eq!(report.excluded_receipts.len(), 1);
    assert!(report.next_probe.is_none());
    Ok(())
}

#[test]
fn malformed_phase_result_and_unknown_schema_are_rejected() -> TestResult {
    let mut input = packet(1)?;
    endpoints(&mut input)?;
    input.receipts.first_mut().ok_or("base")?.exit_code = Some(1);
    assert!(diagnose(&input).is_err());
    input.receipts.first_mut().ok_or("base")?.exit_code = Some(0);
    input.schema_version = "future".into();
    assert!(diagnose(&input).is_err());
    Ok(())
}

#[test]
fn json_roundtrip_is_stable_and_unknown_fields_are_rejected() -> TestResult {
    let mut input = packet(20)?;
    endpoints(&mut input)?;
    let before = diagnose(&input)?;
    let encoded = serde_json::to_vec(&input)?;
    let decoded = serde_json::from_slice(&encoded)?;
    assert_eq!(diagnose(&decoded)?, before);
    let mut value = serde_json::to_value(input)?;
    value.as_object_mut().ok_or("object")?.insert("assume_green".into(), serde_json::json!(true));
    assert!(serde_json::from_value::<faultline_queue::DiagnosisInput>(value).is_err());
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]
    #[test]
    fn arbitrary_budget_never_emits_an_unfunded_probe(failure_at in 1usize..=20, budget in 0usize..=5) {
        let mut input = packet(20).map_err(|e| TestCaseError::fail(e.to_string()))?;
        endpoints(&mut input).map_err(|e| TestCaseError::fail(e.to_string()))?;
        input.policy.max_launches = budget;
        for attempt in 0..=budget {
            let report = diagnose(&input).map_err(|e| TestCaseError::fail(e.to_string()))?;
            if let Some(plan) = report.next_probe {
                prop_assert!(attempt < budget);
                prop_assert!(report.remaining_launches > 0);
                add(&mut input, plan.state.ordinal, &format!("native-{attempt}"), plan.state.ordinal < failure_at, EvidenceSource::Native).map_err(|e| TestCaseError::fail(e.to_string()))?;
            } else {
                prop_assert!(report.recorded_launches <= budget);
                break;
            }
        }
    }
}
