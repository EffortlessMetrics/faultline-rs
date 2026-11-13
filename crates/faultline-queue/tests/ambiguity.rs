mod common;

use common::{add, endpoints, packet};
use faultline_integration::{IntegrationOutcome, StateEquivalence};
use faultline_queue::{DiagnosisInput, DiagnosisReport, DiagnosisStatus, EvidenceSource, diagnose};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn unstable_packet() -> Result<DiagnosisInput, Box<dyn std::error::Error>> {
    let mut input = packet(20)?;
    endpoints(&mut input)?;
    add(
        &mut input,
        10,
        "stable-pass",
        true,
        EvidenceSource::Provider,
    )?;
    add(
        &mut input,
        12,
        "conflict-pass",
        true,
        EvidenceSource::Provider,
    )?;
    add(
        &mut input,
        12,
        "conflict-fail",
        false,
        EvidenceSource::Provider,
    )?;
    add(
        &mut input,
        15,
        "stable-fail",
        false,
        EvidenceSource::Provider,
    )?;
    Ok(input)
}

fn window(report: &DiagnosisReport) -> Result<(usize, usize, usize), Box<dyn std::error::Error>> {
    match &report.outcome {
        Some(IntegrationOutcome::SuspectTransitions {
            lower_bound_exclusive,
            upper_bound_inclusive,
            transitions,
            ..
        }) => Ok((
            lower_bound_exclusive.ordinal,
            upper_bound_inclusive.ordinal,
            transitions.len(),
        )),
        _ => Err("expected a suspect interval".into()),
    }
}

fn reversal_pairs(
    report: &DiagnosisReport,
) -> Result<Vec<(usize, usize)>, Box<dyn std::error::Error>> {
    let value = serde_json::to_value(report)?;
    let reversals = value
        .pointer("/outcome/NonMonotonic/reversals")
        .and_then(serde_json::Value::as_array)
        .ok_or("missing observed reversals")?;
    reversals
        .iter()
        .map(|reversal| {
            let lower = reversal
                .pointer("/failing_state/ordinal")
                .and_then(serde_json::Value::as_u64)
                .ok_or("missing failing state")?;
            let upper = reversal
                .pointer("/later_passing_state/ordinal")
                .and_then(serde_json::Value::as_u64)
                .ok_or("missing later passing state")?;
            Ok((usize::try_from(lower)?, usize::try_from(upper)?))
        })
        .collect()
}

#[test]
fn non_monotonic_outcome_retains_the_observed_interval() -> TestResult {
    let mut input = packet(20)?;
    endpoints(&mut input)?;
    add(&mut input, 4, "fail4", false, EvidenceSource::Provider)?;
    add(&mut input, 8, "pass8", true, EvidenceSource::Provider)?;
    let report = diagnose(&input)?;
    assert_eq!(report.status, DiagnosisStatus::NonMonotonic);
    match &report.outcome {
        Some(IntegrationOutcome::NonMonotonic {
            lower_bound_exclusive,
            upper_bound_inclusive,
            transitions,
            ..
        }) => {
            assert_eq!(
                lower_bound_exclusive.as_ref().map(|s| s.ordinal),
                Some(4),
                "known failing state was discarded"
            );
            assert_eq!(upper_bound_inclusive.as_ref().map(|s| s.ordinal), Some(8));
            assert_eq!(transitions.len(), 4);
        }
        _ => return Err("missing non-monotonic outcome".into()),
    }
    assert_eq!(reversal_pairs(&report)?, vec![(4, 8)]);
    assert!(report.next_probe.is_none());
    Ok(())
}

#[test]
fn non_monotonic_outcome_preserves_multiple_reversal_segments() -> TestResult {
    let mut input = packet(20)?;
    endpoints(&mut input)?;
    for (ordinal, passing) in [(4, false), (8, true), (10, false), (12, true)] {
        add(
            &mut input,
            ordinal,
            &format!("sample-{ordinal}"),
            passing,
            EvidenceSource::Provider,
        )?;
    }
    let report = diagnose(&input)?;
    assert_eq!(reversal_pairs(&report)?, vec![(4, 8), (10, 12)]);
    let value = serde_json::to_value(&report)?;
    assert_eq!(
        value
            .pointer("/outcome/NonMonotonic/reversals/0/transitions")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(4)
    );
    assert_eq!(
        value
            .pointer("/outcome/NonMonotonic/reversals/1/transitions")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(2)
    );
    assert!(report.next_probe.is_none());
    Ok(())
}

