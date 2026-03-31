//! SARIF v2.1.0 export adapter for faultline `AnalysisReport`.

use faultline_types::{AnalysisReport, LocalizationOutcome};
use serde::Serialize;

/// Converts an `AnalysisReport` into a SARIF v2.1.0 JSON string.
pub fn to_sarif(report: &AnalysisReport) -> Result<String, serde_json::Error> {
    let sarif = SarifLog {
        schema: "https://raw.githubusercontent.com/oasis-tcs/sarif-spec/main/sarif-2.1/schema/sarif-schema-2.1.0.json".into(),
        version: "2.1.0".into(),
        runs: vec![build_run(report)],
    };
    serde_json::to_string_pretty(&sarif)
}

fn build_run(report: &AnalysisReport) -> SarifRun {
    let (level, message) = match &report.outcome {
        LocalizationOutcome::FirstBad {
            last_good,
            first_bad,
            ..
        } => (
            "error".to_string(),
            format!("FirstBad: regression introduced after {last_good} in {first_bad}"),
        ),
        LocalizationOutcome::SuspectWindow {
            lower_bound_exclusive,
            upper_bound_inclusive,
            ..
        } => (
            "warning".to_string(),
            format!(
                "SuspectWindow: regression between {lower_bound_exclusive} and {upper_bound_inclusive}"
            ),
        ),
        LocalizationOutcome::Inconclusive { reasons } => {
            let reason_strs: Vec<String> = reasons.iter().map(|r| r.to_string()).collect();
            (
                "note".to_string(),
                format!("Inconclusive: {}", reason_strs.join(", ")),
            )
        }
    };

    let locations: Vec<SarifLocation> = report
        .changed_paths
        .iter()
        .map(|pc| SarifLocation {
            physical_location: SarifPhysicalLocation {
                artifact_location: SarifArtifactLocation {
                    uri: pc.path.clone(),
                },
            },
        })
        .collect();

    let result = SarifResult {
        rule_id: "faultline/localization".to_string(),
        level,
        message: SarifMessage { text: message },
        locations,
    };

    SarifRun {
        tool: SarifTool {
            driver: SarifToolDriver {
                name: "faultline".to_string(),
                version: report.schema_version.clone(),
            },
        },
        results: vec![result],
    }
}

// --- Internal SARIF structs ---

#[derive(Debug, Clone, Serialize)]
struct SarifLog {
    #[serde(rename = "$schema")]
    schema: String,
    version: String,
    runs: Vec<SarifRun>,
}

#[derive(Debug, Clone, Serialize)]
struct SarifRun {
    tool: SarifTool,
    results: Vec<SarifResult>,
}

#[derive(Debug, Clone, Serialize)]
struct SarifTool {
    driver: SarifToolDriver,
}

