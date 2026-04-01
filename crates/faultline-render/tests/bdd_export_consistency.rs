//! BDD-style integration tests verifying that all export formats
//! (Markdown, SARIF, JUnit XML) produce consistent, well-formed output
//! from the same `AnalysisReport` input.

use std::path::PathBuf;

use faultline_codes::*;
use faultline_types::*;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a complete `AnalysisReport` with every field populated.
/// Uses a `FirstBad` outcome by default.
fn full_report() -> AnalysisReport {
    AnalysisReport {
        schema_version: "0.1.0".into(),
        run_id: "bdd-run-1".into(),
        created_at_epoch_seconds: 1_700_000_000,
        request: AnalysisRequest {
            repo_root: PathBuf::from("/tmp/repo"),
            good: RevisionSpec("aaa1111".into()),
            bad: RevisionSpec("bbb2222".into()),
            history_mode: HistoryMode::AncestryPath,
            probe: ProbeSpec::Exec {
                kind: ProbeKind::Test,
                program: "cargo".into(),
                args: vec!["test".into()],
                env: vec![],
                timeout_seconds: 300,
            },
            policy: SearchPolicy::default(),
        },
        sequence: RevisionSequence {
            revisions: vec![
                CommitId("aaa1111".into()),
                CommitId("ccc3333".into()),
                CommitId("bbb2222".into()),
            ],
        },
        observations: vec![
            ProbeObservation {
                commit: CommitId("aaa1111".into()),
                class: ObservationClass::Pass,
                kind: ProbeKind::Test,
                exit_code: Some(0),
                timed_out: false,
                duration_ms: 120,
                stdout: "all tests passed".into(),
                stderr: String::new(),
                sequence_index: 0,
                signal_number: None,
                probe_command: "cargo test".into(),
                working_dir: "/tmp/repo".into(),
                flake_signal: None,
            },
            ProbeObservation {
                commit: CommitId("bbb2222".into()),
                class: ObservationClass::Fail,
                kind: ProbeKind::Test,
                exit_code: Some(1),
                timed_out: false,
                duration_ms: 95,
                stdout: String::new(),
                stderr: "assertion failed".into(),
                sequence_index: 2,
                signal_number: None,
                probe_command: "cargo test".into(),
                working_dir: "/tmp/repo".into(),
                flake_signal: None,
            },
        ],
        outcome: LocalizationOutcome::FirstBad {
            last_good: CommitId("aaa1111".into()),
            first_bad: CommitId("bbb2222".into()),
            confidence: Confidence::high(),
        },
        changed_paths: vec![
            PathChange {
                status: ChangeStatus::Modified,
                path: "src/main.rs".into(),
            },
            PathChange {
                status: ChangeStatus::Added,
                path: "src/new_module.rs".into(),
            },
        ],
        surface: SurfaceSummary {
            total_changes: 2,
            buckets: vec![SubsystemBucket {
                name: "src".into(),
                change_count: 2,
                paths: vec!["src/main.rs".into(), "src/new_module.rs".into()],
                surface_kinds: vec!["source".into()],
            }],
            execution_surfaces: vec!["src/main.rs".into()],
        },
        suspect_surface: vec![SuspectEntry {
            path: "src/main.rs".into(),
            priority_score: 90,
            surface_kind: "source".into(),
            change_status: ChangeStatus::Modified,
            is_execution_surface: true,
            owner_hint: Some("core-team".into()),
        }],
        reproduction_capsules: vec![ReproductionCapsule {
            commit: CommitId("bbb2222".into()),
            predicate: ProbeSpec::Exec {
                kind: ProbeKind::Test,
                program: "cargo".into(),
                args: vec!["test".into()],
                env: vec![],
                timeout_seconds: 300,
            },
            env: vec![],
            working_dir: "/tmp/repo".into(),
            timeout_seconds: 300,
        }],
    }
}