#[test]
fn incomplete_samples_retain_reversals_without_claiming_stability() -> TestResult {
    let mut input = packet(20)?;
    input.policy.minimum_attempts = 2;
    add(&mut input, 4, "fail4", false, EvidenceSource::Provider)?;
    add(&mut input, 8, "pass8", true, EvidenceSource::Provider)?;
    let report = diagnose(&input)?;
    assert_eq!(report.status, DiagnosisStatus::NonMonotonic);
    assert_eq!(reversal_pairs(&report)?, vec![(4, 8)]);
    assert_eq!(report.states.get(4).ok_or("state")?.target_failures, 1);
    assert!(report.next_probe.is_none());
    Ok(())
}

#[test]
fn unstable_interior_retains_the_nearest_supported_bounds() -> TestResult {
    let report = diagnose(&unstable_packet()?)?;
    assert_eq!(report.status, DiagnosisStatus::Suspect);
    assert_eq!(
        window(&report)?,
        (10, 15, 5),
        "stable interior bounds were discarded"
    );
    assert!(report.reason_codes.contains(&"conflicting_attempts".into()));
    assert!(report.next_probe.is_none());
    Ok(())
}

#[test]
fn conservative_conflict_window_keeps_unobserved_earlier_prefix() -> TestResult {
    let mut input = unstable_packet()?;
    input.policy.assume_monotonic = false;
    let report = diagnose(&input)?;
    assert_eq!(
        window(&report)?,
        (0, 15, 15),
        "a sampled pass cannot prove the entire earlier prefix"
    );
    Ok(())
}

#[test]
fn conservative_conflict_window_can_use_a_complete_passing_prefix() -> TestResult {
    let mut input = unstable_packet()?;
    input.policy.assume_monotonic = false;
    for ordinal in 1..10 {
        add(
            &mut input,
            ordinal,
            &format!("prefix-{ordinal}"),
            true,
            EvidenceSource::Provider,
        )?;
    }
    assert_eq!(window(&diagnose(&input)?)?, (10, 15, 5));
    Ok(())
}

#[test]
fn weak_passing_evidence_cannot_tighten_a_conflict_window() -> TestResult {
    let mut input = unstable_packet()?;
    input
        .sequence
        .states
        .get_mut(10)
        .ok_or("state")?
        .equivalence = StateEquivalence::PatchEquivalent;
    let report = diagnose(&input)?;
    assert_eq!(window(&report)?, (0, 15, 15));
    assert_eq!(report.excluded_receipts.len(), 1);
    Ok(())
}

struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            eprintln!("ambiguity fixture cleanup failed: {error}");
        }
    }
}

#[test]
fn markdown_exposes_the_observed_reversal_without_blame() -> TestResult {
    let mut input = packet(20)?;
    endpoints(&mut input)?;
    add(&mut input, 4, "fail4", false, EvidenceSource::Provider)?;
    add(&mut input, 8, "pass8", true, EvidenceSource::Provider)?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "faultline-ambiguity-{}-{stamp}",
        std::process::id()
    ));
    fs::create_dir(&path)?;
    let scratch = Scratch(path);
    let source = scratch.0.join("input.json");
    fs::write(&source, serde_json::to_vec(&input)?)?;
    let output = Command::new(env!("CARGO_BIN_EXE_faultline-queue"))
        .args(["diagnose", "--input"])
        .arg(source)
        .args(["--format", "markdown"])
        .output()?;
    assert_eq!(output.status.code(), Some(2));
    let text = String::from_utf8(output.stdout)?;
    assert!(text.contains("Observed reversal: target failure at S4; passing sample at S8; 4 transitions between them."), "operator report lost the contrary samples");
    assert!(text.contains("No supported first-transition attribution."));
    Ok(())
}

#[test]
fn conflicting_attempts_never_become_a_single_transition_claim() -> TestResult {
    let report = diagnose(&unstable_packet()?)?;
    assert_eq!(report.status, DiagnosisStatus::Suspect);
    assert!(!matches!(
        report.outcome,
        Some(IntegrationOutcome::FirstFailingTransition { .. })
    ));
    assert!(report.next_probe.is_none());
    Ok(())
}
