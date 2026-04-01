//! Backward-compatibility tests for the `AnalysisReport` schema.
//!
//! These tests ensure that:
//! - Older JSON payloads (v0.1.0, before product-sharpening fields) still
//!   deserialize into the current `AnalysisReport` type.
//! - Newer payloads with all v0.2.0 fields round-trip correctly.
//! - Unknown future fields are silently ignored (forward compatibility).
//! - The `schema_version` field is present and valid semver.

use faultline_types::{
    AnalysisReport, AnalysisRequest, ChangeStatus, CommitId, Confidence, FlakeSignal,
    LocalizationOutcome, PathChange, ProbeObservation, ProbeSpec, ReproductionCapsule,
    RevisionSequence, RevisionSpec, SearchPolicy, SubsystemBucket, SurfaceSummary, SuspectEntry,
};
use serde_json::Value;
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Minimal v0.1.0 report JSON -- no suspect_surface, reproduction_capsules,
/// flake_signal, schema_version, sequence_index, signal_number, probe_command,
/// or working_dir fields.
fn v010_json() -> &'static str {
    r#"{
        "run_id": "run-001",
        "created_at_epoch_seconds": 1700000000,
        "request": {
            "repo_root": "/tmp/repo",
            "good": "aaa",
            "bad": "bbb",
            "history_mode": "FirstParent",
            "probe": {
                "Exec": {
                    "kind": "Test",
                    "program": "cargo",
                    "args": ["test"],
                    "env": [],
                    "timeout_seconds": 300
                }
            },
            "policy": {
                "max_probes": 64,
                "flake_policy": {
                    "retries": 0,
                    "stability_threshold": 1.0
                }
            }
        },
        "sequence": {
            "revisions": ["aaa", "bbb"]
        },
        "observations": [
            {
                "commit": "aaa",
                "class": "Pass",
                "kind": "Test",
                "exit_code": 0,
                "timed_out": false,
                "duration_ms": 1200,
                "stdout": "",
                "stderr": ""
            },
            {
                "commit": "bbb",
                "class": "Fail",
                "kind": "Test",
                "exit_code": 1,
                "timed_out": false,
                "duration_ms": 800,
                "stdout": "",
                "stderr": "FAILED"
            }
        ],
        "outcome": {
            "FirstBad": {
                "last_good": "aaa",
                "first_bad": "bbb",
                "confidence": { "score": 95, "label": "high" }
            }
        },
        "changed_paths": [
            { "status": "Modified", "path": "src/main.rs" }
        ],
        "surface": {
            "total_changes": 1,
            "buckets": [
                {
                    "name": "src",
                    "change_count": 1,
                    "paths": ["src/main.rs"],
                    "surface_kinds": ["source"]
                }
            ],
            "execution_surfaces": ["src/main.rs"]
        }
    }"#
}

