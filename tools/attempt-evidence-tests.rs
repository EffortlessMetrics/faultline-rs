use faultline_app::{FaultlineApp, LocalizeOptions};
use faultline_codes::{ObservationClass, ProbeKind};
use faultline_ports::{CheckoutPort, HistoryPort, ProbePort, RunStorePort};
use faultline_store::FileRunStore;
use faultline_types::{AnalysisRequest, CheckedOutRevision, CommitId, FaultlineError, FlakePolicy, HistoryMode, LocalizationOutcome, PathChange, ProbeObservation, ProbeSpec, RedactionPolicy, Result, RevisionSequence, RevisionSpec, SearchPolicy, ShellKind, aggregate_attempts, aggregate_projection};
use std::cell::Cell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;
struct History;
impl HistoryPort for History {
    fn linearize(&self, _: &RevisionSpec, _: &RevisionSpec, _: HistoryMode) -> Result<RevisionSequence> {
        Ok(RevisionSequence { revisions: (0..3).map(|n| CommitId(format!("c{n}"))).collect() })
    }
    fn changed_paths(&self, _: &CommitId, _: &CommitId) -> Result<Vec<PathChange>> { Ok(Vec::new()) }
    fn codeowners_for_paths(&self, _: &[String]) -> Result<HashMap<String, Option<String>>> { Ok(HashMap::new()) }
    fn blame_frequency(&self, _: &[String]) -> Result<HashMap<String, Option<String>>> { Ok(HashMap::new()) }
}
struct Checkout { fail_cleanup: bool, cleaned: Cell<usize> }
impl Checkout {
    fn new(fail_cleanup: bool) -> Self { Self { fail_cleanup, cleaned: Cell::new(0) } }
}
impl CheckoutPort for Checkout {
    fn checkout_revision(&self, commit: &CommitId) -> Result<CheckedOutRevision> {
        Ok(CheckedOutRevision { commit: commit.clone(), path: PathBuf::from("fixture") })
    }
    fn cleanup_checkout(&self, _: &CheckedOutRevision) -> Result<()> {
        self.cleaned.set(self.cleaned.get() + 1);
        if self.fail_cleanup { Err(FaultlineError::Git("injected cleanup error".into())) } else { Ok(()) }
    }
}
struct Probe { calls: Cell<usize>, fail_at: usize, mixed: Option<&'static str> }
impl Probe {
    fn new(fail_at: usize, mixed: Option<&'static str>) -> Self { Self { calls: Cell::new(0), fail_at, mixed } }
}
impl ProbePort for Probe {
    fn run(&self, checkout: &CheckedOutRevision, _: &ProbeSpec) -> Result<ProbeObservation> {
        let call = self.calls.get() + 1;
        self.calls.set(call);
        if call == self.fail_at { return Err(FaultlineError::Probe("injected interrupted retry".into())); }
        let class = if self.mixed == Some(checkout.commit.0.as_str()) {
            if call % 3 == 2 { ObservationClass::Fail } else { ObservationClass::Pass }
        } else if checkout.commit.0 == "c0" { ObservationClass::Pass } else { ObservationClass::Fail };
        Ok(raw(&checkout.commit.0, class, u64::try_from(call).map_err(|e| FaultlineError::Probe(e.to_string()))?))
    }
}
fn raw(commit: &str, class: ObservationClass, index: u64) -> ProbeObservation {
    ProbeObservation { commit: CommitId(commit.into()), class, kind: ProbeKind::Test,
        exit_code: Some(if class == ObservationClass::Pass { 0 } else { 1 }), timed_out: false,
        duration_ms: index + 10, stdout: format!("original-{index}-{class:?}"), stderr: format!("stderr-{index}"),
        sequence_index: index, signal_number: None, probe_command: "fixture predicate".into(), working_dir: "fixture".into(), flake_signal: None }
}
fn request(root: &Path) -> AnalysisRequest {
    AnalysisRequest { repo_root: root.into(), good: RevisionSpec("c0".into()), bad: RevisionSpec("c2".into()), history_mode: HistoryMode::FirstParent,
        probe: ProbeSpec::Shell { kind: ProbeKind::Test, shell: ShellKind::Default, script: "fixture".into(), env: Vec::new(), timeout_seconds: 5 },
        policy: SearchPolicy { max_probes: 20, flake_policy: FlakePolicy { retries: 2, stability_threshold: 0.5 } } }
}

#[test]
fn retry_attempts_survive_store_reopen_with_original_metadata() -> TestResult {
    let temp = TempDir::new()?;
    let root = temp.path().join("runs");
    let store = FileRunStore::new(&root)?;
    let probe = Probe::new(0, Some("c1"));
    let checkout = Checkout::new(false);
    let result = FaultlineApp::new(&History, &checkout, &probe, &store).localize(request(temp.path()))?;
    assert_eq!(probe.calls.get(), 9);
    assert_eq!(result.report.probe_attempts.len(), 9);
    let reopened = FileRunStore::new(root)?;
    assert_eq!(reopened.load_probe_attempts(&result.run)?, result.report.probe_attempts);
    let middle: Vec<_> = result.report.probe_attempts.iter().filter(|a| a.commit.0 == "c1").collect();
    assert_eq!(middle.iter().map(|a| a.class).collect::<Vec<_>>(), vec![ObservationClass::Pass, ObservationClass::Fail, ObservationClass::Pass]);
    assert_eq!(middle.iter().map(|a| a.exit_code).collect::<Vec<_>>(), vec![Some(0), Some(1), Some(0)]);
    assert!(middle.iter().all(|a| a.flake_signal.is_none() && a.stdout.contains("original-")));
    assert!(!matches!(result.report.outcome, LocalizationOutcome::FirstBad { .. }));
    assert_eq!(checkout.cleaned.get(), 9);
    Ok(())
}

#[test]
fn aggregate_never_borrows_first_attempt_process_metadata() -> TestResult {
    let original = vec![raw("c1", ObservationClass::Pass, 1), raw("c1", ObservationClass::Fail, 2), raw("c1", ObservationClass::Fail, 3)];
    let before = original.clone();
    let aggregate = aggregate_attempts(&original)?;
    let projection = aggregate_projection(&original)?;
    assert_eq!(aggregate.class, ObservationClass::Indeterminate);
    assert_eq!(aggregate.attempt_ids, vec![1, 2, 3]);
    assert_eq!(aggregate.counts.fail_count, 2);
    assert!(!aggregate.counts.is_stable);
    assert_eq!(projection.exit_code, None);
    assert_eq!(projection.duration_ms, 0);
    assert!(projection.stdout.is_empty() && projection.stderr.is_empty() && projection.probe_command.is_empty());
    assert_eq!(original, before);
    Ok(())
}

#[test]
fn verified_journal_resume_does_not_reexecute_completed_batches() -> TestResult {
    let temp = TempDir::new()?;
    let root = temp.path().join("runs");
    let store = FileRunStore::new(&root)?;
    let checkout = Checkout::new(false);
    let first = FaultlineApp::new(&History, &checkout, &Probe::new(0, None), &store).localize(request(temp.path()))?;
    let reopened = FileRunStore::new(&root)?;
    let forbidden = Probe::new(1, None);
    let second = FaultlineApp::new(&History, &checkout, &forbidden, &reopened).localize(request(temp.path()))?;
    assert_eq!(forbidden.calls.get(), 0);
    assert_eq!(second.report.probe_attempts, first.report.probe_attempts);
    assert_eq!(second.report.outcome, first.report.outcome);
    assert_eq!(second.report.probe_aggregates, first.report.probe_aggregates);
    Ok(())
}

#[test]
fn completed_attempt_survives_later_retry_error_and_recollection() -> TestResult {
    let temp = TempDir::new()?;
    let root = temp.path().join("runs");
    let store = FileRunStore::new(&root)?;
    let req = request(temp.path());
    let checkout = Checkout::new(false);
    let interrupted = Probe::new(2, None);
    assert!(FaultlineApp::new(&History, &checkout, &interrupted, &store).localize(req.clone()).is_err());
    let run = store.prepare_run(&req)?;
    let before = store.load_probe_attempts(&run)?;
    assert_eq!(before.len(), 1);
    assert!(store.load_observations(&run)?.is_empty());
    assert_eq!(checkout.cleaned.get(), 2);
    let reopened = FileRunStore::new(root)?;
    let result = FaultlineApp::new(&History, &Checkout::new(false), &Probe::new(0, None), &reopened).localize(req)?;
    assert_eq!(result.report.probe_attempts.first(), before.first());
    assert_eq!(result.report.probe_attempts.len(), 10);
    assert!(result.report.probe_aggregates.iter().all(|a| !a.attempt_ids.contains(&0)));
    Ok(())
}

#[test]
fn cleanup_failure_does_not_discard_a_completed_attempt() -> TestResult {
    let temp = TempDir::new()?;
    let store = FileRunStore::new(temp.path().join("runs"))?;
    let req = request(temp.path());
    let checkout = Checkout::new(true);
    assert!(FaultlineApp::new(&History, &checkout, &Probe::new(0, None), &store).localize(req.clone()).is_err());
    let run = store.prepare_run(&req)?;
    assert_eq!(store.load_probe_attempts(&run)?.len(), 1);
    assert!(store.load_observations(&run)?.is_empty());
    assert_eq!(checkout.cleaned.get(), 1);
    Ok(())
}

#[test]
fn journal_is_idempotent_but_never_rewrites_an_attempt() -> TestResult {
    let temp = TempDir::new()?;
    let store = FileRunStore::new(temp.path().join("runs"))?;
    let run = store.prepare_run(&request(temp.path()))?;
    let first = raw("c1", ObservationClass::Pass, 0);
    store.save_probe_attempt(&run, &first)?;
    store.save_probe_attempt(&run, &first)?;
    assert!(store.save_probe_attempt(&run, &raw("c1", ObservationClass::Fail, 0)).is_err());
    let second = raw("c1", ObservationClass::Fail, 1);
    store.save_probe_attempt(&run, &second)?;
    assert_eq!(store.load_probe_attempts(&run)?, vec![first, second]);
    store.clear_observations(&run)?;
    assert_eq!(store.load_probe_attempts(&run)?.len(), 2);
    Ok(())
}

#[test]
fn corrupt_attempt_record_blocks_execution_instead_of_being_skipped() -> TestResult {
    let temp = TempDir::new()?;
    let store = FileRunStore::new(temp.path().join("runs"))?;
    let req = request(temp.path());
    let run = store.prepare_run(&req)?;
    std::fs::create_dir(run.root.join("attempts"))?;
    std::fs::write(run.root.join("attempts/00000000000000000000.json"), b"{")?;
    let probe = Probe::new(0, None);
    assert!(FaultlineApp::new(&History, &Checkout::new(false), &probe, &store).localize(req).is_err());
    assert_eq!(probe.calls.get(), 0);
    Ok(())
}

#[test]
fn legacy_cache_without_original_attempts_requires_recollection() -> TestResult {
    let temp = TempDir::new()?;
    let store = FileRunStore::new(temp.path().join("runs"))?;
    let req = request(temp.path());
    let run = store.prepare_run(&req)?;
    store.save_observation(&run, &raw("c0", ObservationClass::Pass, 0))?;
    let probe = Probe::new(0, None);
    assert!(FaultlineApp::new(&History, &Checkout::new(false), &probe, &store).localize(req.clone()).is_err());
    assert_eq!(probe.calls.get(), 0);
    let result = FaultlineApp::new(&History, &Checkout::new(false), &probe, &store).localize_with_options(req, LocalizeOptions { force: true, ..LocalizeOptions::default() })?;
    assert_eq!(result.report.probe_attempts.len(), 9);
    Ok(())
}

#[test]
fn force_recollects_without_erasing_historical_attempts() -> TestResult {
    let temp = TempDir::new()?;
    let store = FileRunStore::new(temp.path().join("runs"))?;
    let req = request(temp.path());
    let before = FaultlineApp::new(&History, &Checkout::new(false), &Probe::new(0, None), &store).localize(req.clone())?;
    let after = FaultlineApp::new(&History, &Checkout::new(false), &Probe::new(0, None), &store).localize_with_options(req, LocalizeOptions { force: true, ..LocalizeOptions::default() })?;
    assert_eq!(after.report.probe_attempts.len(), 18);
    assert!(after.report.probe_attempts.starts_with(&before.report.probe_attempts));
    assert!(after.report.probe_aggregates.iter().flat_map(|a| &a.attempt_ids).all(|id| *id >= 9));
    Ok(())
}

#[test]
fn mixed_endpoint_is_inconclusive_even_with_a_permissive_legacy_threshold() -> TestResult {
    let temp = TempDir::new()?;
    let store = FileRunStore::new(temp.path().join("runs"))?;
    let probe = Probe::new(0, Some("c0"));
    let result = FaultlineApp::new(&History, &Checkout::new(false), &probe, &store).localize(request(temp.path()))?;
    assert!(matches!(result.report.outcome, LocalizationOutcome::Inconclusive { .. }));
    assert_eq!(probe.calls.get(), 6);
    assert_eq!(result.report.probe_attempts.len(), 6);
    Ok(())
}

#[test]
fn all_report_exports_preserve_attempts_and_redact_their_output() -> TestResult {
    let temp = TempDir::new()?;
    let store = FileRunStore::new(temp.path().join("runs"))?;
    let mut report = FaultlineApp::new(&History, &Checkout::new(false), &Probe::new(0, Some("c1")), &store).localize(request(temp.path()))?.report;
    let marker = "ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghij";
    report.probe_attempts.first_mut().ok_or("attempt")?.stdout = marker.into();
    let policy = RedactionPolicy::default_safe();
    let export = temp.path().join("export");
    faultline_render::ReportRenderer::new(&export).render_with_markdown(&report)?;
    for path in ["analysis.json", "index.html", "dossier.md"] {
        let content = std::fs::read_to_string(export.join(path))?;
        assert!(!content.contains(marker), "new attempt output leaked in {path}");
    }
    let json: serde_json::Value = serde_json::from_slice(&std::fs::read(export.join("analysis.json"))?)?;
    assert_eq!(json.get("probe_attempts").and_then(serde_json::Value::as_array).map(Vec::len), Some(9));
    for content in [faultline_sarif::to_sarif(&report, &policy)?, faultline_junit::to_junit_xml(&report, &policy)?] {
        assert!(!content.contains(marker));
        assert!(content.contains("Indeterminate"));
    }
    assert_eq!(report.probe_attempts.first().ok_or("attempt")?.stdout, marker);
    Ok(())
}

proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config::with_cases(128))]
    #[test]
    fn arbitrary_mixed_attempts_never_become_decisive(samples in proptest::collection::vec(proptest::bool::ANY, 1..32)) {
        let mut observations = Vec::new();
        for (index, passed) in samples.iter().enumerate() {
            observations.push(raw("c1", if *passed { ObservationClass::Pass } else { ObservationClass::Fail }, u64::try_from(index).map_err(|e| proptest::test_runner::TestCaseError::fail(e.to_string()))?));
        }
        let original = observations.clone();
        let aggregate = aggregate_attempts(&observations).map_err(|e| proptest::test_runner::TestCaseError::fail(e.to_string()))?;
        if samples.iter().any(|v| *v) && samples.iter().any(|v| !*v) {
            proptest::prop_assert_eq!(aggregate.class, ObservationClass::Indeterminate);
            proptest::prop_assert!(!aggregate.counts.is_stable);
        }
        proptest::prop_assert_eq!(observations, original);
        proptest::prop_assert_eq!(aggregate.attempt_ids.len(), samples.len());
    }
}
