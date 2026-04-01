//! 15.4: Export surfaces scenario
//! Validates: Requirements 8.1
//!
//! Generate SARIF + JUnit from the same `AnalysisReport` with suspect_surface
//! entries and verify both outputs contain consistent suspect surface data.

use faultline_codes::{ObservationClass, ProbeKind};
use faultline_junit::to_junit_xml;
use faultline_sarif::to_sarif;
use faultline_types::*;
use std::path::PathBuf;

fn report_with_suspect_surface() -> AnalysisReport {
    AnalysisReport {
        schema_version: "0.2.0".into(),
        run_id: "export-surfaces-run".into(),
        created_at_epoch_seconds: 1700000000,
        request: AnalysisRequest {
            repo_root: PathBuf::from("/tmp/repo"),
            good: RevisionSpec("good111".into()),
            bad: RevisionSpec("bad222".into()),
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
            revisions: vec![CommitId("good111".into()), CommitId("bad222".into())],
        },
        observations: vec![
            ProbeObservation {
                commit: CommitId("good111".into()),
                class: ObservationClass::Pass,
                kind: ProbeKind::Test,
                exit_code: Some(0),
                timed_out: false,
                duration_ms: 100,
                stdout: "ok".into(),
                stderr: String::new(),
                sequence_index: 0,
                signal_number: None,
                probe_command: String::new(),
                working_dir: String::new(),
                flake_signal: None,
            },
            ProbeObservation {
                commit: CommitId("bad222".into()),
                class: ObservationClass::Fail,
                kind: ProbeKind::Test,
                exit_code: Some(1),
                timed_out: false,
                duration_ms: 80,
                stdout: String::new(),
                stderr: "fail".into(),
                sequence_index: 1,
                signal_number: None,
                probe_command: String::new(),
                working_dir: String::new(),
                flake_signal: None,
            },
        ],
        outcome: LocalizationOutcome::FirstBad {
            last_good: CommitId("good111".into()),
            first_bad: CommitId("bad222".into()),
            confidence: Confidence::high(),
        },
        changed_paths: vec![
            PathChange {
                status: ChangeStatus::Modified,
                path: "src/lib.rs".into(),
            },
            PathChange {
                status: ChangeStatus::Deleted,
                path: "src/old_module.rs".into(),
            },
        ],
        surface: SurfaceSummary {
            total_changes: 2,
            buckets: vec![],
            execution_surfaces: vec![],
        },
        suspect_surface: vec![
            SuspectEntry {
                path: "src/lib.rs".into(),
                priority_score: 150,
                surface_kind: "source".into(),
                change_status: ChangeStatus::Modified,
                is_execution_surface: false,
                owner_hint: Some("bob".into()),
            },
            SuspectEntry {
                path: "src/old_module.rs".into(),
                priority_score: 250,
                surface_kind: "source".into(),
                change_status: ChangeStatus::Deleted,
                is_execution_surface: false,
                owner_hint: None,
            },
        ],
        reproduction_capsules: vec![],
    }
}

#[test]
fn scenario_export_surfaces_sarif_and_junit_consistency() {
    let report = report_with_suspect_surface();

    // Generate SARIF
    let sarif_json = to_sarif(&report).expect("SARIF generation must succeed");
    let sarif: serde_json::Value =
        serde_json::from_str(&sarif_json).expect("SARIF must be valid JSON");

    // Generate JUnit
    let junit_xml = to_junit_xml(&report);

    // --- Verify SARIF contains suspect surface paths ---
    let results = sarif["runs"][0]["results"].as_array().unwrap();

    // There should be a suspect-surface result with locations for each suspect path
    let suspect_result = results
        .iter()
        .find(|r| r["ruleId"] == "faultline/suspect-surface");
    assert!(
        suspect_result.is_some(),
        "SARIF must contain a faultline/suspect-surface result"
    );

    let suspect_locations = suspect_result.unwrap()["locations"].as_array().unwrap();
    let sarif_paths: Vec<&str> = suspect_locations
        .iter()
        .map(|loc| {
            loc["physicalLocation"]["artifactLocation"]["uri"]
                .as_str()
                .unwrap()
        })
        .collect();

    assert!(
        sarif_paths.contains(&"src/lib.rs"),
        "SARIF suspect surface must contain src/lib.rs"
    );
    assert!(
        sarif_paths.contains(&"src/old_module.rs"),
        "SARIF suspect surface must contain src/old_module.rs"
    );

    // --- Verify JUnit contains suspect surface paths ---
    assert!(
        junit_xml.contains("src/lib.rs"),
        "JUnit output must contain suspect path src/lib.rs"
    );
    assert!(
        junit_xml.contains("src/old_module.rs"),
        "JUnit output must contain suspect path src/old_module.rs"
    );

    // --- Verify both exports reference the same suspect paths ---
    // The suspect paths in SARIF locations should match what appears in JUnit
    for path in &["src/lib.rs", "src/old_module.rs"] {
        assert!(
            sarif_paths.contains(path),
            "SARIF must contain suspect path: {}",
            path
        );
        assert!(
            junit_xml.contains(path),
            "JUnit must contain suspect path: {}",
            path
        );
    }
}
