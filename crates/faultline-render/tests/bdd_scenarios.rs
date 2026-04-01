//! BDD/scenario integration tests for faultline-render.
//!
//! - 15.1: Report generation end-to-end (app → render → verify artifacts)
//! - 15.2: Resume/rerender (load cached, re-render, verify consistency)

use faultline_codes::{ObservationClass, ProbeKind};
use faultline_render::ReportRenderer;
use faultline_types::*;
use std::path::PathBuf;
use tempfile::TempDir;

/// Build a realistic `AnalysisReport` using fixture-style construction.
fn build_sample_report() -> AnalysisReport {
    use faultline_fixtures::RevisionSequenceBuilder;

    let sequence = RevisionSequenceBuilder::new()
        .push("aaa1111111111111111111111111111111111111a")
        .push("bbb2222222222222222222222222222222222222b")
        .push("ccc3333333333333333333333333333333333333c")
        .build();

    AnalysisReport {
        schema_version: "0.2.0".into(),
        run_id: "bdd-e2e-run-001".into(),
        created_at_epoch_seconds: 1700000000,
        request: AnalysisRequest {
            repo_root: PathBuf::from("/tmp/repo"),
            good: RevisionSpec("aaa1111111111111111111111111111111111111a".into()),
            bad: RevisionSpec("ccc3333333333333333333333333333333333333c".into()),
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
        sequence,
        observations: vec![
            ProbeObservation {
                commit: CommitId("aaa1111111111111111111111111111111111111a".into()),
                class: ObservationClass::Pass,
                kind: ProbeKind::Test,
                exit_code: Some(0),
                timed_out: false,
                duration_ms: 120,
                stdout: "ok".into(),
                stderr: String::new(),
                sequence_index: 0,
                signal_number: None,
                probe_command: "cargo test".into(),
                working_dir: "/tmp/repo".into(),
                flake_signal: None,
            },
            ProbeObservation {
                commit: CommitId("ccc3333333333333333333333333333333333333c".into()),
                class: ObservationClass::Fail,
                kind: ProbeKind::Test,
                exit_code: Some(1),
                timed_out: false,
                duration_ms: 95,
                stdout: String::new(),
                stderr: "test failed".into(),
                sequence_index: 1,
                signal_number: None,
                probe_command: "cargo test".into(),
                working_dir: "/tmp/repo".into(),
                flake_signal: None,
            },
        ],
        outcome: LocalizationOutcome::FirstBad {
            last_good: CommitId("aaa1111111111111111111111111111111111111a".into()),
            first_bad: CommitId("ccc3333333333333333333333333333333333333c".into()),
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
            execution_surfaces: vec![],
        },
        suspect_surface: vec![
            SuspectEntry {
                path: "src/main.rs".into(),
                priority_score: 150,
                surface_kind: "source".into(),
                change_status: ChangeStatus::Modified,
                is_execution_surface: false,
                owner_hint: Some("alice".into()),
            },
            SuspectEntry {
                path: "src/new_module.rs".into(),
                priority_score: 100,
                surface_kind: "source".into(),
                change_status: ChangeStatus::Added,
                is_execution_surface: false,
                owner_hint: None,
            },
        ],
        reproduction_capsules: vec![ReproductionCapsule {
            commit: CommitId("ccc3333333333333333333333333333333333333c".into()),
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

// ---------------------------------------------------------------------------
// 15.1: Report generation end-to-end scenario
// Validates: Requirements 8.1
// ---------------------------------------------------------------------------

/// End-to-end: build report → render JSON + HTML → verify artifacts exist and
/// contain expected fields. Then render with markdown and verify dossier.md.
#[test]
fn scenario_report_generation_end_to_end() {
    let tmp = TempDir::new().unwrap();
    let output_dir = tmp.path().join("output");
    let renderer = ReportRenderer::new(&output_dir);
    let report = build_sample_report();

    // Render JSON + HTML
    renderer.render(&report).expect("render should succeed");

    // --- Verify analysis.json exists and contains expected fields ---
    let json_path = output_dir.join("analysis.json");
    assert!(json_path.exists(), "analysis.json must exist");

    let json_content = std::fs::read_to_string(&json_path).unwrap();
    let parsed: serde_json::Value =
        serde_json::from_str(&json_content).expect("analysis.json must be valid JSON");

    assert_eq!(parsed["run_id"], "bdd-e2e-run-001");
    assert_eq!(parsed["schema_version"], "0.2.0");
    assert!(parsed["request"].is_object(), "request field must exist");
    assert!(parsed["sequence"].is_object(), "sequence field must exist");
    assert!(
        parsed["observations"].is_array(),
        "observations must be array"
    );
    assert!(parsed["outcome"].is_object(), "outcome must exist");
    assert!(
        parsed["changed_paths"].is_array(),
        "changed_paths must be array"
    );
    assert!(parsed["surface"].is_object(), "surface must exist");
    assert!(
        parsed["suspect_surface"].is_array(),
        "suspect_surface must be array"
    );
    assert_eq!(parsed["suspect_surface"].as_array().unwrap().len(), 2);
    assert!(
        parsed["reproduction_capsules"].is_array(),
        "reproduction_capsules must be array"
    );

    // Verify suspect_surface entries have expected fields
    let suspect_0 = &parsed["suspect_surface"][0];
    assert_eq!(suspect_0["path"], "src/main.rs");
    assert_eq!(suspect_0["priority_score"], 150);
    assert_eq!(suspect_0["surface_kind"], "source");
    assert!(suspect_0["change_status"].is_string());
    assert!(suspect_0.get("is_execution_surface").is_some());
    assert_eq!(suspect_0["owner_hint"], "alice");

    // --- Verify index.html exists and contains key content ---
    let html_path = output_dir.join("index.html");
    assert!(html_path.exists(), "index.html must exist");

    let html_content = std::fs::read_to_string(&html_path).unwrap();
    assert!(
        html_content.contains("bdd-e2e-run-001"),
        "HTML must contain run_id"
    );
    assert!(
        html_content.contains("Exact boundary"),
        "HTML must contain outcome"
    );
    assert!(
        html_content.contains("src/main.rs"),
        "HTML must contain changed paths"
    );

    // --- Render with markdown and verify dossier.md ---
    let md_dir = tmp.path().join("output_md");
    let md_renderer = ReportRenderer::new(&md_dir);
    md_renderer
        .render_with_markdown(&report)
        .expect("render_with_markdown should succeed");

    let dossier_path = md_dir.join("dossier.md");
    assert!(
        dossier_path.exists(),
        "dossier.md must exist after render_with_markdown"
    );

    let md_content = std::fs::read_to_string(&dossier_path).unwrap();
    assert!(
        md_content.contains("FirstBad") || md_content.contains("Outcome"),
        "dossier.md must contain outcome section"
    );
    assert!(
        md_content.contains("src/main.rs"),
        "dossier.md must contain suspect surface paths"
    );

    // Also verify JSON + HTML exist in the markdown output dir
    assert!(md_dir.join("analysis.json").exists());
    assert!(md_dir.join("index.html").exists());
}

// ---------------------------------------------------------------------------
// 15.2: Resume/rerender scenario
// Validates: Requirements 8.1
// ---------------------------------------------------------------------------

/// Resume/rerender: create a report, serialize it, deserialize it back,
/// re-render, and verify the re-rendered output matches the original.
#[test]
fn scenario_resume_rerender_consistency() {
    let report = build_sample_report();

    // Serialize the report (simulating store persistence)
    let serialized = serde_json::to_string_pretty(&report).expect("serialize should succeed");

    // Deserialize (simulating loading from store cache)
    let deserialized: AnalysisReport =
        serde_json::from_str(&serialized).expect("deserialize should succeed");

    // Verify round-trip fidelity
    assert_eq!(report, deserialized, "round-trip must preserve report");

    // Render the original
    let tmp = TempDir::new().unwrap();
    let dir_original = tmp.path().join("original");
    let dir_rerendered = tmp.path().join("rerendered");

    ReportRenderer::new(&dir_original)
        .render_with_markdown(&report)
        .expect("original render should succeed");

    // Re-render from deserialized (cached) report
    ReportRenderer::new(&dir_rerendered)
        .render_with_markdown(&deserialized)
        .expect("rerender should succeed");

    // Verify JSON artifacts are identical
    let json_original = std::fs::read_to_string(dir_original.join("analysis.json")).unwrap();
    let json_rerendered = std::fs::read_to_string(dir_rerendered.join("analysis.json")).unwrap();
    assert_eq!(
        json_original, json_rerendered,
        "re-rendered JSON must match original"
    );

    // Verify HTML artifacts are identical
    let html_original = std::fs::read_to_string(dir_original.join("index.html")).unwrap();
    let html_rerendered = std::fs::read_to_string(dir_rerendered.join("index.html")).unwrap();
    assert_eq!(
        html_original, html_rerendered,
        "re-rendered HTML must match original"
    );

    // Verify Markdown dossiers are identical
    let md_original = std::fs::read_to_string(dir_original.join("dossier.md")).unwrap();
    let md_rerendered = std::fs::read_to_string(dir_rerendered.join("dossier.md")).unwrap();
    assert_eq!(
        md_original, md_rerendered,
        "re-rendered Markdown must match original"
    );
}