/// Build a minimal `AnalysisReport` with `Inconclusive` outcome and
/// no observations, no suspect surface, no changed paths, no capsules.
fn minimal_report() -> AnalysisReport {
    AnalysisReport {
        schema_version: "0.1.0".into(),
        run_id: "bdd-minimal".into(),
        created_at_epoch_seconds: 1_700_000_000,
        request: AnalysisRequest {
            repo_root: PathBuf::from("/tmp/repo"),
            good: RevisionSpec("aaa".into()),
            bad: RevisionSpec("bbb".into()),
            history_mode: HistoryMode::AncestryPath,
            probe: ProbeSpec::Exec {
                kind: ProbeKind::Test,
                program: "true".into(),
                args: vec![],
                env: vec![],
                timeout_seconds: 60,
            },
            policy: SearchPolicy::default(),
        },
        sequence: RevisionSequence { revisions: vec![] },
        observations: vec![],
        outcome: LocalizationOutcome::Inconclusive { reasons: vec![] },
        changed_paths: vec![],
        surface: SurfaceSummary {
            total_changes: 0,
            buckets: vec![],
            execution_surfaces: vec![],
        },
        suspect_surface: vec![],
        reproduction_capsules: vec![],
    }
}

// ---------------------------------------------------------------------------
// Scenario 1: All formats accept the same report without error
// ---------------------------------------------------------------------------

#[test]
fn given_a_complete_report_all_formats_succeed() {
    let report = full_report();

    // Markdown renders without panic
    let md = faultline_render::render_markdown(&report);
    assert!(!md.is_empty(), "Markdown output must not be empty");

    // SARIF renders without error
    let sarif = faultline_sarif::to_sarif(&report).expect("SARIF export must succeed");
    assert!(!sarif.is_empty(), "SARIF output must not be empty");

    // JUnit renders without panic
    let junit = faultline_junit::to_junit_xml(&report);
    assert!(!junit.is_empty(), "JUnit output must not be empty");
}

// ---------------------------------------------------------------------------
// Scenario 2: All formats handle an empty/minimal report
// ---------------------------------------------------------------------------

#[test]
fn given_a_minimal_inconclusive_report_all_formats_succeed() {
    let report = minimal_report();

    let md = faultline_render::render_markdown(&report);
    assert!(
        !md.is_empty(),
        "Markdown output must not be empty for minimal report"
    );

    let sarif =
        faultline_sarif::to_sarif(&report).expect("SARIF export must succeed for minimal report");
    assert!(
        !sarif.is_empty(),
        "SARIF output must not be empty for minimal report"
    );

    let junit = faultline_junit::to_junit_xml(&report);
    assert!(
        !junit.is_empty(),
        "JUnit output must not be empty for minimal report"
    );
}

// ---------------------------------------------------------------------------
// Scenario 3: Markdown contains outcome keyword
// ---------------------------------------------------------------------------

#[test]
fn given_a_first_bad_report_markdown_contains_first_bad_keyword() {
    let report = full_report();
    let md = faultline_render::render_markdown(&report);

    assert!(
        md.contains("FirstBad"),
        "Markdown for a FirstBad report must contain the keyword 'FirstBad'. Got:\n{md}"
    );
}

// ---------------------------------------------------------------------------
// Scenario 4: SARIF output is valid JSON
// ---------------------------------------------------------------------------

#[test]
fn given_a_complete_report_sarif_output_is_valid_json() {
    let report = full_report();
    let sarif = faultline_sarif::to_sarif(&report).expect("SARIF export must succeed");

    let parsed: serde_json::Value =
        serde_json::from_str(&sarif).expect("SARIF output must be valid JSON");

    // Sanity: top-level keys exist
    assert!(
        parsed.get("version").is_some(),
        "SARIF JSON must have a 'version' field"
    );
    assert!(
        parsed.get("runs").is_some(),
        "SARIF JSON must have a 'runs' field"
    );
}

// ---------------------------------------------------------------------------
// Scenario 5: JUnit output starts with XML declaration
// ---------------------------------------------------------------------------

#[test]
fn given_a_complete_report_junit_output_starts_with_xml() {
    let report = full_report();
    let junit = faultline_junit::to_junit_xml(&report);

    let trimmed = junit.trim_start();
    assert!(
        trimmed.starts_with("<?xml") || trimmed.starts_with("<testsuites"),
        "JUnit output must start with '<?xml' or '<testsuites>'. Got:\n{}",
        &trimmed[..trimmed.len().min(200)]
    );
}
