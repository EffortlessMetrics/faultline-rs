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
