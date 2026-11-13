mod common;

use common::{add, endpoints, packet};
use faultline_queue::{DiagnosisStatus, EvidenceSource, PhaseConclusion, diagnose};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn no_evidence_never_proposes_a_launch() -> TestResult {
    let report = diagnose(&packet(20)?)?;
    assert!(
        report.next_probe.is_none(),
        "empty evidence must not launch a predicate"
    );
    assert_eq!(report.status, DiagnosisStatus::Inconclusive);
    assert!(
        report
            .reason_codes
            .contains(&"target_failure_not_established_at_head".into())
    );
    Ok(())
}

#[test]
fn partial_pass_at_head_does_not_start_diagnosis() -> TestResult {
    let mut input = packet(20)?;
    input.policy.minimum_attempts = 2;
    add(&mut input, 20, "one-pass", true, EvidenceSource::Provider)?;
    let report = diagnose(&input)?;
    assert!(
        report.next_probe.is_none(),
        "a partial pass is not a red-queue trigger"
    );
    assert_eq!(report.status, DiagnosisStatus::Inconclusive);
    Ok(())
}

#[test]
fn incompatible_head_failure_does_not_authorize_a_launch() -> TestResult {
    let mut input = packet(20)?;
    endpoints(&mut input)?;
    input.receipts.last_mut().ok_or("head receipt")?.context_id = "different-context".into();
    let report = diagnose(&input)?;
    assert_eq!(report.excluded_receipts.len(), 1);
    assert!(
        report.next_probe.is_none(),
        "excluded failure evidence cannot admit work"
    );
    Ok(())
}

#[test]
fn partial_target_failure_at_head_can_request_confirmation() -> TestResult {
    let mut input = packet(20)?;
    input.policy.minimum_attempts = 2;
    add(
        &mut input,
        20,
        "one-failure",
        false,
        EvidenceSource::Provider,
    )?;
    let report = diagnose(&input)?;
    assert_eq!(report.status, DiagnosisStatus::NeedsEvidence);
    assert_eq!(
        report.next_probe.ok_or("confirmation probe")?.state.ordinal,
        20
    );
    assert_eq!(report.recorded_launches, 0);
    Ok(())
}

#[test]
fn missing_native_receipt_cannot_finalize() -> TestResult {
    let mut input = packet(1)?;
    endpoints(&mut input)?;
    add(&mut input, 1, "unfinished", false, EvidenceSource::Native)?;
    let receipt = input.receipts.last_mut().ok_or("native receipt")?;
    receipt.conclusion = PhaseConclusion::Missing;
    receipt.exit_code = None;
    receipt.failure_signature = None;
    let report = diagnose(&input)?;
    assert_eq!(
        report.status,
        DiagnosisStatus::AwaitingEvidence,
        "missing is not terminal evidence"
    );
    assert_eq!(report.pending_attempts, vec!["unfinished"]);
    assert_eq!(report.recorded_launches, 1);
    assert!(report.outcome.is_none());
    assert!(report.next_probe.is_none());
    Ok(())
}

#[test]
fn terminal_operational_receipt_settles_without_code_vote() -> TestResult {
    for conclusion in [PhaseConclusion::Operational, PhaseConclusion::Cancelled] {
        let mut input = packet(1)?;
        endpoints(&mut input)?;
        add(&mut input, 1, "terminated", false, EvidenceSource::Native)?;
        let receipt = input.receipts.last_mut().ok_or("native receipt")?;
        receipt.conclusion = conclusion;
        receipt.exit_code = None;
        receipt.failure_signature = None;
        let report = diagnose(&input)?;
        assert_eq!(report.status, DiagnosisStatus::Localized);
        assert!(report.pending_attempts.is_empty());
        assert_eq!(report.recorded_launches, 1);
        assert_eq!(report.states.get(1).ok_or("head")?.target_failures, 1);
        assert_eq!(report.states.get(1).ok_or("head")?.operational_attempts, 1);
    }
    Ok(())
}

#[test]
fn over_budget_ledger_is_rejected() -> TestResult {
    let mut input = packet(1)?;
    input.policy.max_launches = 0;
    add(&mut input, 0, "base", true, EvidenceSource::Provider)?;
    add(&mut input, 1, "overspent", false, EvidenceSource::Native)?;
    assert!(
        diagnose(&input).is_err(),
        "an overspent ledger must not finalize successfully"
    );
    Ok(())
}

#[test]
fn provider_only_complete_evidence_needs_no_launch_budget() -> TestResult {
    let mut input = packet(1)?;
    input.policy.max_launches = 0;
    endpoints(&mut input)?;
    let report = diagnose(&input)?;
    assert_eq!(report.status, DiagnosisStatus::Localized);
    assert_eq!(report.recorded_launches, 0);
    assert!(report.next_probe.is_none());
    Ok(())
}

#[test]
fn at_budget_completed_native_result_is_valid() -> TestResult {
    let mut input = packet(1)?;
    input.policy.max_launches = 1;
    add(&mut input, 0, "base", true, EvidenceSource::Provider)?;
    add(&mut input, 1, "native-head", false, EvidenceSource::Native)?;
    let report = diagnose(&input)?;
    assert_eq!(report.status, DiagnosisStatus::Localized);
    assert_eq!(report.recorded_launches, 1);
    assert_eq!(report.remaining_launches, 0);
    Ok(())
}
