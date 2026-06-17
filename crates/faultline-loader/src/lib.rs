//! Shared report-loading infrastructure for faultline.
//!
//! This crate provides the [`locate_and_load_report`] function, which resolves
//! and loads an `AnalysisReport` from either a directory path or a direct file
//! path. It implements deterministic precedence: `report.json` > `analysis.json`
//! when given a directory.
//!
//! Both `faultline-cli` and `xtask` depend on this crate to avoid duplicating
//! report-loading logic.

use faultline_types::{AnalysisReport, ArtifactSource, FaultlineError, LocatedReport, Result};
use std::path::Path;

/// Resolve and load an `AnalysisReport` from a path.
///
/// If `path` is a directory:
///   1. `report.json` if present (preferred)
///   2. `analysis.json` if present (fallback)
///   3. Error if neither exists
///
/// If `path` is a file:
///   Load directly from that file path.
///
/// When both `report.json` and `analysis.json` are present in a directory,
/// a diagnostic message is added to `LocatedReport::diagnostics`.
pub fn locate_and_load_report(path: &Path) -> Result<LocatedReport> {
    if path.is_file() {
        return load_from_file(path);
    }

    if path.is_dir() {
        return load_from_directory(path);
    }

    Err(FaultlineError::Store(format!(
        "path does not exist or is not accessible: {}",
        path.display()
    )))
}

fn load_from_file(path: &Path) -> Result<LocatedReport> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| FaultlineError::Store(format!("failed to read {}: {}", path.display(), e)))?;

    let report: AnalysisReport = serde_json::from_str(&content)
        .map_err(|e| FaultlineError::Store(format!("failed to parse {}: {}", path.display(), e)))?;

    Ok(LocatedReport {
        report,
        source: ArtifactSource::DirectFile,
        diagnostics: Vec::new(),
    })
}

fn load_from_directory(dir: &Path) -> Result<LocatedReport> {
    let report_json = dir.join("report.json");
    let analysis_json = dir.join("analysis.json");

    let has_report = report_json.exists();
    let has_analysis = analysis_json.exists();

    match (has_report, has_analysis) {
        (true, true) => {
            let report = read_and_parse(&report_json)?;
            Ok(LocatedReport {
                report,
                source: ArtifactSource::ReportJson,
                diagnostics: vec![format!(
                    "both report.json and analysis.json present in {}; chose report.json",
                    dir.display()
                )],
            })
        }
        (true, false) => {
            let report = read_and_parse(&report_json)?;
            Ok(LocatedReport {
                report,
                source: ArtifactSource::ReportJson,
                diagnostics: Vec::new(),
            })
        }
        (false, true) => {
            let report = read_and_parse(&analysis_json)?;
            Ok(LocatedReport {
                report,
                source: ArtifactSource::AnalysisJson,
                diagnostics: Vec::new(),
            })
        }
        (false, false) => Err(FaultlineError::Store(format!(
            "no report.json or analysis.json found in {}",
            dir.display()
        ))),
    }
}