/// Full v0.2.0 report JSON with all product-sharpening fields present.
fn v020_json() -> &'static str {
    r#"{
        "schema_version": "0.2.0",
        "run_id": "run-002",
        "created_at_epoch_seconds": 1700000001,
        "request": {
            "repo_root": "/tmp/repo",
            "good": "aaa",
            "bad": "ccc",
            "history_mode": "AncestryPath",
            "probe": {
                "Shell": {
                    "kind": "Build",
                    "shell": "PosixSh",
                    "script": "make test",
                    "env": [["CI", "true"]],
                    "timeout_seconds": 600
                }
            },
            "policy": {
                "max_probes": 32,
                "flake_policy": {
                    "retries": 3,
                    "stability_threshold": 0.8
                }
            }
        },
        "sequence": {
            "revisions": ["aaa", "bbb", "ccc"]
        },
        "observations": [
            {
                "commit": "bbb",
                "class": "Pass",
                "kind": "Build",
                "exit_code": 0,
                "timed_out": false,
                "duration_ms": 5000,
                "stdout": "ok",
                "stderr": "",
                "sequence_index": 1,
                "signal_number": null,
                "probe_command": "make test",
                "working_dir": "/tmp/repo",
                "flake_signal": {
                    "total_runs": 3,
                    "pass_count": 3,
                    "fail_count": 0,
                    "skip_count": 0,
                    "indeterminate_count": 0,
                    "is_stable": true
                }
            },
            {
                "commit": "ccc",
                "class": "Fail",
                "kind": "Build",
                "exit_code": 2,
                "timed_out": false,
                "duration_ms": 3000,
                "stdout": "",
                "stderr": "error",
                "sequence_index": 2,
                "signal_number": null,
                "probe_command": "make test",
                "working_dir": "/tmp/repo",
                "flake_signal": {
                    "total_runs": 3,
                    "pass_count": 0,
                    "fail_count": 3,
                    "skip_count": 0,
                    "indeterminate_count": 0,
                    "is_stable": true
                }
            }
        ],
        "outcome": {
            "FirstBad": {
                "last_good": "bbb",
                "first_bad": "ccc",
                "confidence": { "score": 95, "label": "high" }
            }
        },
        "changed_paths": [
            { "status": "Added", "path": "src/new_file.rs" },
            { "status": "Modified", "path": "Cargo.toml" }
        ],
        "surface": {
            "total_changes": 2,
            "buckets": [
                {
                    "name": "src",
                    "change_count": 1,
                    "paths": ["src/new_file.rs"],
                    "surface_kinds": ["source"]
                },
                {
                    "name": "config",
                    "change_count": 1,
                    "paths": ["Cargo.toml"],
                    "surface_kinds": ["config"]
                }
            ],
            "execution_surfaces": ["src/new_file.rs"]
        },
        "suspect_surface": [
            {
                "path": "src/new_file.rs",
                "priority_score": 100,
                "surface_kind": "source",
                "change_status": "Added",
                "is_execution_surface": true,
                "owner_hint": "team-alpha"
            }
        ],
        "reproduction_capsules": [
            {
                "commit": "ccc",
                "predicate": {
                    "Shell": {
                        "kind": "Build",
                        "shell": "PosixSh",
                        "script": "make test",
                        "env": [],
                        "timeout_seconds": 600
                    }
                },
                "env": [["CI", "true"]],
                "working_dir": "/tmp/repo",
                "timeout_seconds": 600
            }
        ]
    }"#
}

