mod common;

use common::{add, endpoints, packet};
use faultline_integration::{ChangeUnit, StateEquivalence};
use faultline_queue::{
    DiagnosisInput, DiagnosisReport, DiagnosisStatus, EvidenceSource, PhaseConclusion,
    SCHEMA_VERSION, diagnose,
};
use faultline_types::CommitId;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

type TestResult = Result<(), Box<dyn std::error::Error>>;
static NEXT: AtomicU64 = AtomicU64::new(0);

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "faultline-review-{}-{timestamp}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path)?;
        Ok(Self(path))
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            eprintln!("review fixture cleanup failed: {error}");
        }
    }
}
fn render(input: &DiagnosisInput) -> Result<String, Box<dyn std::error::Error>> {
    let scratch = Scratch::new()?;
    let path = scratch.0.join("input.json");
    fs::write(&path, serde_json::to_vec(input)?)?;
    let result = Command::new(env!("CARGO_BIN_EXE_faultline-queue"))
        .args(["diagnose", "--input"])
        .arg(path)
        .args(["--format", "markdown"])
        .output()?;
    assert!(
        matches!(result.status.code(), Some(0 | 2)),
        "unexpected CLI failure: {result:?}"
    );
    Ok(String::from_utf8(result.stdout)?)
}

#[test]
fn terminal_ineligible_native_receipt_does_not_block_replanning() -> TestResult {
    for field in ["context", "phase", "snapshot", "equivalence"] {
        let mut input = packet(20)?;
        endpoints(&mut input)?;
        add(&mut input, 10, "ineligible", true, EvidenceSource::Native)?;
        let receipt = input.receipts.last_mut().ok_or("receipt")?;
        match field {
            "context" => receipt.context_id = "wrong-context".into(),
            "phase" => receipt.phase = "wrong-phase".into(),
            "snapshot" => receipt.snapshot_id = "wrong-snapshot".into(),
            _ => {
                input
                    .sequence
                    .states
                    .get_mut(10)
                    .ok_or("state")?
                    .equivalence = StateEquivalence::PatchEquivalent
            }
        }
        let report = diagnose(&input)?;
        assert!(
            report.pending_attempts.is_empty(),
            "terminal but excluded is not pending"
        );
        assert_eq!(report.excluded_receipts.len(), 1);
        assert_eq!(report.recorded_launches, 1);
        assert_eq!(report.remaining_launches, 4);
        assert_eq!(report.states.get(10).ok_or("state")?.passing_attempts, 0);
        assert!(
            report
                .reason_codes
                .contains(&"launch_receipt_ineligible".into())
        );
        assert_eq!(report.status, DiagnosisStatus::NeedsEvidence);
        assert!(report.next_probe.is_some());
    }
    Ok(())
}

#[test]
fn completed_ineligible_receipt_does_not_block_supported_boundary() -> TestResult {
    let mut input = packet(1)?;
    endpoints(&mut input)?;
    add(&mut input, 1, "wrong-context", true, EvidenceSource::Native)?;
    input.receipts.last_mut().ok_or("receipt")?.context_id = "other-context".into();
    let report = diagnose(&input)?;
    assert_eq!(
        report.status,
        DiagnosisStatus::Localized,
        "completed excluded work must not deadlock replay"
    );
    assert!(report.pending_attempts.is_empty());
    assert_eq!(report.recorded_launches, 1);
    assert_eq!(report.excluded_receipts.len(), 1);
    assert_eq!(report.states.get(1).ok_or("head")?.passing_attempts, 0);
    Ok(())
}

#[test]
fn excluded_missing_receipt_remains_pending() -> TestResult {
    let mut input = packet(1)?;
    endpoints(&mut input)?;
    add(
        &mut input,
        1,
        "still-running",
        false,
        EvidenceSource::Native,
    )?;
    let receipt = input.receipts.last_mut().ok_or("receipt")?;
    receipt.context_id = "other-context".into();
    receipt.conclusion = PhaseConclusion::Missing;
    receipt.exit_code = None;
    receipt.failure_signature = None;
    let report = diagnose(&input)?;
    assert_eq!(report.status, DiagnosisStatus::AwaitingEvidence);
    assert_eq!(report.pending_attempts, vec!["still-running"]);
    assert_eq!(report.recorded_launches, 1);
    assert!(report.next_probe.is_none());
    Ok(())
}