fn read_and_parse(path: &Path) -> Result<AnalysisReport> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| FaultlineError::Store(format!("failed to read {}: {}", path.display(), e)))?;

    serde_json::from_str(&content)
        .map_err(|e| FaultlineError::Store(format!("failed to parse {}: {}", path.display(), e)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use faultline_fixtures::arb::arb_analysis_report;
    use faultline_types::ArtifactSource;
    use proptest::prelude::*;
    use tempfile::TempDir;

    /// Helper: serialize a report to JSON bytes.
    fn report_to_json(report: &AnalysisReport) -> Vec<u8> {
        serde_json::to_vec(report).expect("report serialization should not fail")
    }

    /// Strategy that generates one of four directory layouts:
    /// 0 = only report.json
    /// 1 = only analysis.json
    /// 2 = both report.json and analysis.json
    /// 3 = neither (empty directory)
    fn arb_layout() -> impl Strategy<Value = u8> {
        0u8..4
    }

    // **Validates: Requirements 2.1, 2.2**
    proptest! {
        #![proptest_config(ProptestConfig { cases: 100, .. ProptestConfig::default() })]

        /// Property 1: Report Locator Precedence
        ///
        /// For any directory containing any combination of report.json and
        /// analysis.json files, locate_and_load_report returns the file matching
        /// deterministic precedence: report.json > analysis.json, error when
        /// neither exists, and non-empty diagnostics when both are present.
        #[test]
        fn prop_report_locator_precedence(
            report in arb_analysis_report(),
            layout in arb_layout(),
        ) {
            let dir = TempDir::new().expect("failed to create temp dir");
            let dir_path = dir.path();

            let report_json_bytes = report_to_json(&report);

            // Write files based on layout
            match layout {
                0 => {
                    // Only report.json
                    std::fs::write(dir_path.join("report.json"), &report_json_bytes).unwrap();
                }
                1 => {
                    // Only analysis.json
                    std::fs::write(dir_path.join("analysis.json"), &report_json_bytes).unwrap();
                }
                2 => {
                    // Both report.json and analysis.json
                    std::fs::write(dir_path.join("report.json"), &report_json_bytes).unwrap();
                    std::fs::write(dir_path.join("analysis.json"), &report_json_bytes).unwrap();
                }
                3 => {
                    // Neither — empty directory
                }
                _ => unreachable!(),
            }

            let result = locate_and_load_report(dir_path);

            match layout {
                0 => {
                    // Only report.json → ReportJson, empty diagnostics
                    let located = result.expect("should succeed with report.json present");
                    prop_assert_eq!(located.source, ArtifactSource::ReportJson,
                        "with only report.json, source must be ReportJson");
                    prop_assert!(located.diagnostics.is_empty(),
                        "with only report.json, diagnostics must be empty");
                    prop_assert_eq!(located.report, report,
                        "loaded report must match written report");
                }
                1 => {
                    // Only analysis.json → AnalysisJson, empty diagnostics
                    let located = result.expect("should succeed with analysis.json present");
                    prop_assert_eq!(located.source, ArtifactSource::AnalysisJson,
                        "with only analysis.json, source must be AnalysisJson");
                    prop_assert!(located.diagnostics.is_empty(),
                        "with only analysis.json, diagnostics must be empty");
                    prop_assert_eq!(located.report, report,
                        "loaded report must match written report");
                }
                2 => {
                    // Both → ReportJson, non-empty diagnostics
                    let located = result.expect("should succeed with both files present");
                    prop_assert_eq!(located.source, ArtifactSource::ReportJson,
                        "with both files, source must be ReportJson (precedence)");
                    prop_assert!(!located.diagnostics.is_empty(),
                        "with both files present, diagnostics must be non-empty");
                    prop_assert_eq!(located.report, report,
                        "loaded report must match written report.json content");
                }
                3 => {
                    // Neither → error
                    prop_assert!(result.is_err(),
                        "with neither file present, must return an error");
                }
                _ => unreachable!(),
            }
        }

        /// Property 1 (supplement): Direct file path returns DirectFile source.
        #[test]
        fn prop_direct_file_returns_direct_file_source(
            report in arb_analysis_report(),
        ) {
            let dir = TempDir::new().expect("failed to create temp dir");
            let file_path = dir.path().join("custom_report.json");
            let report_json_bytes = report_to_json(&report);
            std::fs::write(&file_path, &report_json_bytes).unwrap();

            let result = locate_and_load_report(&file_path);
            let located = result.expect("should succeed loading from direct file path");

            prop_assert_eq!(located.source, ArtifactSource::DirectFile,
                "direct file path must return DirectFile source");
            prop_assert!(located.diagnostics.is_empty(),
                "direct file path must have empty diagnostics");
            prop_assert_eq!(located.report, report,
                "loaded report must match written report");
        }
    }
}
