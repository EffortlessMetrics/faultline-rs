//! Markdown dossier export for faultline `AnalysisReport`.
//!
//! `render_markdown` never fails — it returns placeholder text for missing/empty fields.

use faultline_codes::ObservationClass;
use faultline_types::{AnalysisReport, LocalizationOutcome};

/// Render a complete Markdown dossier from an `AnalysisReport`.
///
/// Sections:
/// 1. Outcome summary (one-line)
/// 2. Boundary info (good/bad/window width)
/// 3. Ranked suspect surface (top 10 with scores and owners)
/// 4. Observation timeline (table)
/// 5. Reproduction command (shell one-liner for boundary commit)
/// 6. Artifact links
pub fn render_markdown(report: &AnalysisReport) -> String {
    let mut md = String::new();

    md.push_str("# faultline report\n\n");
    md.push_str(&format!("**Run ID:** `{}`\n\n", report.run_id));

    // 1. Outcome summary
    md.push_str("## Outcome\n\n");
    md.push_str(&render_outcome_summary(&report.outcome));
    md.push_str("\n\n");

    // 2. Boundary info
    md.push_str("## Boundary info\n\n");
    md.push_str(&render_boundary_info(report));
    md.push_str("\n\n");

    // 3. Ranked suspect surface
    md.push_str("## Suspect surface\n\n");
    md.push_str(&render_suspect_surface(report));
    md.push_str("\n");

    // 4. Observation timeline
    md.push_str("## Observation timeline\n\n");
    md.push_str(&render_observation_timeline(report));
    md.push_str("\n");

    // 5. Reproduction command
    md.push_str("## Reproduction\n\n");
    md.push_str(&render_reproduction(report));
    md.push_str("\n\n");

    // 6. Artifact links
    md.push_str("## Artifacts\n\n");
    md.push_str(&render_artifact_links());
    md.push_str("\n");

    md
}