#[derive(Debug, Clone, Serialize)]
struct SarifToolDriver {
    name: String,
    version: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifResult {
    rule_id: String,
    level: String,
    message: SarifMessage,
    locations: Vec<SarifLocation>,
}

#[derive(Debug, Clone, Serialize)]
struct SarifMessage {
    text: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifLocation {
    physical_location: SarifPhysicalLocation,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifPhysicalLocation {
    artifact_location: SarifArtifactLocation,
}

#[derive(Debug, Clone, Serialize)]
struct SarifArtifactLocation {
    uri: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use faultline_codes::*;
    use faultline_types::*;
    use std::path::PathBuf;

    fn sample_report(outcome: LocalizationOutcome) -> AnalysisReport {
        AnalysisReport {
            schema_version: "0.1.0".into(),
            run_id: "run-1".into(),
            created_at_epoch_seconds: 1700000000,
            request: AnalysisRequest {
                repo_root: PathBuf::from("/tmp/repo"),
                good: RevisionSpec("abc123".into()),
                bad: RevisionSpec("def456".into()),
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
                revisions: vec![CommitId("abc123".into()), CommitId("def456".into())],
            },
            observations: vec![],
            outcome,
            changed_paths: vec![
                PathChange {
                    status: ChangeStatus::Modified,
                    path: "src/main.rs".into(),
                },
                PathChange {
                    status: ChangeStatus::Added,
                    path: "src/lib.rs".into(),
                },
            ],
            surface: SurfaceSummary {
                total_changes: 2,
                buckets: vec![],
                execution_surfaces: vec![],
            },
        }
    }

    #[test]
    fn sarif_first_bad_produces_error_level() {
        let report = sample_report(LocalizationOutcome::FirstBad {
            last_good: CommitId("abc123".into()),
            first_bad: CommitId("def456".into()),
            confidence: Confidence::high(),
        });
        let json = to_sarif(&report).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["version"], "2.1.0");
        assert!(
            v["$schema"]
                .as_str()
                .unwrap()
                .contains("sarif-schema-2.1.0")
        );
        assert_eq!(v["runs"][0]["tool"]["driver"]["name"], "faultline");
        assert_eq!(v["runs"][0]["tool"]["driver"]["version"], "0.1.0");
        assert_eq!(v["runs"][0]["results"][0]["level"], "error");
        assert!(
            v["runs"][0]["results"][0]["message"]["text"]
                .as_str()
                .unwrap()
                .contains("FirstBad")
        );
        assert_eq!(
            v["runs"][0]["results"][0]["locations"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        assert_eq!(
            v["runs"][0]["results"][0]["locations"][0]["physicalLocation"]["artifactLocation"]["uri"],
            "src/main.rs"
        );
    }

    #[test]
    fn sarif_suspect_window_produces_warning_level() {
        let report = sample_report(LocalizationOutcome::SuspectWindow {
            lower_bound_exclusive: CommitId("aaa".into()),
            upper_bound_inclusive: CommitId("bbb".into()),
            confidence: Confidence::medium(),
            reasons: vec![AmbiguityReason::SkippedRevision],
        });
        let json = to_sarif(&report).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["runs"][0]["results"][0]["level"], "warning");
        assert!(
            v["runs"][0]["results"][0]["message"]["text"]
                .as_str()
                .unwrap()
                .contains("SuspectWindow")
        );
    }

    #[test]
    fn sarif_inconclusive_produces_note_level() {
        let report = sample_report(LocalizationOutcome::Inconclusive {
            reasons: vec![AmbiguityReason::MissingPassBoundary],
        });
        let json = to_sarif(&report).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["runs"][0]["results"][0]["level"], "note");
        assert!(
            v["runs"][0]["results"][0]["message"]["text"]
                .as_str()
                .unwrap()
                .contains("Inconclusive")
        );
    }

    #[test]
    fn sarif_empty_changed_paths_produces_empty_locations() {
        let mut report = sample_report(LocalizationOutcome::Inconclusive {
            reasons: vec![AmbiguityReason::MissingFailBoundary],
        });
        report.changed_paths = vec![];
        let json = to_sarif(&report).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(
            v["runs"][0]["results"][0]["locations"]
                .as_array()
                .unwrap()
                .is_empty()
        );
    }

    // Feature: repo-operating-system, Property 41: SARIF Export Structural Validity
    // **Validates: Requirements 3.6**
    mod prop_tests {
        use super::super::*;
        use faultline_fixtures::arb::arb_analysis_report;
        use faultline_types::LocalizationOutcome;
        use proptest::prelude::*;

        proptest! {
            #![proptest_config(ProptestConfig { cases: 100, .. ProptestConfig::default() })]

            #[test]
            fn prop_sarif_export_structural_validity(report in arb_analysis_report()) {
                let json_str = to_sarif(&report).expect("to_sarif must not fail");

                // (a) Valid JSON
                let v: serde_json::Value = serde_json::from_str(&json_str)
                    .expect("SARIF output must be valid JSON");

                // (b) version == "2.1.0"
                prop_assert_eq!(
                    v["version"].as_str().unwrap(),
                    "2.1.0",
                    "SARIF version must be 2.1.0"
                );

                // (c) $schema present
                prop_assert!(
                    v["$schema"].is_string(),
                    "$schema field must be present"
                );

                // (d) Tool name "faultline"
                let tool_name = &v["runs"][0]["tool"]["driver"]["name"];
                prop_assert_eq!(
                    tool_name.as_str().unwrap(),
                    "faultline",
                    "tool name must be faultline"
                );

                // (e) Result level matches outcome type
                let level = v["runs"][0]["results"][0]["level"]
                    .as_str()
                    .expect("level must be a string");
                let expected_level = match &report.outcome {
                    LocalizationOutcome::FirstBad { .. } => "error",
                    LocalizationOutcome::SuspectWindow { .. } => "warning",
                    LocalizationOutcome::Inconclusive { .. } => "note",
                };
                prop_assert_eq!(
                    level,
                    expected_level,
                    "level must match outcome type"
                );
            }
        }
    }
}
