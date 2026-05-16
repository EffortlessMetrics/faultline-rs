//! HTML report rendering for faultline `AnalysisReport`.
//!
//! Each section is produced by a focused helper; [`render_html`] composes
//! them into the final page. Section helpers return empty strings when their
//! input is absent so the page template can splice them in unconditionally.

use faultline_codes::{AmbiguityReason, ObservationClass};
use faultline_types::{
    AnalysisReport, LocalizationOutcome, PathChange, ProbeObservation, SubsystemBucket,
    SuspectEntry,
};

/// Render the complete HTML report. The output is byte-identical to the
/// prior monolithic implementation and is covered by golden snapshots.
pub(crate) fn render_html(report: &AnalysisReport) -> String {
    let mut sorted_observations = report.observations.clone();
    sorted_observations.sort_by_key(|obs| obs.sequence_index);

    let outcome_class = outcome_css_class(&report.outcome);
    let outcome_html = render_outcome(&report.outcome);
    let observations = render_observation_rows(&sorted_observations);
    let buckets = render_surface_buckets(&report.surface.buckets);
    let execution_surfaces_html = render_execution_surfaces(&report.surface.execution_surfaces);
    let changed_paths = render_changed_paths(&report.changed_paths);
    let suspect_surface_html = render_suspect_surface(&report.suspect_surface);
    let log_section = render_log_links(&sorted_observations);

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
    .outcome-firstbad {{ border-left: 4px solid #22c55e; padding: 0.75rem; margin: 0.5rem 0; }}
    .outcome-suspect {{ border-left: 4px solid #f59e0b; padding: 0.75rem; margin: 0.5rem 0; }}
    .outcome-inconclusive {{ border-left: 4px solid #ef4444; padding: 0.75rem; margin: 0.5rem 0; }}
    .badge {{ display: inline-block; padding: 0.15rem 0.5rem; border-radius: 4px; font-size: 0.85em; background: #e5e7eb; margin: 0.1rem; }}
    .badge-signal {{ background: #fef3c7; color: #92400e; }}
    .obs-pass {{ background: #f0fdf4; }}
    .obs-fail {{ background: #fef2f2; }}
    .obs-skip {{ background: #f3f4f6; }}
    .obs-indeterminate {{ background: #fefce8; }}
    .execution-surfaces {{ background: #fffbeb; border: 1px solid #f59e0b; border-radius: 4px; padding: 0.5rem; }}
    .suspect-list {{ list-style: none; padding: 0; }}
    .suspect-entry {{ padding: 0.5rem; margin: 0.25rem 0; border: 1px solid #e5e7eb; border-radius: 4px; }}
    .suspect-entry.exec-surface {{ border-left: 4px solid #f59e0b; background: #fffbeb; font-weight: bold; }}
    .suspect-score {{ display: inline-block; min-width: 3em; text-align: right; margin-right: 0.5rem; color: #6b7280; font-size: 0.85em; }}
    .suspect-kind {{ display: inline-block; padding: 0.1rem 0.4rem; border-radius: 4px; font-size: 0.8em; background: #e5e7eb; margin-left: 0.5rem; }}
    .suspect-status {{ display: inline-block; padding: 0.1rem 0.4rem; border-radius: 4px; font-size: 0.8em; background: #dbeafe; margin-left: 0.25rem; }}
    .suspect-owner {{ color: #6b7280; font-size: 0.85em; margin-left: 0.5rem; }}
    .badge-exec {{ background: #fef3c7; color: #92400e; }}
  </style>
</head>
<body>
  <h1>faultline</h1>
  <p>Run ID: <code>{}</code></p>
  <h2>Summary</h2>
  <div class="{}">
  {}
  </div>
  <p><strong>Probe fingerprint:</strong> <code>{}</code></p>
  <p><strong>History mode:</strong> {:?}</p>
  <h2>Observation timeline</h2>
  <table>
    <thead><tr><th>Commit</th><th>Class</th><th>Kind</th><th>Exit</th><th>Duration ms</th></tr></thead>
    <tbody>{}</tbody>
  </table>
  <h2>Changed surface</h2>
  <ul>{}</ul>
  {}
  <h2>Changed paths</h2>
  <ul>{}</ul>{}{}
</body>
</html>"#,
        escape_html(&report.run_id),
        outcome_class,
        outcome_html,
        escape_html(&report.request.probe.fingerprint()),
        report.request.history_mode,
        observations,
        buckets,
        execution_surfaces_html,
        changed_paths,
        suspect_surface_html,
        log_section,
    )
}

/// CSS class that selects the colored border for the outcome summary box.
fn outcome_css_class(outcome: &LocalizationOutcome) -> &'static str {
    match outcome {
        LocalizationOutcome::FirstBad { .. } => "outcome-firstbad",
        LocalizationOutcome::SuspectWindow { .. } => "outcome-suspect",
        LocalizationOutcome::Inconclusive { .. } => "outcome-inconclusive",
    }
}

/// One-line outcome summary with confidence and (where applicable) reason badges.
fn render_outcome(outcome: &LocalizationOutcome) -> String {
    match outcome {
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
            "<p><strong>Suspect window:</strong> {} → {} (confidence: {} / {})</p><p>{}</p>",
            escape_html(&lower_bound_exclusive.0),
            escape_html(&upper_bound_inclusive.0),
            confidence.score,
            escape_html(&confidence.label),
            render_reason_badges(reasons),
        ),
        LocalizationOutcome::Inconclusive { reasons } => format!(
            "<p><strong>Inconclusive.</strong></p><p>{}</p>",
            render_reason_badges(reasons),
        ),
    }
}

/// Space-joined `<span class="badge ...">` for each ambiguity reason.
fn render_reason_badges(reasons: &[AmbiguityReason]) -> String {
    reasons
        .iter()
        .map(|reason| {
            let reason_slug = escape_html(&reason.to_string()).replace(' ', "-");
            format!(
                "<span class=\"badge badge-{}\">{}</span>",
                reason_slug,
                escape_html(&reason.to_string()),
            )
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Observation timeline rows. Caller is expected to sort by `sequence_index`.
fn render_observation_rows(sorted: &[ProbeObservation]) -> String {
    sorted
        .iter()
        .map(|obs| {
            let row_class = match obs.class {
                ObservationClass::Pass => "obs-pass",
                ObservationClass::Fail => "obs-fail",
                ObservationClass::Skip => "obs-skip",
                ObservationClass::Indeterminate => "obs-indeterminate",
            };
            let signal_badge = if obs.class == ObservationClass::Indeterminate {
                if let Some(sig) = obs.signal_number {
                    format!(" <span class=\"badge badge-signal\">signal {sig}</span>")
                } else {
                    String::new()
                }
            } else {
                String::new()
            };
            format!(
                "<tr class=\"{}\"><td><code>{}</code></td><td>{:?}{}</td><td>{}</td><td>{:?}</td><td>{}</td></tr>",
                row_class,
                escape_html(&obs.commit.0),
                obs.class,
                signal_badge,
                escape_html(&obs.kind.to_string()),
                obs.exit_code,
                obs.duration_ms,
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Subsystem-bucket list items for the "Changed surface" section.
fn render_surface_buckets(buckets: &[SubsystemBucket]) -> String {
    buckets
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
        .join("\n")
}

/// Highlighted "Execution surfaces" section. Empty string when none exist.
fn render_execution_surfaces(surfaces: &[String]) -> String {
    if surfaces.is_empty() {
        return String::new();
    }
    let items = surfaces
        .iter()
        .map(|path| format!("<li><code>{}</code></li>", escape_html(path)))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "  <h2>Execution surfaces</h2>\n  <div class=\"execution-surfaces\">\n  <ul>{items}</ul>\n  </div>",
    )
}

/// "Changed paths" list items.
fn render_changed_paths(changes: &[PathChange]) -> String {
    changes
        .iter()
        .map(|change| {
            format!(
                "<li><code>{:?}</code> {}</li>",
                change.status,
                escape_html(&change.path),
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Ranked "Suspect surface" section. Empty string when no entries.
fn render_suspect_surface(entries: &[SuspectEntry]) -> String {
    if entries.is_empty() {
        return String::new();
    }
    let items = entries
        .iter()
        .map(render_suspect_entry)
        .collect::<Vec<_>>()
        .join("\n");
    format!("\n  <h2>Suspect surface</h2>\n  <ul class=\"suspect-list\">{items}</ul>")
}

fn render_suspect_entry(entry: &SuspectEntry) -> String {
    let entry_class = if entry.is_execution_surface {
        "suspect-entry exec-surface"
    } else {
        "suspect-entry"
    };
    let exec_badge = if entry.is_execution_surface {
        " <span class=\"badge badge-exec\">exec</span>".to_string()
    } else {
        String::new()
    };
    let owner = entry
        .owner_hint
        .as_ref()
        .map(|o| {
            format!(
                " <span class=\"suspect-owner\">owner: {}</span>",
                escape_html(o)
            )
        })
        .unwrap_or_default();
    format!(
        "<li class=\"{}\"><span class=\"suspect-score\">{}</span><code>{}</code>{}<span class=\"suspect-kind\">{}</span><span class=\"suspect-status\">{:?}</span>{}</li>",
        entry_class,
        entry.priority_score,
        escape_html(&entry.path),
        exec_badge,
        escape_html(&entry.surface_kind),
        entry.change_status,
        owner,
    )
}

/// "Probe logs" section linking to on-disk log files for truncated probe output.
fn render_log_links(sorted: &[ProbeObservation]) -> String {
    let log_links: Vec<String> = sorted
        .iter()
        .filter(|obs| obs.stdout.ends_with("[truncated]") || obs.stderr.ends_with("[truncated]"))
        .map(render_log_link_entry)
        .collect();

    if log_links.is_empty() {
        String::new()
    } else {
        format!(
            "\n  <h2>Probe logs</h2>\n  <ul>{}</ul>",
            log_links.join("\n"),
        )
    }
}

fn render_log_link_entry(obs: &ProbeObservation) -> String {
    let sha = escape_html(&obs.commit.0);
    let mut links = Vec::new();
    if obs.stdout.ends_with("[truncated]") {
        links.push(format!(
            "<a href=\"logs/{}_stdout.log\">stdout</a>",
            escape_html(&obs.commit.0),
        ));
    }
    if obs.stderr.ends_with("[truncated]") {
        links.push(format!(
            "<a href=\"logs/{}_stderr.log\">stderr</a>",
            escape_html(&obs.commit.0),
        ));
    }
    format!("<li><code>{}</code>: {}</li>", sha, links.join(", "))
}

/// HTML-escape the five characters that have special meaning in markup.
pub(crate) fn escape_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
