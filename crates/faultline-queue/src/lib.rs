//! Offline integration diagnosis and bounded probe planning.
//!
//! This application layer reuses Faultline's localization engine. It does not
//! execute predicates, authenticate providers, fetch Git objects, or change a
//! queue. Receipt authenticity and Git-object custody belong to the collector.
//! All evidence used here must nevertheless match the captured experiment.

#![forbid(unsafe_code)]

use faultline_codes::{AmbiguityReason, ObservationClass, ProbeKind};
use faultline_integration::{
    IntegrationOutcome, IntegrationReversal, IntegrationSequence, IntegrationStateRef,
    LocalizationBasis, StateEquivalence,
};
use faultline_localization::LocalizationSession;
use faultline_types::{Confidence, LocalizationOutcome, ProbeObservation, SearchPolicy};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const SCHEMA_VERSION: &str = "0.1.0";
const MAX_STATES: usize = 4096;
const MAX_RECEIPTS: usize = 65536;

#[derive(Debug, Error)]
pub enum DiagnosisError {
    #[error("invalid queue evidence: {0}")]
    Invalid(String),
    #[error(transparent)]
    Integration(#[from] faultline_integration::IntegrationError),
    #[error(transparent)]
    Localization(#[from] faultline_types::FaultlineError),
}

type Result<T> = std::result::Result<T, DiagnosisError>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TargetIdentity {
    pub phase: String,
    pub failure_signature: String,
    pub predicate_id: String,
    pub context_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DiagnosisPolicy {
    /// Includes every native launch in this investigation, even without a receipt.
    pub max_launches: usize,
    /// Explicit sample policy. Conflicting samples never become a majority vote.
    pub minimum_attempts: usize,
    /// False requires observed passing evidence for the entire earlier prefix.
    pub assume_monotonic: bool,
    pub required_equivalence: StateEquivalence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceSource {
    Provider,
    Native,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PhaseConclusion {
    Passed,
    Failed,
    Operational,
    Cancelled,
    Missing,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PhaseReceipt {
    pub attempt_id: String,
    pub snapshot_id: String,
    pub state_id: String,
    pub commit: String,
    pub tree: String,
    pub phase: String,
    pub predicate_id: String,
    pub context_id: String,
    pub source: EvidenceSource,
    pub conclusion: PhaseConclusion,
    pub exit_code: Option<i32>,
    pub failure_signature: Option<String>,
    /// Opaque retained-log identity; no raw log content is exported.
    pub log_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LaunchRecord {
    pub attempt_id: String,
    pub state_id: String,
    pub commit: String,
    pub tree: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DiagnosisInput {
    #[schemars(schema_with = "schema_version_schema")]
    pub schema_version: String,
    pub sequence: IntegrationSequence,
    pub target: TargetIdentity,
    pub policy: DiagnosisPolicy,
    pub launches: Vec<LaunchRecord>,
    pub receipts: Vec<PhaseReceipt>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StateEvidence {
    Unobserved,
    Incomplete,
    Passing,
    TargetFailing,
    Unstable,
    OtherFailure,
    Operational,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct StateSummary {
    pub state: IntegrationStateRef,
    pub evidence: StateEvidence,
    pub receipt_ids: Vec<String>,
    pub passing_attempts: usize,
    pub target_failures: usize,
    pub other_failures: usize,
    pub operational_attempts: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ExcludedReceipt {
    pub attempt_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosisStatus {
    NoTargetFailure,
    NeedsEvidence,
    AwaitingEvidence,
    BudgetExhausted,
    Localized,
    Suspect,
    NonMonotonic,
    Inconclusive,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProbePlan {
    pub state: IntegrationStateRef,
    pub target: TargetIdentity,
    pub snapshot_id: String,
    pub maximum_new_launches: usize,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DiagnosisReport {
    #[schemars(schema_with = "schema_version_schema")]
    pub schema_version: String,
    pub snapshot_id: String,
    pub target: TargetIdentity,
    pub policy: DiagnosisPolicy,
    pub status: DiagnosisStatus,
    pub outcome: Option<IntegrationOutcome>,
    pub next_probe: Option<ProbePlan>,
    pub states: Vec<StateSummary>,
    pub excluded_receipts: Vec<ExcludedReceipt>,
    pub pending_attempts: Vec<String>,
    pub recorded_launches: usize,
    pub remaining_launches: usize,
    pub provider_receipts: usize,
    pub native_receipts: usize,
    pub reason_codes: Vec<String>,
    pub limitations: Vec<String>,
}

fn schema_version_schema(_: &mut schemars::r#gen::SchemaGenerator) -> schemars::schema::Schema {
    schemars::schema::SchemaObject {
        instance_type: Some(schemars::schema::InstanceType::String.into()),
        const_value: Some(serde_json::Value::String(SCHEMA_VERSION.into())),
        ..Default::default()
    }
    .into()
}

fn invalid(message: impl Into<String>) -> DiagnosisError {
    DiagnosisError::Invalid(message.into())
}

fn identifier(value: &str, field: &str) -> Result<()> {
    if value.trim().is_empty() || value.len() > 1024 || value.chars().any(char::is_control) {
        return Err(invalid(format!(
            "{field} must be a nonempty bounded identifier"
        )));
    }
    Ok(())
}

fn validate_receipt(receipt: &PhaseReceipt) -> Result<()> {
    for (field, value) in [
        ("attempt_id", &receipt.attempt_id),
        ("snapshot_id", &receipt.snapshot_id),
        ("state_id", &receipt.state_id),
        ("commit", &receipt.commit),
        ("tree", &receipt.tree),
        ("phase", &receipt.phase),
        ("predicate_id", &receipt.predicate_id),
        ("context_id", &receipt.context_id),
        ("log_id", &receipt.log_id),
    ] {
        identifier(value, field)?;
    }
    match receipt.conclusion {
        PhaseConclusion::Passed
            if receipt.exit_code == Some(0) && receipt.failure_signature.is_none() => {}
        PhaseConclusion::Failed if receipt.exit_code.is_some_and(|code| code != 0) => {
            identifier(
                receipt
                    .failure_signature
                    .as_deref()
                    .ok_or_else(|| invalid("failed phase requires a signature"))?,
                "failure_signature",
            )?;
        }
        PhaseConclusion::Operational | PhaseConclusion::Cancelled | PhaseConclusion::Missing
            if receipt.failure_signature.is_none() => {}
        _ => {
            return Err(invalid(
                "phase conclusion contradicts its exit code or signature",
            ));
        }
    }
    Ok(())
}

/// Validate, aggregate without overwriting attempts, and plan at most one launch.
/// An identical duplicate receipt is idempotent; a rewritten attempt is rejected.
pub fn diagnose(input: &DiagnosisInput) -> Result<DiagnosisReport> {
    if input.schema_version != SCHEMA_VERSION {
        return Err(invalid("unsupported diagnosis input schema_version"));
    }
    if input.sequence.len() > MAX_STATES
        || input.receipts.len() > MAX_RECEIPTS
        || input.launches.len() > MAX_RECEIPTS
    {
        return Err(invalid("diagnosis evidence exceeds bounded input limits"));
    }
    input.sequence.validate()?;
    if !(1..=64).contains(&input.policy.minimum_attempts) {
        return Err(invalid("minimum_attempts must be between 1 and 64"));
    }
    for (field, value) in [
        ("phase", &input.target.phase),
        ("failure_signature", &input.target.failure_signature),
        ("predicate_id", &input.target.predicate_id),
        ("context_id", &input.target.context_id),
    ] {
        identifier(value, field)?;
    }
    let by_id: BTreeMap<_, _> = input
        .sequence
        .states
        .iter()
        .map(|state| (state.id.0.as_str(), state))
        .collect();
    let mut launches = BTreeMap::new();
    for launch in &input.launches {
        identifier(&launch.attempt_id, "launch attempt_id")?;
        let state = by_id
            .get(launch.state_id.as_str())
            .ok_or_else(|| invalid("launch references an unknown state"))?;
        if state.commit.0 != launch.commit || state.tree.0 != launch.tree {
            return Err(invalid("launch does not match the captured state"));
        }
        if let Some(previous) = launches.insert(launch.attempt_id.as_str(), launch)
            && previous != launch
        {
            return Err(invalid("conflicting duplicate launch identity"));
        }
    }
    if launches.len() > input.policy.max_launches {
        return Err(invalid("native launch ledger exceeds total launch budget"));
    }
    let mut receipts = BTreeMap::new();
    for receipt in &input.receipts {
        validate_receipt(receipt)?;
        if let Some(previous) = receipts.insert(receipt.attempt_id.as_str(), receipt)
            && previous != receipt
        {
            return Err(invalid("conflicting duplicate receipt identity"));
        }
    }
    let mut eligible: BTreeMap<usize, Vec<&PhaseReceipt>> = BTreeMap::new();
    let mut excluded_receipts = Vec::new();
    let mut ineligible_native_receipt = false;
    let mut settled = BTreeSet::new();
    let mut provider_receipts = 0;
    let mut native_receipts = 0;
    let minimum = input
        .policy
        .required_equivalence
        .max(StateEquivalence::ExactTree);
    for receipt in receipts.values() {
        match receipt.source {
            EvidenceSource::Native => {
                let launch = launches
                    .get(receipt.attempt_id.as_str())
                    .ok_or_else(|| invalid("native receipt has no charged launch"))?;
                if launch.state_id != receipt.state_id
                    || launch.commit != receipt.commit
                    || launch.tree != receipt.tree
                {
                    return Err(invalid("native receipt contradicts its launch identity"));
                }
                native_receipts += 1;
            }
            EvidenceSource::Provider => {
                if launches.contains_key(receipt.attempt_id.as_str()) {
                    return Err(invalid(
                        "native launch cannot be relabeled as provider evidence",
                    ));
                }
                provider_receipts += 1;
            }
        }
        let state = by_id.get(receipt.state_id.as_str());
        let reason = if receipt.snapshot_id != input.sequence.snapshot_id.0 {
            Some("snapshot_mismatch")
        } else if receipt.phase != input.target.phase {
            Some("phase_mismatch")
        } else if receipt.predicate_id != input.target.predicate_id {
            Some("predicate_mismatch")
        } else if receipt.context_id != input.target.context_id {
            Some("context_mismatch")
        } else if let Some(state) = state {
            if receipt.commit != state.commit.0 || receipt.tree != state.tree.0 {
                Some("state_identity_mismatch")
            } else if !state.equivalence.satisfies(minimum) {
                Some("insufficient_state_equivalence")
            } else {
                None
            }
        } else {
            Some("unknown_state")
        };
        if let Some(reason) = reason {
            if receipt.source == EvidenceSource::Native {
                ineligible_native_receipt = true;
                // Completion and target-evidence eligibility are separate.
                // Missing output still cannot prove worker termination.
                if receipt.conclusion != PhaseConclusion::Missing {
                    settled.insert(receipt.attempt_id.as_str());
                }
            }
            excluded_receipts.push(ExcludedReceipt {
                attempt_id: receipt.attempt_id.clone(),
                reason: reason.into(),
            });
        } else if let Some(state) = state {
            // Missing output does not prove that the owned worker has terminated.
            if receipt.conclusion != PhaseConclusion::Missing {
                settled.insert(receipt.attempt_id.as_str());
            }
            eligible.entry(state.ordinal).or_default().push(receipt);
        }
    }
    let states = input
        .sequence
        .states
        .iter()
        .map(|state| {
            let attempts = eligible.get(&state.ordinal).cloned().unwrap_or_default();
            summarize(
                state.into(),
                &attempts,
                &input.target,
                input.policy.minimum_attempts,
            )
        })
        .collect();
    // A matching terminal attempt may settle while its evidence is ineligible.
    // Missing receipts remain pending; wrong state/commit/tree is rejected above.
    let pending_attempts = launches
        .keys()
        .filter(|id| !settled.contains(**id))
        .map(|id| (*id).to_string())
        .collect();
    let mut report = DiagnosisReport {
        schema_version: SCHEMA_VERSION.into(),
        snapshot_id: input.sequence.snapshot_id.0.clone(),
        target: input.target.clone(),
        policy: input.policy.clone(),
        status: DiagnosisStatus::Inconclusive,
        outcome: None,
        next_probe: None,
        states,
        excluded_receipts,
        pending_attempts,
        recorded_launches: launches.len(),
        remaining_launches: input.policy.max_launches.saturating_sub(launches.len()),
        provider_receipts,
        native_receipts,
        reason_codes: if ineligible_native_receipt { vec!["launch_receipt_ineligible".into()] } else { Vec::new() },
        limitations: vec![
            "Offline replay: no predicates executed and no queue mutations performed.".into(),
            "Identity consistency is checked; provider authenticity and object custody are external.".into(),
            "A transition is a trigger in this ordering, not independent-defect or interaction proof.".into(),
        ],
    };
    if input.sequence.provider == "synthetic" {
        report
            .limitations
            .push("Synthetic demonstration input, not live queue evidence.".into());
    }
    plan(input, &mut report)?;
    Ok(report)
}

fn summarize(
    state: IntegrationStateRef,
    receipts: &[&PhaseReceipt],
    target: &TargetIdentity,
    minimum_attempts: usize,
) -> StateSummary {
    let mut summary = StateSummary {
        state,
        evidence: StateEvidence::Unobserved,
        receipt_ids: Vec::new(),
        passing_attempts: 0,
        target_failures: 0,
        other_failures: 0,
        operational_attempts: 0,
    };
    for receipt in receipts {
        summary.receipt_ids.push(receipt.attempt_id.clone());
        match receipt.conclusion {
            PhaseConclusion::Passed => summary.passing_attempts += 1,
            PhaseConclusion::Failed
                if receipt.failure_signature.as_deref()
                    == Some(target.failure_signature.as_str()) =>
            {
                summary.target_failures += 1;
            }
            PhaseConclusion::Failed => summary.other_failures += 1,
            _ => summary.operational_attempts += 1,
        }
    }
    summary.evidence = if summary.passing_attempts > 0 && summary.target_failures > 0 {
        StateEvidence::Unstable
    } else if summary.other_failures > 0 {
        StateEvidence::OtherFailure
    } else if summary.passing_attempts >= minimum_attempts {
        StateEvidence::Passing
    } else if summary.target_failures >= minimum_attempts {
        StateEvidence::TargetFailing
    } else if summary.passing_attempts > 0 || summary.target_failures > 0 {
        StateEvidence::Incomplete
    } else if summary.operational_attempts > 0 {
        StateEvidence::Operational
    } else {
        StateEvidence::Unobserved
    };
    summary
}

fn propose(report: &mut DiagnosisReport, ordinal: usize, reason: &str) -> Result<()> {
    let state = report
        .states
        .get(ordinal)
        .ok_or_else(|| invalid("probe plan ordinal is out of bounds"))?;
    let minimum = report
        .policy
        .required_equivalence
        .max(StateEquivalence::ExactTree);
    if !state.state.equivalence.satisfies(minimum) {
        report
            .reason_codes
            .push("insufficient_state_equivalence".into());
    } else if !report.pending_attempts.is_empty() {
        report.status = DiagnosisStatus::AwaitingEvidence;
        report.reason_codes.push("pending_launch_receipts".into());
    } else if report.remaining_launches == 0 {
        report.status = DiagnosisStatus::BudgetExhausted;
        report.reason_codes.push("launch_budget_exhausted".into());
    } else {
        report.status = DiagnosisStatus::NeedsEvidence;
        report.next_probe = Some(ProbePlan {
            state: state.state.clone(),
            target: report.target.clone(),
            snapshot_id: report.snapshot_id.clone(),
            maximum_new_launches: 1,
            reason: reason.into(),
        });
    }
    Ok(())
}

fn observed_reversals(input: &DiagnosisInput, states: &[StateSummary]) -> Vec<IntegrationReversal> {
    let mut reversals = Vec::new();
    let mut previous_failure: Option<&StateSummary> = None;
    for summary in states {
        // Process passing samples first: one flaky state is not its own reversal.
        if summary.passing_attempts > 0
            && let Some(failing) = previous_failure.take()
        {
            reversals.push(IntegrationReversal {
                failing_state: failing.state.clone(),
                later_passing_state: summary.state.clone(),
                transitions: input
                    .sequence
                    .states
                    .iter()
                    .filter(|state| {
                        state.ordinal > failing.state.ordinal
                            && state.ordinal <= summary.state.ordinal
                    })
                    .filter_map(|state| state.transition.clone())
                    .collect(),
            });
        }
        if summary.target_failures > 0 {
            previous_failure = Some(summary);
        }
    }
    reversals
}

fn plan(input: &DiagnosisInput, report: &mut DiagnosisReport) -> Result<()> {
    if !report.pending_attempts.is_empty() {
        report.status = DiagnosisStatus::AwaitingEvidence;
        report.reason_codes.push("pending_launch_receipts".into());
        return Ok(());
    }
    let first_failure = report
        .states
        .iter()
        .position(|s| s.evidence == StateEvidence::TargetFailing);
    let last_pass = report
        .states
        .iter()
        .rposition(|s| s.evidence == StateEvidence::Passing);
    // Even an incomplete repeated-sample set can contain contrary evidence.
    let observed_failure = report.states.iter().position(|s| s.target_failures > 0);
    let observed_later_pass = report.states.iter().rposition(|s| s.passing_attempts > 0);
    if let (Some(failure), Some(pass)) = (observed_failure, observed_later_pass)
        && failure < pass
    {
        report.status = DiagnosisStatus::NonMonotonic;
        let failing = report
            .states
            .get(failure)
            .ok_or_else(|| invalid("missing contrary failure state"))?;
        let passing = report
            .states
            .get(pass)
            .ok_or_else(|| invalid("missing contrary passing state"))?;
        let mut outcome = input.sequence.map_localization_outcome(
            &LocalizationOutcome::SuspectWindow {
                lower_bound_exclusive: failing.state.commit.clone(),
                upper_bound_inclusive: passing.state.commit.clone(),
                confidence: Confidence::low(),
                reasons: vec![AmbiguityReason::NonMonotonicEvidence],
            },
            input.policy.required_equivalence,
        )?;
        if let IntegrationOutcome::NonMonotonic { reversals, .. } = &mut outcome {
            *reversals = observed_reversals(input, &report.states);
        }
        report.outcome = Some(outcome);
        report
            .reason_codes
            .push("non_monotonic_target_evidence".into());
        return Ok(());
    }
    let last = report.states.len().saturating_sub(1);
    let head = report
        .states
        .get(last)
        .ok_or_else(|| invalid("missing integration head"))?;
    match head.evidence {
        StateEvidence::Passing => {
            report.status = DiagnosisStatus::NoTargetFailure;
            report
                .reason_codes
                .push("target_not_reproduced_at_head".into());
            return Ok(());
        }
        StateEvidence::Unobserved | StateEvidence::Incomplete => {
            // A requested failure signature is not evidence of a red group.
            if head.target_failures == 0 {
                report
                    .reason_codes
                    .push("target_failure_not_established_at_head".into());
                return Ok(());
            }
            return propose(report, last, "reproduce_target_at_group_head");
        }
        StateEvidence::TargetFailing => {}
        _ => {
            report.reason_codes.push("head_evidence_ineligible".into());
            return Ok(());
        }
    }
    let base = report
        .states
        .first()
        .ok_or_else(|| invalid("missing base"))?;
    match base.evidence {
        StateEvidence::Passing => {}
        StateEvidence::Unobserved | StateEvidence::Incomplete => {
            return propose(report, 0, "establish_passing_base");
        }
        StateEvidence::TargetFailing => {
            report.reason_codes.push("baseline_already_failing".into());
            return Ok(());
        }
        _ => {
            report.reason_codes.push("base_evidence_ineligible".into());
            return Ok(());
        }
    }
    let has_unstable = report
        .states
        .iter()
        .any(|s| s.evidence == StateEvidence::Unstable);
    if has_unstable {
        // Only the declared monotonic policy can exclude an unobserved
        // earlier prefix from a first-failure suspect interval.
        let lower = if input.policy.assume_monotonic {
            last_pass.unwrap_or(0)
        } else {
            report
                .states
                .iter()
                .take_while(|s| s.evidence == StateEvidence::Passing)
                .count()
                .saturating_sub(1)
        };
        let upper = first_failure.ok_or_else(|| invalid("missing stable failing bound"))?;
        let base = input
            .sequence
            .states
            .get(lower)
            .ok_or_else(|| invalid("missing stable passing bound"))?;
        let head = input
            .sequence
            .states
            .get(upper)
            .ok_or_else(|| invalid("missing stable failing bound"))?;
        report.status = DiagnosisStatus::Suspect;
        report.outcome = Some(input.sequence.map_localization_outcome(
            &LocalizationOutcome::SuspectWindow {
                lower_bound_exclusive: base.commit.clone(),
                upper_bound_inclusive: head.commit.clone(),
                confidence: Confidence::low(),
                reasons: vec![AmbiguityReason::IndeterminateRevision],
            },
            input.policy.required_equivalence,
        )?);
        report.reason_codes.push("conflicting_attempts".into());
        return Ok(());
    }
    let upper = first_failure.ok_or_else(|| invalid("missing failing head after validation"))?;
    let prefix_passes = report
        .states
        .iter()
        .take_while(|s| s.evidence == StateEvidence::Passing)
        .count();
    let lower = if input.policy.assume_monotonic {
        last_pass.unwrap_or(0)
    } else {
        prefix_passes.saturating_sub(1)
    };
    let prefix_complete = prefix_passes == upper;
    let basis = if prefix_complete {
        LocalizationBasis::CompletePrefixObserved
    } else if input.policy.assume_monotonic {
        LocalizationBasis::MonotonicTargetAssumed
    } else {
        LocalizationBasis::Unspecified
    };
    let low_state = input
        .sequence
        .states
        .get(lower)
        .ok_or_else(|| invalid("missing lower state"))?;
    let high_state = input
        .sequence
        .states
        .get(upper)
        .ok_or_else(|| invalid("missing upper state"))?;
    let clean_adjacent = upper == lower + 1;
    let outcome = if clean_adjacent {
        LocalizationOutcome::FirstBad {
            last_good: low_state.commit.clone(),
            first_bad: high_state.commit.clone(),
            confidence: Confidence::high(),
        }
    } else {
        LocalizationOutcome::SuspectWindow {
            lower_bound_exclusive: low_state.commit.clone(),
            upper_bound_inclusive: high_state.commit.clone(),
            confidence: Confidence::low(),
            reasons: vec![AmbiguityReason::NeedsMoreProbes],
        }
    };
    report.outcome = Some(input.sequence.map_localization_outcome_with_basis(
        &outcome,
        input.policy.required_equivalence,
        basis,
    )?);
    report.status = if matches!(
        report.outcome,
        Some(IntegrationOutcome::FirstFailingTransition { .. })
    ) {
        DiagnosisStatus::Localized
    } else {
        DiagnosisStatus::Suspect
    };
    if report.status == DiagnosisStatus::Localized {
        return Ok(());
    }
    let minimum = input
        .policy
        .required_equivalence
        .max(StateEquivalence::ExactTree);
    let candidate = if input.policy.assume_monotonic {
        // The legacy engine's limit counts observations. Disable that limit for
        // this bounded replay; the charged-launch ledger is enforced separately.
        let mut session = LocalizationSession::new(
            input.sequence.revision_sequence()?,
            SearchPolicy {
                max_probes: input.sequence.len().saturating_add(1),
                ..SearchPolicy::default()
            },
        )?;
        for summary in &report.states {
            // An unusable state remains a gap, not a reason to stop probing
            // other eligible states or to upgrade its equivalence.
            let class = match (
                summary.state.equivalence.satisfies(minimum),
                summary.evidence,
            ) {
                (false, _) => ObservationClass::Indeterminate,
                (true, StateEvidence::Passing) => ObservationClass::Pass,
                (true, StateEvidence::TargetFailing) => ObservationClass::Fail,
                (true, StateEvidence::Unobserved | StateEvidence::Incomplete) => continue,
                _ => ObservationClass::Indeterminate,
            };
            session.record(ProbeObservation {
                commit: summary.state.commit.clone(),
                class,
                kind: ProbeKind::Custom,
                exit_code: None,
                timed_out: false,
                duration_ms: 0,
                stdout: String::new(),
                stderr: String::new(),
                sequence_index: 0,
                signal_number: None,
                probe_command: String::new(),
                working_dir: String::new(),
                flake_signal: None,
            })?;
        }
        session.next_probe().and_then(|commit| {
            input
                .sequence
                .states
                .iter()
                .position(|state| state.commit == commit)
        })
    } else {
        report.states.iter().take(upper).position(|s| {
            s.state.equivalence.satisfies(minimum)
                && matches!(
                    s.evidence,
                    StateEvidence::Unobserved | StateEvidence::Incomplete
                )
        })
    };
    if let Some(ordinal) = candidate {
        propose(report, ordinal, "narrow_target_transition")?;
    } else {
        report
            .reason_codes
            .push("interior_evidence_ineligible".into());
    }
    Ok(())
}