/// Build a fully-populated `AnalysisReport` programmatically.
fn full_report() -> AnalysisReport {
    AnalysisReport {
        schema_version: "0.2.0".to_string(),
        run_id: "run-full".to_string(),
        created_at_epoch_seconds: 1700000099,
        request: AnalysisRequest {
            repo_root: PathBuf::from("/tmp/repo"),
            good: RevisionSpec("g00d".to_string()),
            bad: RevisionSpec("b4d0".to_string()),
            history_mode: faultline_types::HistoryMode::FirstParent,
            probe: ProbeSpec::Exec {
                kind: faultline_codes::ProbeKind::Test,
                program: "cargo".to_string(),
                args: vec!["test".to_string()],
                env: vec![],
                timeout_seconds: 300,
            },
            policy: SearchPolicy::default(),
        },
        sequence: RevisionSequence {
            revisions: vec![CommitId("g00d".to_string()), CommitId("b4d0".to_string())],
        },
        observations: vec![ProbeObservation {
            commit: CommitId("b4d0".to_string()),
            class: faultline_codes::ObservationClass::Fail,
            kind: faultline_codes::ProbeKind::Test,
            exit_code: Some(1),
            timed_out: false,
            duration_ms: 450,
            stdout: String::new(),
            stderr: "assertion failed".to_string(),
            sequence_index: 0,
            signal_number: None,
            probe_command: "cargo test".to_string(),
            working_dir: "/tmp/repo".to_string(),
            flake_signal: Some(FlakeSignal {
                total_runs: 1,
                pass_count: 0,
                fail_count: 1,
                skip_count: 0,
                indeterminate_count: 0,
                is_stable: true,
            }),
        }],
        outcome: LocalizationOutcome::FirstBad {
            last_good: CommitId("g00d".to_string()),
            first_bad: CommitId("b4d0".to_string()),
            confidence: Confidence::high(),
        },
        changed_paths: vec![PathChange {
            status: ChangeStatus::Modified,
            path: "lib.rs".to_string(),
        }],
        surface: SurfaceSummary {
            total_changes: 1,
            buckets: vec![SubsystemBucket {
                name: "src".to_string(),
                change_count: 1,
                paths: vec!["lib.rs".to_string()],
                surface_kinds: vec!["source".to_string()],
            }],
            execution_surfaces: vec!["lib.rs".to_string()],
        },
        suspect_surface: vec![SuspectEntry {
            path: "lib.rs".to_string(),
            priority_score: 90,
            surface_kind: "source".to_string(),
            change_status: ChangeStatus::Modified,
            is_execution_surface: true,
            owner_hint: Some("core-team".to_string()),
        }],
        reproduction_capsules: vec![ReproductionCapsule {
            commit: CommitId("b4d0".to_string()),
            predicate: ProbeSpec::Exec {
                kind: faultline_codes::ProbeKind::Test,
                program: "cargo".to_string(),
                args: vec!["test".to_string()],
                env: vec![],
                timeout_seconds: 300,
            },
            env: vec![],
            working_dir: "/tmp/repo".to_string(),
            timeout_seconds: 300,
        }],
    }
}

// ---------------------------------------------------------------------------
// Scenario 1: v0.1.0 report (pre-sharpening) deserializes into current types
// ---------------------------------------------------------------------------

#[test]
fn v010_report_deserializes_with_defaults() {
    let report: AnalysisReport =
        serde_json::from_str(v010_json()).expect("v0.1.0 JSON should deserialize");

    // Core fields are populated.
    assert_eq!(report.run_id, "run-001");
    assert_eq!(report.created_at_epoch_seconds, 1700000000);
    assert_eq!(report.observations.len(), 2);
    assert_eq!(report.changed_paths.len(), 1);

    // Fields added in v0.2.0 default to empty/None.
    assert!(
        report.suspect_surface.is_empty(),
        "suspect_surface should default to empty vec"
    );
    assert!(
        report.reproduction_capsules.is_empty(),
        "reproduction_capsules should default to empty vec"
    );

    // schema_version defaults via default_schema_version().
    assert_eq!(report.schema_version, "0.2.0");

    // Observation-level v0.2.0 fields default correctly.
    let obs = &report.observations[0];
    assert_eq!(obs.sequence_index, 0, "sequence_index should default to 0");
    assert_eq!(
        obs.signal_number, None,
        "signal_number should default to None"
    );
    assert_eq!(
        obs.probe_command, "",
        "probe_command should default to empty string"
    );
    assert_eq!(
        obs.working_dir, "",
        "working_dir should default to empty string"
    );
    assert_eq!(
        obs.flake_signal, None,
        "flake_signal should default to None"
    );
}

// ---------------------------------------------------------------------------
// Scenario 2: v0.2.0 report with all new fields deserializes
// ---------------------------------------------------------------------------