#[test]
fn version_schema_constrains_input_and_report() -> TestResult {
    for schema in [
        schemars::schema_for!(DiagnosisInput),
        schemars::schema_for!(DiagnosisReport),
    ] {
        let value = serde_json::to_value(schema)?;
        assert_eq!(
            value.pointer("/properties/schema_version/const"),
            Some(&serde_json::json!(SCHEMA_VERSION)),
            "library schema must match the runtime version"
        );
    }
    Ok(())
}

#[test]
fn cli_schemas_enforce_the_runtime_version() -> TestResult {
    for kind in ["input", "report"] {
        let output = Command::new(env!("CARGO_BIN_EXE_faultline-queue"))
            .args(["schema", kind])
            .output()?;
        assert!(output.status.success());
        let value: serde_json::Value = serde_json::from_slice(&output.stdout)?;
        assert_eq!(
            value.pointer("/properties/schema_version/const"),
            Some(&serde_json::json!(SCHEMA_VERSION)),
            "CLI schema must reject an unsupported version"
        );
    }
    Ok(())
}

#[test]
fn markdown_names_predicate_and_context() -> TestResult {
    let mut input = packet(1)?;
    input.target.predicate_id = "<predicate-v1>".into();
    input.target.context_id = "<context-v1>".into();
    endpoints(&mut input)?;
    let text = render(&input)?;
    assert!(
        text.contains("Predicate: &lt;predicate-v1&gt;"),
        "missing exact predicate"
    );
    assert!(
        text.contains("Context: &lt;context-v1&gt;"),
        "missing exact context"
    );
    assert!(!text.contains("<predicate-v1>"));
    assert!(!text.contains("<context-v1>"));
    Ok(())
}

#[test]
fn markdown_names_captured_pull_request_identity() -> TestResult {
    let mut input = packet(1)?;
    endpoints(&mut input)?;
    let text = render(&input)?;
    assert!(text.contains("Introduced PR: synthetic / fixture/queue #1 at captured head 0000000000000000000000000000000000000065."), "missing captured PR identity");
    Ok(())
}

#[test]
fn markdown_names_commit_transition() -> TestResult {
    let mut input = packet(1)?;
    let change = ChangeUnit::Commit {
        repository: "fixture/queue".into(),
        commit: CommitId("captured-commit-identity".into()),
    };
    let state = input.sequence.states.get_mut(1).ok_or("state")?;
    state.transition = Some(change.clone());
    state.included_changes = vec![change];
    endpoints(&mut input)?;
    let text = render(&input)?;
    assert!(
        text.contains("Introduced commit: fixture/queue at captured-commit-identity."),
        "commit transition identity is missing"
    );
    Ok(())
}

#[test]
fn markdown_lists_pending_and_excluded_receipts() -> TestResult {
    let mut input = packet(20)?;
    endpoints(&mut input)?;
    add(&mut input, 10, "<pending>", false, EvidenceSource::Native)?;
    let receipt = input.receipts.last_mut().ok_or("pending")?;
    receipt.conclusion = PhaseConclusion::Missing;
    receipt.exit_code = None;
    receipt.failure_signature = None;
    add(&mut input, 5, "<excluded>", false, EvidenceSource::Provider)?;
    input.receipts.last_mut().ok_or("excluded")?.context_id = "other-context".into();
    let text = render(&input)?;
    assert!(
        text.contains("Pending attempt: &lt;pending&gt;."),
        "pending attempt identity is missing"
    );
    assert!(
        text.contains("Excluded receipt: &lt;excluded&gt;"),
        "excluded receipt identity is missing"
    );
    assert!(text.contains(r"context\_mismatch"));
    assert!(!text.contains("<pending>"));
    assert!(!text.contains("<excluded>"));
    Ok(())
}

#[test]
fn native_receipt_cannot_settle_another_state() -> TestResult {
    let mut input = packet(20)?;
    endpoints(&mut input)?;
    add(&mut input, 10, "native", true, EvidenceSource::Native)?;
    input.receipts.last_mut().ok_or("receipt")?.commit = "not-the-charged-commit".into();
    assert!(diagnose(&input).is_err());
    Ok(())
}
