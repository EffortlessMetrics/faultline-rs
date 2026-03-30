use faultline_types::{AnalysisReport, LocalizationOutcome, Result};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct ReportRenderer {
    output_dir: PathBuf,
}

impl ReportRenderer {
    pub fn new(output_dir: impl Into<PathBuf>) -> Self {
        Self {
            output_dir: output_dir.into(),
        }
    }

    pub fn render(&self, report: &AnalysisReport) -> Result<()> {
        fs::create_dir_all(&self.output_dir)?;
        fs::write(
            self.output_dir.join("analysis.json"),
            serde_json::to_string_pretty(report)?,
        )?;
        fs::write(self.output_dir.join("index.html"), self.render_html(report))?;
        Ok(())
    }

    pub fn output_dir(&self) -> &Path {
        &self.output_dir
    }

    fn render_html(&self, report: &AnalysisReport) -> String {
        let outcome_html = match &report.outcome {
            LocalizationOutcome::FirstBad {
                last_good,
                first_bad,
                confidence,
            } => format!(
                "<p><strong>Exact boundary:</strong> {} → {} (confidence: {} / {})</p>",
                escape_html(&last_good.0),
                escape_html(&first_bad.0),
                confidence.score,
                escape_html(&confidence.label),
            ),
            LocalizationOutcome::SuspectWindow {
                lower_bound_exclusive,
                upper_bound_inclusive,
                confidence,
                reasons,
            } => format!(
                "<p><strong>Suspect window:</strong> {} → {} (confidence: {} / {})</p><p>Reasons: {}</p>",
                escape_html(&lower_bound_exclusive.0),
                escape_html(&upper_bound_inclusive.0),
                confidence.score,
                escape_html(&confidence.label),
                reasons
                    .iter()
                    .map(|reason| escape_html(&reason.to_string()))
                    .collect::<Vec<_>>()
                    .join("\n"),
            ),
            LocalizationOutcome::Inconclusive { reasons } => format!(
                "<p><strong>Inconclusive.</strong> Reasons: {}</p>",
                reasons
                    .iter()
                    .map(|reason| escape_html(&reason.to_string()))
                    .collect::<Vec<_>>()
                    .join("\n"),
            ),
        };

        let observations = report
            .observations
            .iter()
            .map(|obs| {
                format!(
                    "<tr><td><code>{}</code></td><td>{:?}</td><td>{}</td><td>{:?}</td><td>{}</td></tr>",
                    escape_html(&obs.commit.0),
                    obs.class,
                    escape_html(&obs.kind.to_string()),
                    obs.exit_code,
                    obs.duration_ms,
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        let buckets = report
            .surface
            .buckets
            .iter()
            .map(|bucket| {
                format!(
                    "<li><strong>{}</strong> — {} changes — kinds: {}<br/><small>{}</small></li>",
                    escape_html(&bucket.name),
                    bucket.change_count,
                    escape_html(&bucket.surface_kinds.join(", ")),
                    escape_html(&bucket.paths.join(", ")),
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        let changed_paths = report
            .changed_paths
            .iter()
            .map(|change| {
                format!(
                    "<li><code>{:?}</code> {}</li>",
                    change.status,
                    escape_html(&change.path),
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <title>faultline report</title>
  <style>
    body {{ font-family: system-ui, sans-serif; margin: 2rem auto; max-width: 1100px; line-height: 1.5; }}
    code {{ background: #f3f4f6; padding: 0.15rem 0.35rem; border-radius: 4px; }}
    table {{ width: 100%; border-collapse: collapse; }}
    th, td {{ border: 1px solid #ddd; padding: 0.5rem; text-align: left; }}
    th {{ background: #f8f8f8; }}
  </style>
</head>
<body>
  <h1>faultline</h1>
  <p>Run ID: <code>{}</code></p>
  <h2>Summary</h2>
  {}
  <p><strong>Probe fingerprint:</strong> <code>{}</code></p>
  <p><strong>History mode:</strong> {:?}</p>
  <h2>Observation timeline</h2>
  <table>
    <thead><tr><th>Commit</th><th>Class</th><th>Kind</th><th>Exit</th><th>Duration ms</th></tr></thead>
    <tbody>{}</tbody>
  </table>
  <h2>Changed surface</h2>
  <ul>{}</ul>
  <h2>Changed paths</h2>
  <ul>{}</ul>
</body>
</html>"#,
            escape_html(&report.run_id),
            outcome_html,
            escape_html(&report.request.probe.fingerprint()),
            report.request.history_mode,
            observations,
            buckets,
            changed_paths,
        )
    }
}

fn escape_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use faultline_codes::{ObservationClass, ProbeKind};
    use faultline_types::*;
    use std::path::PathBuf;

    fn sample_report() -> AnalysisReport {
        AnalysisReport {
            run_id: "test-run-001".into(),
            created_at_epoch_seconds: 1700000000,
            request: AnalysisRequest {
                repo_root: PathBuf::from("/tmp/repo"),
                good: RevisionSpec("aaa111".into()),
                bad: RevisionSpec("bbb222".into()),
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
                revisions: vec![CommitId("aaa111".into()), CommitId("bbb222".into())],
            },
            observations: vec![
                ProbeObservation {
                    commit: CommitId("aaa111".into()),
                    class: ObservationClass::Pass,
                    kind: ProbeKind::Test,
                    exit_code: Some(0),
                    timed_out: false,
                    duration_ms: 120,
                    stdout: "ok".into(),
                    stderr: String::new(),
                },
                ProbeObservation {
                    commit: CommitId("bbb222".into()),
                    class: ObservationClass::Fail,
                    kind: ProbeKind::Test,
                    exit_code: Some(1),
                    timed_out: false,
                    duration_ms: 95,
                    stdout: String::new(),
                    stderr: "test failed".into(),
                },
            ],
            outcome: LocalizationOutcome::FirstBad {
                last_good: CommitId("aaa111".into()),
                first_bad: CommitId("bbb222".into()),
                confidence: Confidence::high(),
            },
            changed_paths: vec![PathChange {
                status: ChangeStatus::Modified,
                path: "src/main.rs".into(),
            }],
            surface: SurfaceSummary {
                total_changes: 1,
                buckets: vec![SubsystemBucket {
                    name: "src".into(),
                    change_count: 1,
                    paths: vec!["src/main.rs".into()],
                    surface_kinds: vec!["source".into()],
                }],
                execution_surfaces: vec![],
            },
        }
    }

    fn temp_output_dir(name: &str) -> PathBuf {
        std::env::temp_dir()
            .join("faultline-render-tests")
            .join(name)
            .join(format!("{}", std::process::id()))
    }

    // Req 6.1: writes analysis.json to output directory
    #[test]
    fn render_writes_analysis_json() {
        let dir = temp_output_dir("writes-json");
        let _ = std::fs::remove_dir_all(&dir);
        let renderer = ReportRenderer::new(&dir);
        let report = sample_report();

        renderer.render(&report).expect("render should succeed");

        let json_path = dir.join("analysis.json");
        assert!(json_path.exists(), "analysis.json must be created");
        let _ = std::fs::remove_dir_all(&dir);
    }

    // Req 6.2: analysis.json contains all required fields
    #[test]
    fn analysis_json_contains_all_fields() {
        let dir = temp_output_dir("all-fields");
        let _ = std::fs::remove_dir_all(&dir);
        let renderer = ReportRenderer::new(&dir);
        let report = sample_report();

        renderer.render(&report).expect("render should succeed");

        let content = std::fs::read_to_string(dir.join("analysis.json")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();

        assert!(parsed.get("run_id").is_some());
        assert!(parsed.get("created_at_epoch_seconds").is_some());
        assert!(parsed.get("request").is_some());
        assert!(parsed.get("sequence").is_some());
        assert!(parsed.get("observations").is_some());
        assert!(parsed.get("outcome").is_some());
        assert!(parsed.get("changed_paths").is_some());
        assert!(parsed.get("surface").is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }

    // Req 6.3: deterministic output — same report produces identical JSON
    #[test]
    fn analysis_json_is_deterministic() {
        let dir1 = temp_output_dir("deterministic-1");
        let dir2 = temp_output_dir("deterministic-2");
        let _ = std::fs::remove_dir_all(&dir1);
        let _ = std::fs::remove_dir_all(&dir2);

        let report = sample_report();
        ReportRenderer::new(&dir1).render(&report).unwrap();
        ReportRenderer::new(&dir2).render(&report).unwrap();

        let json1 = std::fs::read_to_string(dir1.join("analysis.json")).unwrap();
        let json2 = std::fs::read_to_string(dir2.join("analysis.json")).unwrap();
        assert_eq!(
            json1, json2,
            "identical reports must produce identical JSON"
        );

        let _ = std::fs::remove_dir_all(&dir1);
        let _ = std::fs::remove_dir_all(&dir2);
    }

    // Req 6.4: analysis.json is valid JSON
    #[test]
    fn analysis_json_is_valid_json() {
        let dir = temp_output_dir("valid-json");
        let _ = std::fs::remove_dir_all(&dir);
        let renderer = ReportRenderer::new(&dir);
        let report = sample_report();

        renderer.render(&report).unwrap();

        let content = std::fs::read_to_string(dir.join("analysis.json")).unwrap();
        let result: std::result::Result<serde_json::Value, _> = serde_json::from_str(&content);
        assert!(result.is_ok(), "analysis.json must be valid JSON");
        let _ = std::fs::remove_dir_all(&dir);
    }

    // Creates output directory if it doesn't exist
    #[test]
    fn render_creates_output_directory() {
        let dir = temp_output_dir("creates-dir");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(!dir.exists());

        let renderer = ReportRenderer::new(&dir);
        renderer.render(&sample_report()).unwrap();

        assert!(dir.exists(), "output directory must be created");
        let _ = std::fs::remove_dir_all(&dir);
    }

    // output_dir accessor returns the configured path
    #[test]
    fn output_dir_returns_configured_path() {
        let dir = PathBuf::from("/some/output/path");
        let renderer = ReportRenderer::new(&dir);
        assert_eq!(renderer.output_dir(), dir.as_path());
    }

    // Round-trip: deserializing the written JSON produces the original report
    #[test]
    fn analysis_json_round_trips() {
        let dir = temp_output_dir("round-trip");
        let _ = std::fs::remove_dir_all(&dir);
        let renderer = ReportRenderer::new(&dir);
        let report = sample_report();

        renderer.render(&report).unwrap();

        let content = std::fs::read_to_string(dir.join("analysis.json")).unwrap();
        let deserialized: AnalysisReport = serde_json::from_str(&content).unwrap();
        assert_eq!(report, deserialized);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