#[test]
fn v020_report_deserializes_with_all_fields() {
    let report: AnalysisReport =
        serde_json::from_str(v020_json()).expect("v0.2.0 JSON should deserialize");

    assert_eq!(report.schema_version, "0.2.0");
    assert_eq!(report.run_id, "run-002");

    // suspect_surface populated
    assert_eq!(report.suspect_surface.len(), 1);
    let suspect = &report.suspect_surface[0];
    assert_eq!(suspect.path, "src/new_file.rs");
    assert_eq!(suspect.priority_score, 100);
    assert_eq!(suspect.surface_kind, "source");
    assert_eq!(suspect.change_status, ChangeStatus::Added);
    assert!(suspect.is_execution_surface);
    assert_eq!(suspect.owner_hint, Some("team-alpha".to_string()));

    // reproduction_capsules populated
    assert_eq!(report.reproduction_capsules.len(), 1);
    let capsule = &report.reproduction_capsules[0];
    assert_eq!(capsule.commit, CommitId("ccc".to_string()));
    assert_eq!(capsule.working_dir, "/tmp/repo");
    assert_eq!(capsule.timeout_seconds, 600);

    // flake_signal on observations
    let obs = &report.observations[0];
    let fs = obs
        .flake_signal
        .as_ref()
        .expect("flake_signal should be present");
    assert_eq!(fs.total_runs, 3);
    assert_eq!(fs.pass_count, 3);
    assert!(fs.is_stable);

    // Other observation-level sharpening fields
    assert_eq!(obs.sequence_index, 1);
    assert_eq!(obs.probe_command, "make test");
    assert_eq!(obs.working_dir, "/tmp/repo");
}

// ---------------------------------------------------------------------------
// Scenario 3: Unknown future fields are ignored (no deny_unknown_fields)
// ---------------------------------------------------------------------------

#[test]
fn unknown_future_fields_are_ignored() {
    // Start from valid v0.2.0 JSON and inject unknown fields at multiple levels.
    let mut val: Value = serde_json::from_str(v020_json()).expect("parse base JSON");

    // Top-level unknown field
    val["future_field"] = Value::String("hello-from-the-future".into());

    // Unknown field inside an observation
    val["observations"][0]["quantum_state"] = Value::String("superposition".into());

    // Unknown field inside suspect_surface entry
    val["suspect_surface"][0]["ai_confidence"] = serde_json::json!(0.99);

    let json = serde_json::to_string(&val).expect("re-serialize");
    let report: AnalysisReport =
        serde_json::from_str(&json).expect("unknown fields should be silently ignored");

    assert_eq!(report.run_id, "run-002");
    assert_eq!(report.suspect_surface.len(), 1);
}

// ---------------------------------------------------------------------------
// Scenario 4: Round-trip serialization preserves all fields
// ---------------------------------------------------------------------------

#[test]
fn round_trip_preserves_all_fields() {
    let original = full_report();

    let json = serde_json::to_string(&original).expect("serialize");
    let deserialized: AnalysisReport = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(original, deserialized, "round-trip should be lossless");
}

// ---------------------------------------------------------------------------
// Scenario 5: schema_version is present and valid semver
// ---------------------------------------------------------------------------

#[test]
fn schema_version_is_valid_semver() {
    let report = full_report();
    let json = serde_json::to_string(&report).expect("serialize");
    let val: Value = serde_json::from_str(&json).expect("parse");

    let version = val["schema_version"]
        .as_str()
        .expect("schema_version should be a string");

    // Validate it looks like semver: MAJOR.MINOR.PATCH
    let parts: Vec<&str> = version.split('.').collect();
    assert_eq!(parts.len(), 3, "semver must have exactly 3 parts");
    for (i, part) in parts.iter().enumerate() {
        part.parse::<u32>().unwrap_or_else(|_| {
            panic!("semver part {i} ({part}) must be a valid integer");
        });
    }
}

#[test]
fn default_schema_version_matches_expected() {
    // When schema_version is absent, the default should be applied.
    let report: AnalysisReport =
        serde_json::from_str(v010_json()).expect("deserialize v0.1.0 JSON");
    assert_eq!(report.schema_version, "0.2.0");

    // When schema_version is explicitly set, it should be preserved.
    let report: AnalysisReport =
        serde_json::from_str(v020_json()).expect("deserialize v0.2.0 JSON");
    assert_eq!(report.schema_version, "0.2.0");
}
